#!/usr/bin/env python3
"""Stable libtest text baseline helper for upeg."""

import argparse
import json
import os
from datetime import datetime, timezone
from pathlib import Path
import re
import subprocess
import sys
import tempfile


PROJECT = "upeg"
VERSION = 1
BASELINE_FILE = "fixtures/test-baseline.json"
CURRENT_FILE = "target/test-baseline/current.json"
DIFF_FILE = "target/test-baseline/diff.json"
COMMENT_FILE = "target/test-baseline/pr-comment.md"
COMMENT_MARKER = "<!-- upeg-test-baseline -->"
COMMENT_BODY_LIMIT = 60000
COMMENT_OMITTED_ROW_BUDGET = 160
COMMENT_SECTION_ITEM_LIMIT = 30
ERROR_OUTPUT_LINE_LIMIT = 80
FAILURE_EXCERPT_LINE_LIMIT = 40
EXCERPT_ELISION = "    ... (excerpt trimmed) ..."

# `--no-fail-fast` is what makes a red run still describe the *whole*
# corpus: without it cargo stops after the first failing target and every
# unreached test would look "removed" to the diff.
SUITES = {
    "workspace": ["cargo", "test", "--workspace", "--all-targets", "--no-fail-fast"],
    "docs": ["cargo", "test", "--workspace", "--doc", "--no-fail-fast"],
    "flutter": ["flutter", "test", "--reporter=json"],
}

# Suites this script used to run. `wasm-plugin` became a *default*
# upeg-cli feature, so the `workspace` suite is a strict superset of
# `cargo test -p upeg-cli --features wasm-plugin --lib` and running it
# separately only inflated the corpus. Kept as a named fixture so the
# self-test can still exercise the "baseline JSON from an older commit
# carries a suite we no longer run" path (`require_exact_suites=False`).
RETIRED_SUITES = {
    "wasmPlugin": ["cargo", "test", "-p", "upeg-cli", "--features", "wasm-plugin", "--lib"],
}
FLUTTER_SUITE_NAME = "flutter"
FLUTTER_SUITE_CWD = "flutter_app"

# The repo dogfoods itself: `<root>/.upeg/toolkits/dev.toml` is a real project Toolkit
# declaring the `dev.*` toolkit. Every cargo suite runs with the repo as
# cwd, so without this the test processes would auto-detect it and load
# 15 external tools into the toolbox they are asserting against. `off`
# disables Project Manifest detection (`upeg_sources::project` module docs);
# the same pair is set by the Justfile's `hermetic_sources`.
PROJECT_MANIFEST_PATH_ENV = "UPEG_PROJECT_MANIFEST_PATH"
PROJECT_MANIFEST_OVERRIDE_OFF = "off"
TOOLKIT_TEST_HOME_ENV = "UPEG_TOOLKIT_TEST_HOME"


def hermetic_env():
    """Keep test diagnostics and runtime state outside the operator's home."""
    env = dict(os.environ)
    for key in ("UPEG_TOOLKITS_DIR", "UPEG_WASM_DIR", "UPEG_MCP_IMPORTS_DIR", "UPEG_LOG_PATH", "UPEG_CREDENTIALS_PATH"):
        env.pop(key, None)
    env["UPEG_HOME"] = env.get(
        TOOLKIT_TEST_HOME_ENV,
        str(Path(__file__).resolve().parents[1] / "target/hermetic-sources/state"),
    )
    env[PROJECT_MANIFEST_PATH_ENV] = PROJECT_MANIFEST_OVERRIDE_OFF
    return env
REQUIRED_TEST_FIELDS = ("className", "methodName", "status", "target", "fullName")
VALID_STATUSES = {"passed", "failed", "ignored"}

TEST_STATUS_RE = re.compile(
    r"^test (?P<name>.+?)\s+\.\.\.\s+(?P<status>ok|FAILED|ignored)(?P<reason>, .+)?$"
)
TEST_STARTED_RE = re.compile(r"^test (?P<name>.+?)\s+\.\.\.(?:\s+|$)")
# libtest appends this marker to the NAME column when it runs a
# `#[should_panic]` test (`test some::case - should panic ... ok`), but
# `--list` reports the bare name. Stripping it keeps the discovered set and
# the run set comparable; without it every `#[should_panic]` test reads as
# "listed but never ran".
SHOULD_PANIC_SUFFIX = " - should panic"
LIST_RE = re.compile(r"^(?P<name>.+):\s+(?P<kind>test|bench)$")
RESULT_FAILED_RE = re.compile(r"^test result:\s+FAILED\.", re.MULTILINE)
RESULT_SUMMARY_RE = re.compile(
    r"^test result:\s+(?:ok|FAILED)\.\s+(?P<passed>\d+) passed;\s+"
    r"(?P<failed>\d+) failed;\s+(?P<ignored>\d+) ignored\b"
)
ANSI_RE = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")
FAILURE_BLOCK_RE = re.compile(r"^----\s+(?P<name>.+?)\s+(?:stdout|stderr)\s+----$")
FAILURE_LIST_RE = re.compile(r"^failures:$")


class BaselineError(Exception):
    pass


def utc_rfc3339z():
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def command_text(argv):
    return " ".join(argv)


def discovery_command(argv):
    return list(argv) + ["--", "--list"]


def repo_root():
    return Path(__file__).resolve().parents[1]


def status_name(raw):
    if raw == "ok":
        return "passed"
    if raw == "FAILED":
        return "failed"
    if raw == "ignored":
        return "ignored"
    raise BaselineError("unknown libtest status: " + raw)


def strip_hash_from_artifact(path_text):
    artifact = Path(path_text).name
    if "-" not in artifact:
        return artifact
    prefix, suffix = artifact.rsplit("-", 1)
    if suffix and all(char in "0123456789abcdef" for char in suffix.lower()):
        return prefix
    return artifact


def strip_ansi(text):
    return ANSI_RE.sub("", text)


def parse_running_target(line):
    stripped = strip_ansi(line).strip()
    if stripped.startswith("Doc-tests "):
        crate = stripped[len("Doc-tests ") :].strip()
        return "doc:" + crate
    if not stripped.startswith("Running "):
        return None

    body = stripped[len("Running ") :]
    artifact = ""
    if " (" in body and body.endswith(")"):
        body, artifact = body.rsplit(" (", 1)
        artifact = artifact[:-1]

    if body.startswith("unittests "):
        body = body[len("unittests ") :]

    body = body.strip()
    crate_or_test = strip_hash_from_artifact(artifact) if artifact else ""
    if crate_or_test:
        return crate_or_test + "::" + body
    return body or "unknown"


def split_class_method(full_name, target):
    if "::" in full_name:
        parts = full_name.split("::")
        return "::".join(parts[:-1]), parts[-1]
    if " - " in full_name:
        class_name, method_name = full_name.split(" - ", 1)
        return class_name.strip(), method_name.strip()
    return target, full_name


def parse_test_list(output):
    tests = []
    for line in output.splitlines():
        match = LIST_RE.match(strip_ansi(line).strip())
        if match and match.group("kind") == "test":
            tests.append(match.group("name"))
    return tests


def parse_libtest_output(output):
    tests = []
    current_target = "unknown"
    pending_name = None
    continued_status = False
    split_status_seen = False
    target_start_index = 0

    def add_status(full_name, raw_status):
        if full_name.endswith(SHOULD_PANIC_SUFFIX):
            full_name = full_name[: -len(SHOULD_PANIC_SUFFIX)]
        class_name, method_name = split_class_method(full_name, current_target)
        tests.append(
            {
                "className": class_name,
                "methodName": method_name,
                "status": status_name(raw_status),
                "target": current_target,
                "fullName": full_name,
            }
        )

    for line in output.splitlines():
        target = parse_running_target(line)
        if target:
            if split_status_seen:
                raise BaselineError("split libtest status has no summary")
            current_target = target
            pending_name = None
            continued_status = False
            target_start_index = len(tests)
            continue

        clean_line = strip_ansi(line).strip()
        match = TEST_STATUS_RE.match(clean_line)
        if match and (not match.group("reason") or match.group("status") == "ignored"):
            add_status(match.group("name"), match.group("status"))
            pending_name = None
            continued_status = False
            continue

        started = TEST_STARTED_RE.match(clean_line)
        if started:
            pending_name = started.group("name")
            continued_status = False
        elif clean_line in ("ok", "FAILED", "ignored"):
            if pending_name is not None:
                add_status(pending_name, clean_line)
                pending_name = None
                continued_status = True
                split_status_seen = True
            elif continued_status:
                raise BaselineError("ambiguous libtest status after a split status line")
        elif clean_line.startswith("test result:"):
            if split_status_seen:
                summary = RESULT_SUMMARY_RE.match(clean_line)
                if not summary:
                    raise BaselineError("split libtest status has no parseable summary")
                target_tests = tests[target_start_index:]
                for status in ("passed", "failed", "ignored"):
                    actual = sum(test["status"] == status for test in target_tests)
                    if actual != int(summary.group(status)):
                        raise BaselineError("split libtest status counts disagree with summary")
                split_status_seen = False
            pending_name = None
            continued_status = False

    if split_status_seen:
        raise BaselineError("split libtest status has no summary")

    tests.sort(key=lambda item: (item["target"], item["fullName"]))
    return tests


