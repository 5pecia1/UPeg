#!/usr/bin/env python3
"""Release artifact set, build manifest, and verified-run gate.

Build side (public release workflow):
  expected-files VERSION TAG [cli] [desktop] [web]
      Print the release file names the selected components must produce.
  manifest DIST_DIR OUT_JSON
      Write release-manifest.json covering every file in DIST_DIR; refuse
      when a selected component produced no file. Selection and metadata
      come from UPEG_* environment variables (see manifest()).

Publish side (trusted publisher):
  check DIST_DIR MANIFEST_JSON
      Verify every file in DIST_DIR against the manifest sha256/size list.
  verify-run OUT_DIR --repo-id ID --tag TAG
      Gate a downloaded public workflow run: run.json, jobs.json,
      artifacts.json, manifest/release-manifest.json and files/ under
      OUT_DIR must describe the same commit, the expected tag, the full
      component set, and byte-identical files. Writes OUT_DIR/verified.json.

Only stdlib; safe to run in the public repository and in the publisher.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys

SCHEMA = 'upeg-release-manifest/v1'
COMPONENTS = ('cli', 'desktop', 'web')
PUBLIC_REPO = '5pecia1/UPeg'
WORKFLOW_PATH = '.github/workflows/release.yml'
REQUIRED_JOBS = ('preflight', 'cli-linux', 'cli-macos', 'cli-windows',
                 'flutter-linux', 'manifest')
ARTIFACT_NAMES = ('cli-linux', 'cli-macos', 'cli-windows', 'flutter-linux',
                  'flutter-web', 'release-manifest')
SHA_PATTERN = re.compile(r'[0-9a-f]{40}')


class GateError(RuntimeError):
    """A failed check; callers must stop before publishing."""


def expected_files(version: str, tag: str, components: list[str]) -> list[str]:
    files: list[str] = []
    for component in components:
        if component == 'cli':
            for target in ('x86_64-unknown-linux-gnu', 'aarch64-apple-darwin'):
                files += [f'upeg-{tag}-{target}.tar.gz',
                          f'upeg-{tag}-{target}.tar.gz.sha256']
            files += [f'upeg-{tag}-x86_64-pc-windows-msvc.zip',
                      f'upeg-{tag}-x86_64-pc-windows-msvc.zip.sha256']
        elif component == 'desktop':
            files += [f'upeg_{version}_amd64.deb',
                      f'upeg_{version}_amd64.deb.sha256',
                      f'upeg-{tag}-x86_64.AppImage',
                      f'upeg-{tag}-x86_64.AppImage.sha256']
        elif component == 'web':
            files += [f'upeg-{tag}-web.tar.gz',
                      f'upeg-{tag}-web.tar.gz.sha256']
        else:
            raise GateError(f'unknown component: {component}')
    return sorted(files)


def selected_components(env: dict[str, str]) -> list[str]:
    selected = [c for c in COMPONENTS if env.get(f'UPEG_WANT_{c.upper()}', '1') == '1']
    unknown = set(selected) - set(COMPONENTS)
    if unknown:
        raise GateError(f'unknown components: {sorted(unknown)}')
    return selected


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b''):
            digest.update(chunk)
    return digest.hexdigest()


def file_entries(dist_dir: Path) -> list[dict]:
    entries = []
    for path in sorted(dist_dir.iterdir()):
        if path.is_file():
            entries.append({'name': path.name, 'sha256': sha256(path),
                            'bytes': path.stat().st_size})
    return entries


def manifest(dist_dir: Path, out_json: Path, env: dict[str, str]) -> dict:
    version = env.get('UPEG_VERSION', '')
    tag = env.get('UPEG_TAG', '')
    components = selected_components(env)
    if not tag.startswith('v') or tag[1:] != version:
        raise GateError(f'tag {tag!r} must equal v{version!r}')
    entries = file_entries(dist_dir)
    present = {entry['name'] for entry in entries}
    missing = sorted(set(expected_files(version, tag, components)) - present)
    if missing:
        raise GateError(f'selected components missing files: {missing}')
    document = {
        'schema': SCHEMA,
        'repo': repo if (repo := env.get('UPEG_REPO', '')) else PUBLIC_REPO,
        'run_id': env.get('UPEG_RUN_ID', ''),
        'run_attempt': env.get('UPEG_RUN_ATTEMPT', ''),
        'workflow': env.get('UPEG_WORKFLOW', 'release'),
        'event': env.get('UPEG_EVENT', 'workflow_dispatch'),
        'ref': env.get('UPEG_REF', 'refs/heads/main'),
        'commit': env.get('UPEG_COMMIT', ''),
        'version': version,
        'tag': tag,
        'flutter_version': env.get('UPEG_FLUTTER_VERSION', ''),
        'components': components,
        'files': entries,
    }
    out_json.write_text(json.dumps(document, indent=2) + '\n')
    return document


def load_manifest(manifest_json: Path) -> dict:
    document = json.loads(manifest_json.read_text())
    if document.get('schema') != SCHEMA:
        raise GateError(f'unexpected manifest schema: {document.get("schema")!r}')
    if not document.get('files'):
        raise GateError('manifest lists no files')
    return document


def check(dist_dir: Path, manifest_json: Path) -> dict:
    document = load_manifest(manifest_json)
    entries = {entry['name']: entry for entry in document['files']}
    actual = {path.name: path for path in dist_dir.iterdir() if path.is_file()}
    if set(actual) != set(entries):
        raise GateError(
            f'artifact set mismatch: missing={sorted(set(entries) - set(actual))}, '
            f'unexpected={sorted(set(actual) - set(entries))}')
    for name, entry in entries.items():
        path = actual[name]
        if path.stat().st_size != entry['bytes'] or sha256(path) != entry['sha256']:
            raise GateError(f'artifact content mismatch: {name}')
    return document


def verify_run(out_dir: Path, repo_id: str, run_id: str, tag: str) -> dict:
    run = json.loads((out_dir / 'run.json').read_text())
    repo = json.loads((out_dir / 'repo.json').read_text())
    jobs = json.loads((out_dir / 'jobs.json').read_text())
    artifacts = json.loads((out_dir / 'artifacts.json').read_text())
    document = check(out_dir / 'files', out_dir / 'manifest' / 'release-manifest.json')

    if not repo_id.isdecimal() or not run_id.isdecimal():
        raise GateError('expected numeric repository and run IDs')
    if repo.get('id') != int(repo_id) or repo.get('full_name') != PUBLIC_REPO:
        raise GateError('repository ID/name mismatch; the URL may still redirect')
    if run.get('id') != int(run_id):
        raise GateError('saved run metadata is not for the requested run ID')
    if run.get('path') != WORKFLOW_PATH:
        raise GateError(f'run is not the release workflow: {run.get("path")!r}')
    if run.get('event') != 'workflow_dispatch' or run.get('head_branch') != 'main':
        raise GateError('run must be a workflow_dispatch on main')
    if run.get('status') != 'completed' or run.get('conclusion') != 'success':
        raise GateError(f'run did not succeed: {run.get("conclusion")!r}')
    commit = run.get('head_sha', '')
    if not SHA_PATTERN.fullmatch(commit):
        raise GateError('run head_sha is not a full commit SHA')
    if run.get('repository', {}).get('id') != int(repo_id) \
            or run.get('head_repository', {}).get('id') != int(repo_id):
        raise GateError('run did not run on the public repository itself')
    if run.get('run_attempt') != int(document.get('run_attempt') or 0):
        raise GateError('manifest run_attempt differs from the verified run')

    job_results = {job['name']: job.get('conclusion') for job in jobs.get('jobs', [])}
    failed = {name: result for name, result in job_results.items()
              if result not in ('success', 'skipped')}
    if failed:
        raise GateError(f'failed jobs in run: {failed}')
    for name in REQUIRED_JOBS:
        if job_results.get(name) != 'success':
            raise GateError(f'required job did not succeed: {name}={job_results.get(name)!r}')

    wanted = set(ARTIFACT_NAMES)
    seen = {}
    for artifact in artifacts.get('artifacts', []):
        name = artifact.get('name')
        if name not in wanted:
            raise GateError(f'unexpected artifact: {name!r}')
        if artifact.get('expired'):
            raise GateError(f'artifact expired: {name!r}')
        seen[name] = artifact
    if set(seen) != wanted:
        raise GateError(f'missing artifacts: {sorted(wanted - set(seen))}')

    if document.get('repo') != PUBLIC_REPO or document.get('commit') != commit:
        raise GateError('manifest repo/commit differ from the verified run')
    if document.get('tag') != tag or document.get('version') != tag.removeprefix('v'):
        raise GateError(f'manifest tag {document.get("tag")!r} does not match {tag!r}')
    if sorted(document.get('components', [])) != list(COMPONENTS):
        raise GateError('run did not build the full release component set')
    expected = set(expected_files(document['version'], tag, list(COMPONENTS)))
    listed = {entry['name'] for entry in document['files']}
    if listed != expected:
        raise GateError(f'release file set differs: missing={sorted(expected - listed)}, '
                        f'unexpected={sorted(listed - expected)}')

    verified = {
        'tag': tag,
        'version': document['version'],
        'commit': commit,
        'repo': PUBLIC_REPO,
        'repo_id': int(repo_id),
        'run_id': run.get('id'),
        'run_attempt': run.get('run_attempt'),
        'files': sorted(listed),
    }
    (out_dir / 'verified.json').write_text(json.dumps(verified, indent=2) + '\n')
    return verified


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    exp = sub.add_parser('expected-files')
    exp.add_argument('version')
    exp.add_argument('tag')
    exp.add_argument('components', nargs='*', default=list(COMPONENTS))
    man = sub.add_parser('manifest')
    man.add_argument('dist_dir', type=Path)
    man.add_argument('out_json', type=Path)
    chk = sub.add_parser('check')
    chk.add_argument('dist_dir', type=Path)
    chk.add_argument('manifest_json', type=Path)
    ver = sub.add_parser('verify-run')
    ver.add_argument('out_dir', type=Path)
    ver.add_argument('--repo-id', required=True)
    ver.add_argument('--run-id', required=True)
    ver.add_argument('--tag', required=True)
    args = parser.parse_args()
    try:
        if args.command == 'expected-files':
            components = args.components or list(COMPONENTS)
            print('\n'.join(expected_files(args.version, args.tag, components)))
        elif args.command == 'manifest':
            document = manifest(args.dist_dir, args.out_json, dict(os.environ))
            print(f"[PASS] manifest: {len(document['files'])} files, tag {document['tag']}")
        elif args.command == 'check':
            document = check(args.dist_dir, args.manifest_json)
            print(f"[PASS] artifacts verified: {len(document['files'])} files, tag {document['tag']}")
        elif args.command == 'verify-run':
            verified = verify_run(args.out_dir, args.repo_id, args.run_id, args.tag)
            print(f"[PASS] run verified: tag {verified['tag']} at {verified['commit']}")
    except (GateError, OSError, ValueError, KeyError) as error:
        print(f'[FAIL] {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