def parse_failure_outputs(output):
    """Map `(target, fullName)` to the captured output libtest prints for
    each failing test.

    libtest emits, after the per-target run, a block of the shape::

        ---- some::test stdout ----
        thread 'some::test' panicked at src/lib.rs:10:5:
        assertion `left == right` failed

    followed by a `failures:` roll-call. We capture the block bodies so
    `compare` can show *why* a test flipped to `failed` without the
    developer re-running cargo by hand.
    """
    captured = {}
    current_target = "unknown"
    current_key = None
    body = []

    def flush():
        if current_key is not None:
            captured[current_key] = "\n".join(body).strip("\n")

    for raw_line in output.splitlines():
        line = strip_ansi(raw_line)
        target = parse_running_target(line)
        if target:
            flush()
            current_key = None
            body = []
            current_target = target
            continue

        match = FAILURE_BLOCK_RE.match(line.strip())
        if match:
            flush()
            current_key = (current_target, match.group("name"))
            body = []
            continue

        if current_key is None:
            continue

        if FAILURE_LIST_RE.match(line.strip()) or TEST_STATUS_RE.match(line.strip()):
            flush()
            current_key = None
            body = []
            continue

        body.append(line.rstrip())

    flush()
    return {key: text for key, text in captured.items() if text}


def bounded_excerpt(text, limit=FAILURE_EXCERPT_LINE_LIMIT):
    """Trim `text` to at most `limit` lines, keeping both ends.

    Head and tail both matter: a Rust panic message lands at the end of
    the captured block, a Dart failure message at the start.
    """
    lines = text.splitlines()
    if len(lines) <= limit:
        return text
    head = limit // 2
    tail = limit - head
    return "\n".join(lines[:head] + [EXCERPT_ELISION] + lines[-tail:])


def output_is_parseable(returncode, output, tests):
    if returncode == 0:
        return True
    if any(test["status"] == "failed" for test in tests):
        return True
    if RESULT_FAILED_RE.search(strip_ansi(output)):
        return True
    return False


def format_missing(names):
    shown = names[:20]
    text = ", ".join(shown)
    if len(names) > len(shown):
        text += ", ... (" + str(len(names)) + " total)"
    return text


def merge_discovered_tests(suite_name, discovered_names, status_tests):
    if suite_name == "docs":
        return status_tests

    status_names = {test["fullName"] for test in status_tests}
    if not discovered_names and status_names:
        raise BaselineError(suite_name + " discovery found no tests, but actual output had test statuses")

    missing = sorted(set(discovered_names) - status_names)
    if missing:
        raise BaselineError(
            suite_name
            + " discovery listed tests missing from actual libtest output: "
            + format_missing(missing)
        )

    discovery_order = {}
    for name in discovered_names:
        if name not in discovery_order:
            discovery_order[name] = len(discovery_order)
    fallback_order = len(discovery_order)
    return sorted(
        status_tests,
        key=lambda test: (discovery_order.get(test["fullName"], fallback_order), test["target"], test["fullName"]),
    )


def save_cargo_failure(root, suite_name, phase, completed):
    base = root / "target/test-baseline" / (suite_name + "-" + phase)
    log_path = Path(str(base) + ".log")
    exit_path = Path(str(base) + ".exit-code")
    write_text_atomic(log_path, completed.stdout)
    write_text_atomic(exit_path, str(completed.returncode) + "\n")
    return "\nraw output: " + str(log_path) + "; exit code: " + str(exit_path)


def run_suite(root, suite_name, argv, tolerate_failures=False):
    """Run one suite and return `(suite, failure_outputs)`.

    `tolerate_failures` splits the two callers apart: `write` must refuse
    to freeze a red tree into the baseline, while `compare` wants the red
    tree *described* (which tests failed, and why) instead of an opaque
    exit code. A run that is not parseable at all (compile error, crashed
    harness) still raises in both modes.
    """
    if suite_name == FLUTTER_SUITE_NAME:
        return run_flutter_suite(root, suite_name, argv, tolerate_failures)

    list_argv = discovery_command(argv)
    print("discovering " + suite_name + ": " + command_text(list_argv), file=sys.stderr)
    list_completed = subprocess.run(
        list_argv,
        cwd=str(root),
        env=hermetic_env(),
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        check=False,
    )
    discovered_names = parse_test_list(list_completed.stdout)
    if list_completed.returncode != 0:
        if suite_name == "docs":
            discovered_names = []
        else:
            raise BaselineError(
                suite_name
                + " cargo test discovery failed; existing baseline was not changed\n"
                + last_lines(list_completed.stdout, ERROR_OUTPUT_LINE_LIMIT)
                + save_cargo_failure(root, suite_name, "discovery", list_completed)
            )

    print("running " + suite_name + ": " + command_text(argv), file=sys.stderr)
    completed = subprocess.run(
        argv,
        cwd=str(root),
        env=hermetic_env(),
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        check=False,
    )
    try:
        tests = parse_libtest_output(completed.stdout)
        if not output_is_parseable(completed.returncode, completed.stdout, tests):
            raise BaselineError(
                suite_name
                + " cargo output was not parseable as libtest results; existing baseline was not changed\n"
                + last_lines(completed.stdout, ERROR_OUTPUT_LINE_LIMIT)
            )
        if completed.returncode != 0 and not tolerate_failures:
            raise BaselineError(
                suite_name
                + " cargo test failed with exit code "
                + str(completed.returncode)
                + "; existing baseline was not changed\n"
                + last_lines(completed.stdout, ERROR_OUTPUT_LINE_LIMIT)
            )
        tests = merge_discovered_tests(suite_name, discovered_names, tests)
    except BaselineError as error:
        raise BaselineError(
            str(error) + save_cargo_failure(root, suite_name, "run", completed)
        ) from error
    failure_outputs = {
        (suite_name,) + key: text
        for key, text in parse_failure_outputs(completed.stdout).items()
    }
    return make_suite(argv, tests), failure_outputs


def run_flutter_suite(root, suite_name, argv, tolerate_failures=False):
    """Run `flutter test --reporter=json` and parse its JSONL stream.

    Flutter has no `-- --list` discovery analog; the JSON report doubles
    as both discovery + result feed because every `testStart` is followed
    by a `testDone`.

    A non-zero exit refuses to write the baseline, exactly like the cargo
    path (`run_suite`). The `and failed_test_names(tests)` this condition
    used to carry was the hole: a load/compile error in ONE test file
    exits non-zero while every *recorded* test passed, so the run looked
    green and the whole unloadable file was written into the baseline as
    if those tests had never existed. `tolerate_failures` (compare mode
    only) is the single deliberate opt-out.
    """
    cwd = root / FLUTTER_SUITE_CWD
    print("running " + suite_name + ": " + command_text(argv) + " (cwd=" + str(cwd) + ")", file=sys.stderr)
    completed = subprocess.run(
        argv,
        cwd=str(cwd),
        env=hermetic_env(),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    try:
        tests = parse_flutter_json(completed.stdout)
    except BaselineError as error:
        raise BaselineError(
            suite_name
            + " flutter test output was not parseable as JSON; existing baseline was not changed: "
            + str(error)
            + "\n"
            + last_lines(completed.stdout, ERROR_OUTPUT_LINE_LIMIT)
        )
    if not tests and completed.returncode != 0:
        raise BaselineError(
            suite_name
            + " flutter test produced no parseable tests and exited non-zero; existing baseline was not changed\n"
            + last_lines(completed.stderr or completed.stdout, ERROR_OUTPUT_LINE_LIMIT)
        )
    if completed.returncode != 0 and not tolerate_failures:
        failed = failed_test_names(tests)
        detail = (
            format_missing(failed)
            if failed
            else last_lines(completed.stderr or completed.stdout, ERROR_OUTPUT_LINE_LIMIT)
        )
        raise BaselineError(
            suite_name
            + " flutter test failed with exit code "
            + str(completed.returncode)
            + "; existing baseline was not changed\n"
            + detail
        )
    failure_outputs = {
        (suite_name,) + key: text
        for key, text in parse_flutter_failure_outputs(completed.stdout).items()
    }
    return make_suite(argv, tests), failure_outputs


def failed_test_names(tests):
    return sorted(test["fullName"] for test in tests if test["status"] == "failed")


def parse_flutter_failure_outputs(output):
    """Map `(target, fullName)` to the `error` event text of failing tests.

    The JSON reporter emits `{"type": "error", "testID": .., "error": ..,
    "stackTrace": ..}` for every failure, separate from the `testDone`
    that carries the result — so we replay the stream a second time and
    join both on `testID`.
    """
    groups = {}
    suites = {}
    tests_by_id = {}
    errors_by_id = {}
    for line in output.splitlines():
        line = line.strip()
        if not line or not line.startswith("{"):
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        event_type = event.get("type")
        if event_type == "suite":
            suite = event.get("suite", {})
            suites[suite.get("id")] = suite.get("path", "")
        elif event_type == "group":
            group = event.get("group", {})
            groups[group.get("id")] = group
        elif event_type == "testStart":
            test = event.get("test", {})
            tests_by_id[test.get("id")] = test
        elif event_type == "error":
            spec = tests_by_id.get(event.get("testID"))
            if spec is None or is_flutter_synthetic_test(spec):
                continue
            text = "\n".join(
                part
                for part in (event.get("error"), event.get("stackTrace"))
                if part
            ).strip()
            if not text:
                continue
            entry = flutter_test_entry(spec, {}, groups, suites)
            key = (entry["target"], entry["fullName"])
            errors_by_id[key] = "\n".join(
                part for part in (errors_by_id.get(key), text) if part
            )
    return errors_by_id


def parse_flutter_json(output):
    """Parse the `flutter test --reporter=json` JSONL stream.

    Spec: every `testStart` event introduces a test (with `id`, `name`,
    `groupIDs`, `url`). Every `testDone` event reports the result by
    `testID`. Group names are accumulated from `group` events so we
    can recover the leaf className.

    Tests with no groupIDs whose name starts with "loading " are
    framework synthetic entries (one per file load); they are
    intentionally filtered out so the baseline tracks only user tests.
    """
    groups = {}
    suites = {}
    tests_by_id = {}
    results = []
    for line in output.splitlines():
        line = line.strip()
        if not line or not line.startswith("{"):
            continue
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        event_type = event.get("type")
        if event_type == "suite":
            suite = event.get("suite", {})
            suites[suite.get("id")] = suite.get("path", "")
        elif event_type == "group":
            group = event.get("group", {})
            groups[group.get("id")] = group
        elif event_type == "testStart":
            test = event.get("test", {})
            tests_by_id[test.get("id")] = test
        elif event_type == "testDone":
            spec = tests_by_id.get(event.get("testID"))
            if spec is None:
                continue
            if is_flutter_synthetic_test(spec):
                continue
            results.append(flutter_test_entry(spec, event, groups, suites))
    results.sort(key=lambda item: (item["target"], item["fullName"]))
    return results


def is_flutter_synthetic_test(spec):
    name = spec.get("name", "")
    group_ids = spec.get("groupIDs") or []
    # `loading <path>` tests are emitted once per suite file and carry
    # no group; they're noise from the test harness.
    if not group_ids and name.startswith("loading "):
        return True
    return False


def flutter_test_entry(spec, done_event, groups, suites):
    name = spec.get("name", "")
    group_ids = spec.get("groupIDs") or []
    leaf_group = ""
    for gid in reversed(group_ids):
        group = groups.get(gid) or {}
        group_name = group.get("name") or ""
        if group_name:
            leaf_group = group_name
            break
    target = flutter_target_for(spec, suites)
    class_name = leaf_group or target
    if leaf_group and name.startswith(leaf_group + " "):
        method_name = name[len(leaf_group) + 1 :]
    else:
        method_name = name
    full_name = class_name + "::" + method_name if leaf_group else name
    return {
        "className": class_name,
        "methodName": method_name,
        "status": flutter_status_name(done_event),
        "target": target,
        "fullName": full_name,
    }


def flutter_target_for(spec, suites):
    url = spec.get("url") or ""
    if url.startswith("file://"):
        url = url[len("file://") :]
    if not url:
        suite_id = spec.get("suiteID")
        url = suites.get(suite_id, "") if suite_id is not None else ""
    return "flutter::" + flutter_relative_path(url)


def flutter_relative_path(path_text):
    if not path_text:
        return "unknown"
    marker = "/flutter_app/"
    idx = path_text.rfind(marker)
    if idx != -1:
        return path_text[idx + 1 :]
    # Fallback: drop everything before the last path segment to keep
    # the baseline path-stable across cwd changes.
    return Path(path_text).name


def flutter_status_name(done_event):
    if done_event.get("skipped"):
        return "ignored"
    result = done_event.get("result")
    if result == "success":
        return "passed"
    return "failed"


def make_suite(argv, tests):
    return {
        "task": command_text(argv),
        "tasks": list(argv),
        "total": len(tests),
        "tests": tests,
    }


def collect_current(root, tolerate_failures=False):
    """Run every suite once; return `(document, failure_outputs)`."""
    suites = {}
    failure_outputs = {}
    for suite_name, argv in SUITES.items():
        suite, suite_failures = run_suite(root, suite_name, argv, tolerate_failures)
        suites[suite_name] = suite
        failure_outputs.update(suite_failures)
    return make_document(suites), failure_outputs


def make_document(suites):
    return {
        "version": VERSION,
        "generatedAt": utc_rfc3339z(),
        "project": PROJECT,
        "suites": suites,
    }


def write_json_atomic(path, payload):
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = Path(str(path) + ".tmp")
    tmp_path.write_text(json.dumps(payload, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    os.replace(str(tmp_path), str(path))


def write_text_atomic(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = Path(str(path) + ".tmp")
    tmp_path.write_text(text, encoding="utf-8")
    os.replace(str(tmp_path), str(path))


def output_path(root, path_text):
    path = Path(path_text)
    if path.is_absolute():
        return path
    return root / path


def read_json(path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise BaselineError(str(path) + " is not valid JSON: " + str(error))


def validate_document(document, source, require_exact_suites=True):
    if not isinstance(document, dict):
        raise BaselineError(source + " must be a JSON object")
    if document.get("version") != VERSION:
        raise BaselineError(source + " must have version " + str(VERSION))
    if document.get("project") != PROJECT:
        raise BaselineError(source + " must have project " + PROJECT)

    suites = document.get("suites")
    if not isinstance(suites, dict):
        raise BaselineError(source + " must have suites object")
    if require_exact_suites:
        expected = set(SUITES.keys())
        actual = set(suites.keys())
        if actual != expected:
            raise BaselineError(
                source
                + " suites must be exactly "
                + ", ".join(sorted(expected))
                + "; got "
                + ", ".join(sorted(actual))
            )

    for suite_name, suite in suites.items():
        suite_source = source + " suite " + suite_name
        if not isinstance(suite, dict):
            raise BaselineError(suite_source + " must be an object")
        tests = suite.get("tests")
        if not isinstance(tests, list):
            raise BaselineError(suite_source + " must have tests list")
        total = suite.get("total")
        if type(total) is not int or total != len(tests):
            raise BaselineError(suite_source + " total must equal len(tests)")
        for index, test in enumerate(tests):
            test_source = suite_source + " test " + str(index)
            if not isinstance(test, dict):
                raise BaselineError(test_source + " must be an object")
            for field in REQUIRED_TEST_FIELDS:
                if field not in test:
                    raise BaselineError(test_source + " missing " + field)
            if test["status"] not in VALID_STATUSES:
                raise BaselineError(test_source + " has invalid status " + repr(test["status"]))


def test_key(suite_name, test):
    return (suite_name, test["target"], test["fullName"])


def with_suite(suite_name, test):
    item = {"suite": suite_name}
    item.update(test)
    return item


def index_tests(document):
    indexed = {}
    for suite_name, suite in document.get("suites", {}).items():
        for test in suite.get("tests", []):
            indexed[test_key(suite_name, test)] = with_suite(suite_name, test)
    return indexed


def sort_items(items):
    return sorted(items, key=lambda item: (item["suite"], item["target"], item["fullName"]))


def compute_diff(baseline, current):
    before = index_tests(baseline)
    after = index_tests(current)

    added = [after[key] for key in after.keys() - before.keys()]
    removed = [before[key] for key in before.keys() - after.keys()]
    status_changed = []
    changed = []
    for key in before.keys() & after.keys():
        old = before[key]
        new = after[key]
        if old["status"] != new["status"]:
            status_changed.append(
                {
                    "suite": new["suite"],
                    "target": new["target"],
                    "fullName": new["fullName"],
                    "className": new["className"],
                    "methodName": new["methodName"],
                    "baselineStatus": old["status"],
                    "currentStatus": new["status"],
                }
            )
        field_changes = diff_test_fields(old, new, ("className", "methodName"))
        if field_changes:
            changed.append(
                {
                    "suite": new["suite"],
                    "target": new["target"],
                    "fullName": new["fullName"],
                    "changes": field_changes,
                }
            )

    return {
        "version": VERSION,
        "generatedAt": utc_rfc3339z(),
        "project": PROJECT,
        "added": sort_items(added),
        "removed": sort_items(removed),
        "statusChanged": sort_items(status_changed),
        "changed": sort_items(changed),
    }


def diff_test_fields(old, new, fields):
    changes = []
    for field in fields:
        if old.get(field) != new.get(field):
            changes.append(
                {
                    "field": field,
                    "baseline": old.get(field),
                    "current": new.get(field),
                }
            )
    return changes


def has_drift(diff):
    return bool(diff["added"] or diff["removed"] or diff["statusChanged"] or diff.get("changed", []))


def drift_summary(diff):
    return (
        "added="
        + str(len(diff["added"]))
        + " removed="
        + str(len(diff["removed"]))
        + " statusChanged="
        + str(len(diff["statusChanged"]))
        + " changed="
        + str(len(diff.get("changed", [])))
    )


def print_test_item(prefix, item):
    print(
        prefix
        + " suite="
        + item["suite"]
        + " target="
        + item["target"]
        + " fullName="
        + item["fullName"],
        file=sys.stderr,
    )


def print_status_changed(item):
    print_test_item(
        "  [statusChanged] " + item["baselineStatus"] + " -> " + item["currentStatus"],
        item,
    )


def print_changed_fields(item):
    fields = ", ".join(
        change["field"] + "=" + str(change["baseline"]) + " -> " + str(change["current"])
        for change in item["changes"]
    )
    print_test_item("  [changed] " + fields, item)


def print_drift_details(diff):
    for item in diff["added"]:
        print_test_item("  [added] status=" + item["status"], item)
    for item in diff["removed"]:
        print_test_item("  [removed] status=" + item["status"], item)
    for item in diff["statusChanged"]:
        print_status_changed(item)
    for item in diff.get("changed", []):
        print_changed_fields(item)


def validate_diff_item(item, source, status_field):
    if not isinstance(item, dict):
        raise BaselineError(source + " must be an object")
    for field in ("suite", "target", "fullName", "className", "methodName", status_field):
        if field not in item:
            raise BaselineError(source + " missing " + field)


def validate_diff(diff, source):
    if not isinstance(diff, dict):
        raise BaselineError(source + " must be a JSON object")
    if diff.get("version") != VERSION:
        raise BaselineError(source + " must have version " + str(VERSION))
    if diff.get("project") != PROJECT:
        raise BaselineError(source + " must have project " + PROJECT)
    for key in ("added", "removed", "statusChanged"):
        if not isinstance(diff.get(key), list):
            raise BaselineError(source + " must have " + key + " list")
    if not isinstance(diff.get("changed", []), list):
        raise BaselineError(source + " must have changed list")
    for index, item in enumerate(diff["added"]):
        validate_diff_item(item, source + " added " + str(index), "status")
    for index, item in enumerate(diff["removed"]):
        validate_diff_item(item, source + " removed " + str(index), "status")
    for index, item in enumerate(diff["statusChanged"]):
        validate_diff_item(item, source + " statusChanged " + str(index), "baselineStatus")
        if "currentStatus" not in item:
            raise BaselineError(source + " statusChanged " + str(index) + " missing currentStatus")
    for index, item in enumerate(diff.get("changed", [])):
        validate_changed_item(item, source + " changed " + str(index))


def validate_changed_item(item, source):
    if not isinstance(item, dict):
        raise BaselineError(source + " must be an object")
    for field in ("suite", "target", "fullName", "changes"):
        if field not in item:
            raise BaselineError(source + " missing " + field)
    if not isinstance(item["changes"], list):
        raise BaselineError(source + " changes must be a list")
    for index, change in enumerate(item["changes"]):
        change_source = source + " change " + str(index)
        if not isinstance(change, dict):
            raise BaselineError(change_source + " must be an object")
        for field in ("field", "baseline", "current"):
            if field not in change:
                raise BaselineError(change_source + " missing " + field)


def markdown_cell(value):
    return str(value).replace("|", "\\|").replace("\n", "<br>")


def markdown_inline_code(value):
    return str(value).replace("`", "\\`")


def markdown_code(value):
    return "`" + markdown_inline_code(markdown_cell(value)) + "`"


def append_comment_blocks(lines, blocks, footer, omitted_line, item_limit, body_limit):
    omitted = 0
    for index, block in enumerate(blocks):
        if item_limit is not None and index >= item_limit:
            omitted += len(blocks) - index
            break
        candidate = lines + block + footer
        if body_limit is not None and len("\n".join(candidate) + "\n") + COMMENT_OMITTED_ROW_BUDGET > body_limit:
            omitted += 1
        else:
            lines.extend(block)
    if omitted:
        lines.extend(["", omitted_line(omitted)])


def append_comment_section(lines, title, blocks, footer, omitted_label, item_limit, body_limit):
    if not blocks:
        return

    lines.extend(["", "#### " + title + " (" + str(len(blocks)) + ")", ""])
    append_comment_blocks(
        lines,
        blocks,
        footer,
        lambda omitted: "_Omitted `"
        + str(omitted)
        + "` additional "
        + omitted_label
        + "._",
        item_limit,
        body_limit,
    )


def test_context_lines(item):
    return [
        "  - Suite: " + markdown_code(item["suite"]),
        "  - Target: " + markdown_code(item["target"]),
    ]


def field_label(field):
    labels = {"className": "Class", "methodName": "Method"}
    return labels.get(field, field)


def status_change_block(item):
    return [
        "- " + markdown_code(item["fullName"]),
        "  - Status: " + markdown_code(item["baselineStatus"]) + " -> " + markdown_code(item["currentStatus"]),
    ] + test_context_lines(item)


def field_change_block(item):
    block = ["- " + markdown_code(item["fullName"])]
    for change in item["changes"]:
        block.append(
            "  - "
            + markdown_cell(field_label(change["field"]))
            + ": "
            + markdown_code(change["baseline"])
            + " -> "
            + markdown_code(change["current"])
        )
    return block + test_context_lines(item)


def presence_block(item, status_label):
    return [
        "- " + markdown_code(item["fullName"]),
        "  - " + status_label + ": " + markdown_code(item["status"]),
        "  - Class: " + markdown_code(item["className"]),
        "  - Method: " + markdown_code(item["methodName"]),
    ] + test_context_lines(item)


def append_changed_sections(lines, diff, footer, item_limit, body_limit):
    blocks = []
    for item in diff["statusChanged"]:
        blocks.append(status_change_block(item))
    for item in diff.get("changed", []):
        blocks.append(field_change_block(item))
    append_comment_section(lines, "Changed", blocks, footer, "changed tests", item_limit, body_limit)


def append_added_section(lines, diff, footer, item_limit, body_limit):
    append_comment_section(
        lines,
        "Added",
        [presence_block(item, "Status") for item in diff["added"]],
        footer,
        "added tests",
        item_limit,
        body_limit,
    )


def append_removed_section(lines, diff, footer, item_limit, body_limit):
    append_comment_section(
        lines,
        "Removed",
        [presence_block(item, "Previous status") for item in diff["removed"]],
        footer,
        "removed tests",
        item_limit,
        body_limit,
    )


def append_diff_sections(lines, diff, footer, item_limit, body_limit):
    append_changed_sections(lines, diff, footer, item_limit, body_limit)
    append_added_section(lines, diff, footer, item_limit, body_limit)
    append_removed_section(lines, diff, footer, item_limit, body_limit)


def format_pr_comment_with_limits(diff, item_limit, body_limit):
    lines = [COMMENT_MARKER, "### Test baseline", ""]
    if not has_drift(diff):
        lines.append("No test baseline drift detected by `just test-baseline-check`.")
        return "\n".join(lines) + "\n"

    footer = [
        "",
        "If this is intentional, run `just test-baseline` locally and commit `fixtures/test-baseline.json`.",
    ]
    append_diff_sections(lines, diff, footer, item_limit, body_limit)
    lines.extend(footer)
    return "\n".join(lines) + "\n"


def format_pr_comment(diff):
    return format_pr_comment_with_limits(diff, COMMENT_SECTION_ITEM_LIMIT, COMMENT_BODY_LIMIT)


def format_pr_full_comment(diff):
    return format_pr_comment_with_limits(diff, None, None)


def format_fixture_pr_comment_with_limits(diff, base_ref, head_ref, base_missing, head_missing, item_limit, body_limit):
    base_label = markdown_inline_code(base_ref or "target branch")
    head_label = markdown_inline_code(head_ref or "this PR")
    lines = [
        COMMENT_MARKER,
        "### Test Baseline Fixture Changes",
        "",
        "Comparing committed `fixtures/test-baseline.json` from target branch `"
        + base_label
        + "` with this PR `"
        + head_label
        + "`.",
        "",
    ]
    if base_missing and head_missing:
        lines.append("Neither side contains `fixtures/test-baseline.json`; no fixture comparison is available.")
        lines.append("")
    elif base_missing:
        lines.append("The target branch does not contain `fixtures/test-baseline.json`; this PR's fixture is shown as newly added.")
        lines.append("")
    elif head_missing:
        lines.append("This PR does not contain `fixtures/test-baseline.json`; the target branch fixture is shown as removed.")
        lines.append("")

    if not has_drift(diff):
        lines.append("No test baseline fixture changes compared with the target branch.")
        lines.append("")
        return "\n".join(lines) + "\n"

    footer = [
        "",
        "This compares committed fixture files only. CI separately verifies generated test output against this PR's fixture. The full Markdown report and JSON diff are uploaded as the `pr-fixture-diffs` workflow artifact.",
    ]
    append_diff_sections(lines, diff, footer, item_limit, body_limit)
    lines.extend(footer)
    return "\n".join(lines) + "\n"


def format_fixture_pr_comment(diff, base_ref, head_ref, base_missing=False, head_missing=False):
    return format_fixture_pr_comment_with_limits(
        diff,
        base_ref,
        head_ref,
        base_missing,
        head_missing,
        COMMENT_SECTION_ITEM_LIMIT,
        COMMENT_BODY_LIMIT,
    )


def format_fixture_pr_full_comment(diff, base_ref, head_ref, base_missing=False, head_missing=False):
    return format_fixture_pr_comment_with_limits(diff, base_ref, head_ref, base_missing, head_missing, None, None)


def last_lines(text, limit):
    lines = text.splitlines()
    if len(lines) <= limit:
        return text
    return "\n".join(lines[-limit:])


def cmd_write(args):
    root = repo_root()
    document, _ = collect_current(root)
    validate_document(document, "current")
    if args.dry_run:
        print(json.dumps(document, indent=2, ensure_ascii=False))
        return 0

    baseline_path = output_path(root, args.output)
    write_json_atomic(baseline_path, document)
    print("wrote " + str(baseline_path) + " with " + str(total_tests(document)) + " tests")
    return 0


def cmd_compare(args):
    root = repo_root()
    baseline_path = output_path(root, args.baseline)
    if not baseline_path.exists():
        print("missing baseline: " + str(baseline_path) + "; run `just test-baseline` first", file=sys.stderr)
        return 2

    baseline = read_json(baseline_path)
    validate_document(baseline, str(baseline_path))
    current, failure_outputs = collect_current(root, tolerate_failures=True)
    validate_document(current, "current")
    current_path = output_path(root, args.current_output)
    write_json_atomic(current_path, current)
    diff = compute_diff(baseline, current)
    diff_path = output_path(root, args.diff_output)
    write_json_atomic(diff_path, diff)
    print("wrote " + str(current_path))
    print("wrote " + str(diff_path))
    failed = failed_tests(current)
    if has_drift(diff):
        print("drift: " + drift_summary(diff), file=sys.stderr)
        print_drift_details(diff)
        print_failure_outputs(diff, failure_outputs)
        return 1
    if failed:
        # The committed baseline can never record a failing test (`write`
        # refuses a red tree), so this only fires on a hand-edited fixture.
        print("failing tests with no baseline drift:", file=sys.stderr)
        for item in failed:
            print_test_item("  [failed]", item)
        print_failure_items(failed, failure_outputs)
        return 1
    print("no test baseline drift")
    return 0


def failed_tests(document):
    return sort_items(
        [item for item in index_tests(document).values() if item["status"] == "failed"]
    )


def failure_key(item):
    return (item["suite"], item["target"], item["fullName"])


def print_failure_items(items, failure_outputs):
    """Print each item's captured output, bounded, so a red `compare` run
    is self-contained — no second plain `cargo test` needed to read the
    panic message."""
    for item in items:
        captured = failure_outputs.get(failure_key(item))
        print("  [output] " + item["fullName"], file=sys.stderr)
        if not captured:
            print("    (no captured output)", file=sys.stderr)
            continue
        for line in bounded_excerpt(captured).splitlines():
            print("    " + line, file=sys.stderr)


def print_failure_outputs(diff, failure_outputs):
    newly_failed = [
        item for item in diff["statusChanged"] if item["currentStatus"] == "failed"
    ]
    newly_failed += [item for item in diff["added"] if item["status"] == "failed"]
    if not newly_failed:
        return
    print("failing tests (" + str(len(newly_failed)) + "):", file=sys.stderr)
    print_failure_items(sort_items(newly_failed), failure_outputs)


def cmd_pr_comment(args):
    root = repo_root()
    diff_path = output_path(root, args.diff)
    diff = read_json(diff_path)
    validate_diff(diff, str(diff_path))
    body = format_pr_comment(diff)
    full_body = format_pr_full_comment(diff)
    if args.output:
        output = output_path(root, args.output)
        write_text_atomic(output, body)
        print("wrote " + str(output))
    else:
        print(body, end="")
    if args.full_output:
        full_output = output_path(root, args.full_output)
        write_text_atomic(full_output, full_body)
        print("wrote " + str(full_output))
    return 0


def cmd_fixture_pr_comment(args):
    root = repo_root()
    base_path = output_path(root, args.base)
    head_path = output_path(root, args.head)
    base, base_missing = read_optional_fixture_document(base_path)
    head, head_missing = read_optional_fixture_document(head_path)
    diff = compute_diff(base, head)
    if args.diff_output:
        diff_path = output_path(root, args.diff_output)
        write_json_atomic(diff_path, diff)
        print("wrote " + str(diff_path))
    body = format_fixture_pr_comment(diff, args.base_ref, args.head_ref, base_missing, head_missing)
    full_body = format_fixture_pr_full_comment(diff, args.base_ref, args.head_ref, base_missing, head_missing)
    if args.output:
        output = output_path(root, args.output)
        write_text_atomic(output, body)
        print("wrote " + str(output))
    else:
        print(body, end="")
    if args.full_output:
        full_output = output_path(root, args.full_output)
        write_text_atomic(full_output, full_body)
        print("wrote " + str(full_output))
    return 0


def empty_fixture_document():
    return make_document(
        {suite_name: make_suite(argv, []) for suite_name, argv in SUITES.items()}
    )


def read_optional_fixture_document(path):
    if not path.exists():
        return empty_fixture_document(), True
    document = read_json(path)
    validate_document(document, str(path), require_exact_suites=False)
    return document, False


def total_tests(document):
    return sum(suite.get("total", 0) for suite in document.get("suites", {}).values())


def assert_equal(label, actual, expected):
    if actual != expected:
        raise BaselineError(label + " expected " + repr(expected) + " but got " + repr(actual))


def assert_raises(label, expected_text, func):
    try:
        func()
    except BaselineError as error:
        if expected_text not in str(error):
            raise BaselineError(label + " raised wrong error: " + str(error))
        return
    raise BaselineError(label + " expected BaselineError")


def cmd_self_test(args):
    del args
    baseline_list = """
fixture::kept: test
fixture::removed: test
fixture::changed: test
"""
    current_list = """
fixture::kept: test
fixture::added: test
fixture::changed: test
"""
    assert_equal(
        "list removed",
        sorted(set(parse_test_list(baseline_list)) - set(parse_test_list(current_list))),
        ["fixture::removed"],
    )
    assert_equal(
        "list added",
        sorted(set(parse_test_list(current_list)) - set(parse_test_list(baseline_list))),
        ["fixture::added"],
    )

    schema_test = {
        "className": "fixture",
        "methodName": "case",
        "status": "passed",
        "target": "target",
        "fullName": "fixture::case",
    }
    partial_document = make_document({"workspace": make_suite(SUITES["workspace"], [schema_test])})
    validate_document(partial_document, "partial self-test", require_exact_suites=False)
    assert_raises(
        "exact suite validation",
        "suites must be exactly",
        lambda: validate_document(partial_document, "partial self-test"),
    )
    bad_status = make_document(
        {"workspace": make_suite(SUITES["workspace"], [dict(schema_test, status="skipped")])}
    )
    assert_raises(
        "status validation",
        "invalid status",
        lambda: validate_document(bad_status, "bad status", require_exact_suites=False),
    )
    bad_total = make_document({"workspace": make_suite(SUITES["workspace"], [schema_test])})
    bad_total["suites"]["workspace"]["total"] = 2
    assert_raises(
        "total validation",
        "total must equal len(tests)",
        lambda: validate_document(bad_total, "bad total", require_exact_suites=False),
    )

    baseline_output = """
     Running unittests src/lib.rs (target/debug/deps/upeg_core-1111111111111111)
running 4 tests
test fixture::kept ... ok
test fixture::removed ... ok
test fixture::changed ... ok
test fixture::ignored_case ... ignored
test result: ok. 3 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
"""
    current_output = """
     Running unittests src/lib.rs (target/debug/deps/upeg_core-1111111111111111)
running 4 tests
test fixture::kept ... ok
test fixture::added ... ok
test fixture::changed ... FAILED
test fixture::ignored_case ... ignored
test result: FAILED. 2 passed; 1 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s
"""
    baseline = make_document({"workspace": make_suite(SUITES["workspace"], parse_libtest_output(baseline_output))})
    current = make_document({"workspace": make_suite(SUITES["workspace"], parse_libtest_output(current_output))})
    validate_document(baseline, "baseline self-test", require_exact_suites=False)
    validate_document(current, "current self-test", require_exact_suites=False)
    diff = compute_diff(baseline, current)

    assert_equal("added count", len(diff["added"]), 1)
    assert_equal("removed count", len(diff["removed"]), 1)
    assert_equal("status changed count", len(diff["statusChanged"]), 1)
    assert_equal("added name", diff["added"][0]["fullName"], "fixture::added")
    assert_equal("removed name", diff["removed"][0]["fullName"], "fixture::removed")
    assert_equal("baseline status", diff["statusChanged"][0]["baselineStatus"], "passed")
    assert_equal("current status", diff["statusChanged"][0]["currentStatus"], "failed")
    assert_equal("ignored unchanged", [item["fullName"] for item in diff["statusChanged"]], ["fixture::changed"])
    validate_diff(diff, "diff self-test")
    comment = format_pr_comment(diff)
    assert_equal("comment marker", COMMENT_MARKER in comment, True)
    assert_equal("comment changed section", "#### Changed (1)" in comment, True)
    assert_equal("comment added section", "#### Added (1)" in comment, True)
    assert_equal("comment removed section", "#### Removed (1)" in comment, True)
    assert_equal("comment added", "fixture::added" in comment, True)
    assert_equal("comment avoids wide tables", "| Change | Suite | Target | Test | Class | Method | Status |" in comment, False)
    assert_equal("comment status change", "`passed` -> `failed`" in comment, True)
    metadata_current = json.loads(json.dumps(baseline))
    metadata_current["suites"]["workspace"]["tests"][0]["className"] = "fixture_renamed"
    metadata_diff = compute_diff(baseline, metadata_current)
    assert_equal("metadata changed count", len(metadata_diff["changed"]), 1)
    assert_equal("metadata changed field", metadata_diff["changed"][0]["changes"][0]["field"], "className")
    metadata_comment = format_fixture_pr_comment(metadata_diff, "main", "feature/test")
    assert_equal("metadata comment section", "#### Changed (1)" in metadata_comment, True)
    assert_equal("metadata comment field", "Class: `fixture` -> `fixture_renamed`" in metadata_comment, True)
    fixture_comment = format_fixture_pr_comment(diff, "main", "feature/test")
    assert_equal("fixture comment title", "Fixture Changes" in fixture_comment, True)
    assert_equal("fixture comment base ref", "target branch `main`" in fixture_comment, True)
    assert_equal("fixture comment status change", "`passed` -> `failed`" in fixture_comment, True)
    no_fixture_comment = format_fixture_pr_comment(compute_diff(baseline, baseline), "main", "feature/test")
    assert_equal("fixture no drift", "No test baseline fixture changes" in no_fixture_comment, True)
    legacy_base = make_document(
        {
            "workspace": make_suite(SUITES["workspace"], parse_libtest_output(baseline_output)),
            "docs": make_suite(SUITES["docs"], []),
            "wasmPlugin": make_suite(RETIRED_SUITES["wasmPlugin"], []),
        }
    )
    head_with_flutter = make_document(
        {
            "workspace": legacy_base["suites"]["workspace"],
            "docs": legacy_base["suites"]["docs"],
            "wasmPlugin": legacy_base["suites"]["wasmPlugin"],
            "flutter": make_suite(
                SUITES["flutter"],
                [
                    dict(
                        schema_test,
                        className="App",
                        methodName="renders_board",
                        target="flutter_app/test/widget_tests/app_test.dart",
                        fullName="App::renders_board",
                    )
                ],
            ),
        }
    )
    with tempfile.TemporaryDirectory() as temp_dir:
        base_path = Path(temp_dir) / "base.json"
        head_path = Path(temp_dir) / "head.json"
        write_json_atomic(base_path, legacy_base)
        write_json_atomic(head_path, head_with_flutter)
        legacy_base_read, legacy_base_missing = read_optional_fixture_document(base_path)
        legacy_head_read, legacy_head_missing = read_optional_fixture_document(head_path)
    assert_equal("retired suite is no longer run", "wasmPlugin" in SUITES, False)
    assert_equal("legacy fixture base present", legacy_base_missing, False)
    assert_equal("legacy fixture head present", legacy_head_missing, False)
    legacy_diff = compute_diff(legacy_base_read, legacy_head_read)
    legacy_flutter_added = [item for item in legacy_diff["added"] if item["suite"] == FLUTTER_SUITE_NAME]
    assert_equal("legacy fixture flutter added count", len(legacy_flutter_added), 1)
    legacy_comment = format_fixture_pr_comment(legacy_diff, "main", "feature/flutter")
    assert_equal("legacy fixture comment renders", "#### Added (1)" in legacy_comment, True)
    missing_base_comment = format_fixture_pr_comment(diff, "main", "feature/test", True, False)
    assert_equal("fixture missing base", "target branch does not contain" in missing_base_comment, True)
    empty_document = empty_fixture_document()
    validate_document(empty_document, "empty fixture self-test")
    assert_equal("empty fixture suites", sorted(empty_document["suites"].keys()), sorted(SUITES.keys()))

    large_diff = {
        "added": [dict(diff["added"][0], fullName="fixture::added_" + str(index).zfill(5)) for index in range(2000)],
        "removed": [],
        "statusChanged": [],
        "changed": [],
    }
    large_comment = format_pr_comment(large_diff)
    assert_equal("large comment bounded", len(large_comment) <= COMMENT_BODY_LIMIT, True)
    assert_equal("large comment reports omitted rows", "additional added tests" in large_comment, True)
    large_full_comment = format_pr_full_comment(large_diff)
    assert_equal("large full comment has last item", "fixture::added_01999" in large_full_comment, True)
    assert_equal("large full comment has no omitted rows", "additional added tests" in large_full_comment, False)

    ansi_output = "\x1b[32m     Running unittests src/lib.rs (target/debug/deps/upeg_core-1111111111111111)\x1b[0m\n"
    ansi_output += "\x1b[32mtest fixture::colorful ... ok\x1b[0m\n"
    ansi_tests = parse_libtest_output(ansi_output)
    assert_equal("ansi target", ansi_tests[0]["target"], "upeg_core::src/lib.rs")
    assert_equal("ansi status", ansi_tests[0]["status"], "passed")
    ignored_reason = parse_libtest_output("test fixture::paused ... ignored, requires network\n")
    assert_equal("ignored reason remains supported", ignored_reason[0]["status"], "ignored")

    # libtest tags a `#[should_panic]` test's name in the run output but not
    # in `--list`; both sides must reduce to the same name or discovery
    # reports it as "listed but never ran".
    should_panic_output = "     Running unittests src/lib.rs (target/debug/deps/upeg_core-1111111111111111)\n"
    should_panic_output += "test fixture::boom - should panic ... ok\n"
    should_panic_tests = parse_libtest_output(should_panic_output)
    assert_equal("should panic name", should_panic_tests[0]["fullName"], "fixture::boom")
    assert_equal("should panic status", should_panic_tests[0]["status"], "passed")
    merge_discovered_tests("workspace", ["fixture::boom"], should_panic_tests)

    interleaved_output = """     Running unittests src/lib.rs (target/debug/deps/upeg_core-1111111111111111)
running 2 tests
test fixture::child_stderr ... upeg: loaded one tool
ok
test fixture::next ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
"""
    interleaved_tests = parse_libtest_output(interleaved_output)
    assert_equal(
        "child stderr between test name and status",
        [(test["fullName"], test["status"]) for test in interleaved_tests],
        [("fixture::child_stderr", "passed"), ("fixture::next", "passed")],
    )
    merge_discovered_tests("workspace", ["fixture::child_stderr", "fixture::next"], interleaved_tests)
    partial_output = "test fixture::partial ... child output\ntest fixture::next ... ok\n"
    assert_raises(
        "another test cannot supply a missing status",
        "missing from actual libtest output",
        lambda: merge_discovered_tests(
            "workspace", ["fixture::partial", "fixture::next"], parse_libtest_output(partial_output)
        ),
    )
    assert_raises(
        "a summary cannot supply a missing status",
        "missing from actual libtest output",
        lambda: merge_discovered_tests(
            "workspace", ["fixture::missing"],
            parse_libtest_output("test fixture::missing ... child output\ntest result: ok. 0 passed; 0 failed\n"),
        ),
    )
    assert_equal(
        "a crashed harness leaves the test unparsed",
        output_is_parseable(1, "test fixture::crashed ... child output\n", parse_libtest_output("test fixture::crashed ... child output\n")),
        False,
    )
    assert_raises(
        "two standalone statuses are ambiguous",
        "ambiguous libtest status",
        lambda: parse_libtest_output("test fixture::ambiguous ... child output\nok\nFAILED\n"),
    )
    assert_raises(
        "child output cannot impersonate a libtest status",
        "split libtest status counts disagree",
        lambda: parse_libtest_output(
            "test fixture::missing ... child output\nok\n"
            "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured\n"
        ),
    )
    assert_raises(
        "inline child output cannot impersonate a libtest status",
        "missing from actual libtest output",
        lambda: merge_discovered_tests(
            "workspace", ["fixture::missing"],
            parse_libtest_output(
                "test fixture::missing ... ok: child output\n"
                "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured\n"
            ),
        ),
    )
    assert_raises(
        "a split status without a summary is incomplete",
        "split libtest status has no summary",
        lambda: parse_libtest_output("test fixture::incomplete ... child output\nok\n"),
    )

    status_tests = [
        dict(schema_test, methodName="later", fullName="fixture::later", status="passed"),
        dict(schema_test, methodName="first", fullName="fixture::first", status="failed"),
    ]
    merged = merge_discovered_tests("workspace", ["fixture::first", "fixture::later"], status_tests)
    assert_equal("discovery order", [item["fullName"] for item in merged], ["fixture::first", "fixture::later"])
    assert_equal("discovery keeps actual status", merged[0]["status"], "failed")
    assert_raises(
        "missing discovered status",
        "missing from actual libtest output",
        lambda: merge_discovered_tests("workspace", ["fixture::missing"], status_tests),
    )
    assert_equal("doc empty discovery", merge_discovered_tests("docs", [], status_tests), status_tests)
    with tempfile.TemporaryDirectory() as temp_dir:
        temp_root = Path(temp_dir)
        cargo_fixture = Path(temp_dir) / "cargo_test_fixture.py"
        cargo_fixture.write_text(
            "import sys\n"
            "if '--list' in sys.argv:\n"
            "    if '--discovery-error' in sys.argv:\n"
            "        print('discovery failed')\n"
            "        raise SystemExit(3)\n"
            "    print('fixture::failed: test')\n"
            "    print('fixture::not_run: test')\n"
            "elif '--success' in sys.argv:\n"
            "    print('test fixture::failed ... ok')\n"
            "else:\n"
            "    print('     Running unittests src/lib.rs (target/debug/deps/fixture-1111111111111111)')\n"
            "    print('test fixture::failed ... FAILED')\n"
            "    print('test result: FAILED. 0 passed; 1 failed; 0 ignored')\n"
            "    raise SystemExit(1)\n",
            encoding="utf-8",
        )
        assert_raises(
            "a failed cargo run reports the exit code before missing tests",
            "cargo test failed with exit code 1; existing baseline was not changed\n"
            "     Running unittests src/lib.rs (target/debug/deps/fixture-1111111111111111)\n"
            "test fixture::failed ... FAILED",
            lambda: run_suite(temp_root, "workspace", [sys.executable, str(cargo_fixture)]),
        )
        diagnostics = temp_root / "target/test-baseline"
        assert_equal(
            "failed cargo output is preserved exactly",
            (diagnostics / "workspace-run.log").read_text(encoding="utf-8"),
            "     Running unittests src/lib.rs (target/debug/deps/fixture-1111111111111111)\n"
            "test fixture::failed ... FAILED\n"
            "test result: FAILED. 0 passed; 1 failed; 0 ignored\n",
        )
        assert_equal(
            "failed cargo exit code is preserved",
            (diagnostics / "workspace-run.exit-code").read_text(encoding="utf-8"),
            "1\n",
        )
        assert_raises(
            "a successful cargo run still checks for missing discovered tests",
            "missing from actual libtest output",
            lambda: run_suite(
                temp_root,
                "workspace",
                [sys.executable, str(cargo_fixture), "--success"],
            ),
        )
        assert_equal(
            "missing status keeps the raw successful output",
            (diagnostics / "workspace-run.log").read_text(encoding="utf-8"),
            "test fixture::failed ... ok\n",
        )
        assert_equal(
            "missing status keeps cargo exit zero",
            (diagnostics / "workspace-run.exit-code").read_text(encoding="utf-8"),
            "0\n",
        )
        assert_raises(
            "discovery errors preserve output before collection",
            "cargo test discovery failed",
            lambda: run_suite(
                temp_root, "workspace", [sys.executable, str(cargo_fixture), "--discovery-error"]
            ),
        )
        assert_equal(
            "discovery output is preserved exactly",
            (diagnostics / "workspace-discovery.log").read_text(encoding="utf-8"),
            "discovery failed\n",
        )
        assert_equal(
            "discovery exit code is preserved",
            (diagnostics / "workspace-discovery.exit-code").read_text(encoding="utf-8"),
            "3\n",
        )
    with tempfile.TemporaryDirectory() as temp_dir:
        flutter_fixture = Path(temp_dir) / "flutter_test_fixture.py"
        # When one file fails to load/compile, flutter exits non-zero while
        # every *recorded* test stays passed — reproduces the hole where
        # that file's tests were frozen into the baseline as if they never
        # existed.
        flutter_fixture.write_text(
            "import json, sys\n"
            "events = [\n"
            "    {'type': 'suite', 'suite': {'id': 0, 'path': '/repo/flutter_app/test/ok_test.dart'}},\n"
            "    {'type': 'group', 'group': {'id': 1, 'name': 'App'}},\n"
            "    {'type': 'testStart', 'test': {'id': 2, 'name': 'App loads', 'groupIDs': [1],\n"
            "                                   'suiteID': 0,\n"
            "                                   'url': 'file:///repo/flutter_app/test/ok_test.dart'}},\n"
            "    {'type': 'testDone', 'testID': 2, 'result': 'success'},\n"
            "]\n"
            "for event in events:\n"
            "    print(json.dumps(event))\n"
            "print('Failed to load \"test/broken_test.dart\": compile error', file=sys.stderr)\n"
            "raise SystemExit(1)\n",
            encoding="utf-8",
        )
        assert_raises(
            "a failed flutter run does not write a baseline even with no recorded failures",
            "flutter test failed with exit code 1; existing baseline was not changed",
            lambda: run_flutter_suite(
                repo_root(), FLUTTER_SUITE_NAME, [sys.executable, str(flutter_fixture)]
            ),
        )
        tolerated, _ = run_flutter_suite(
            repo_root(),
            FLUTTER_SUITE_NAME,
            [sys.executable, str(flutter_fixture)],
            tolerate_failures=True,
        )
        assert_equal(
            "compare mode still tolerates the same run",
            [test["fullName"] for test in tolerated["tests"]],
            ["App::loads"],
        )

    failing_output = """
     Running unittests src/lib.rs (target/debug/deps/upeg_core-1111111111111111)
running 2 tests
test fixture::kept ... ok
test fixture::changed ... FAILED

failures:

---- fixture::changed stdout ----
printed before the panic
thread 'fixture::changed' panicked at upeg-core/src/lib.rs:10:5:
assertion `left == right` failed
  left: 1
 right: 2

failures:
    fixture::changed

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
"""
    captured = parse_failure_outputs(failing_output)
    failing_key = ("upeg_core::src/lib.rs", "fixture::changed")
    assert_equal("failure capture key", failing_key in captured, True)
    assert_equal(
        "failure capture keeps the panic",
        "assertion `left == right` failed" in captured[failing_key],
        True,
    )
    assert_equal(
        "failure capture keeps preceding stdout",
        "printed before the panic" in captured[failing_key],
        True,
    )
    assert_equal(
        "failure capture drops the roll-call",
        "fixture::changed\n    fixture::changed" in captured[failing_key],
        False,
    )
    assert_equal("passing test has no capture", len(captured), 1)

    short_excerpt = "line1\nline2"
    assert_equal("short excerpt untouched", bounded_excerpt(short_excerpt, 4), short_excerpt)
    long_excerpt = bounded_excerpt("\n".join("line" + str(i) for i in range(100)), 4)
    assert_equal("long excerpt bounded", len(long_excerpt.splitlines()), 5)
    assert_equal("long excerpt keeps head", long_excerpt.startswith("line0"), True)
    assert_equal("long excerpt keeps tail", long_excerpt.endswith("line99"), True)
    assert_equal("long excerpt marks elision", EXCERPT_ELISION in long_excerpt, True)

    flutter_error_stream = "\n".join(
        json.dumps(event)
        for event in (
            {"type": "suite", "suite": {"id": 0, "path": "/repo/flutter_app/test/a_test.dart"}},
            {"type": "group", "group": {"id": 1, "name": "App"}},
            {
                "type": "testStart",
                "test": {
                    "id": 2,
                    "name": "App renders_board",
                    "groupIDs": [1],
                    "suiteID": 0,
                    "url": "file:///repo/flutter_app/test/a_test.dart",
                },
            },
            {
                "type": "error",
                "testID": 2,
                "error": "Expected: <1>\n  Actual: <2>",
                "stackTrace": "package:flutter_test/src/widget_tester.dart 1:2",
            },
            {"type": "testDone", "testID": 2, "result": "failure"},
        )
    )
    flutter_captured = parse_flutter_failure_outputs(flutter_error_stream)
    flutter_key = ("flutter::flutter_app/test/a_test.dart", "App::renders_board")
    assert_equal("flutter failure capture key", flutter_key in flutter_captured, True)
    assert_equal(
        "flutter failure capture keeps the message",
        "Expected: <1>" in flutter_captured[flutter_key],
        True,
    )

    failed_document = make_document(
        {
            "workspace": make_suite(
                SUITES["workspace"], parse_libtest_output(failing_output)
            )
        }
    )
    failed_items = failed_tests(failed_document)
    assert_equal("failed_tests finds the failure", len(failed_items), 1)
    assert_equal(
        "failure_key joins suite/target/name",
        failure_key(failed_items[0]),
        ("workspace", "upeg_core::src/lib.rs", "fixture::changed"),
    )

    print("self-test ok")
    return 0


def build_parser():
    parser = argparse.ArgumentParser(description="Maintain stable libtest baselines for upeg.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    write_parser = subparsers.add_parser("write", help="run cargo tests and atomically write fixtures/test-baseline.json")
    write_parser.add_argument("--output", default=BASELINE_FILE, help="baseline path to write (default: fixtures/test-baseline.json)")
    write_parser.add_argument("--dry-run", action="store_true", help="print baseline JSON to stdout without writing a file")
    write_parser.set_defaults(func=cmd_write)

    compare_parser = subparsers.add_parser("compare", help="compare current cargo tests against fixtures/test-baseline.json")
    compare_parser.add_argument("--baseline", default=BASELINE_FILE, help="baseline JSON path (default: fixtures/test-baseline.json)")
    compare_parser.add_argument(
        "--current-output",
        default=CURRENT_FILE,
        help="current run JSON path to write (default: target/test-baseline/current.json)",
    )
    compare_parser.add_argument(
        "--diff-output",
        default=DIFF_FILE,
        help="diff JSON path to write (default: target/test-baseline/diff.json)",
    )
    compare_parser.set_defaults(func=cmd_compare)

    comment_parser = subparsers.add_parser("pr-comment", help="render a PR comment body from a diff JSON file")
    comment_parser.add_argument("--diff", default=DIFF_FILE, help="diff JSON path (default: target/test-baseline/diff.json)")
    comment_parser.add_argument(
        "--output",
        default=COMMENT_FILE,
        help="markdown path to write (default: target/test-baseline/pr-comment.md); pass an empty string for stdout",
    )
    comment_parser.add_argument(
        "--full-output",
        default="",
        help="full markdown report path to write; omit to skip",
    )
    comment_parser.set_defaults(func=cmd_pr_comment)

    fixture_comment_parser = subparsers.add_parser(
        "fixture-pr-comment",
        help="render a PR comment body comparing committed fixture JSON files",
    )
    fixture_comment_parser.add_argument("--base", required=True, help="target branch fixture JSON path")
    fixture_comment_parser.add_argument("--head", required=True, help="PR head fixture JSON path")
    fixture_comment_parser.add_argument(
        "--diff-output",
        default="target/test-baseline/pr-fixture-diff.json",
        help="fixture diff JSON path to write (default: target/test-baseline/pr-fixture-diff.json); pass an empty string to skip",
    )
    fixture_comment_parser.add_argument(
        "--output",
        default=COMMENT_FILE,
        help="markdown path to write (default: target/test-baseline/pr-comment.md); pass an empty string for stdout",
    )
    fixture_comment_parser.add_argument(
        "--full-output",
        default="",
        help="full markdown report path to write; omit to skip",
    )
    fixture_comment_parser.add_argument("--base-ref", default="target branch", help="label for the target branch ref")
    fixture_comment_parser.add_argument("--head-ref", default="this PR", help="label for the PR head ref")
    fixture_comment_parser.set_defaults(func=cmd_fixture_pr_comment)

    self_parser = subparsers.add_parser("self-test", help="run fixture-based parser and drift checks")
    self_parser.set_defaults(func=cmd_self_test)
    return parser


def main(argv):
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return args.func(args)
    except BaselineError as error:
        print(str(error), file=sys.stderr)
        return 2
    except KeyboardInterrupt:
        print("interrupted", file=sys.stderr)
        return 130


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
