/// Native presentation E2E for a standalone, read-only Git Toolkit.
///
/// Prepare the same isolated UPEG runtime directories as
/// presentation_toolkit_test.dart, but no ecosystem installation is needed:
///
/// ```sh
/// repo_root="$(git rev-parse --show-toplevel)"
/// cd "$repo_root/flutter_app"
/// lab="$(mktemp -d "${TMPDIR:-/tmp}/upeg-git-presentation.XXXXXX")"
/// mkdir -p "$lab/upeg-home/toolkits" "$lab/upeg-home/wasm" "$lab/upeg-home/mcp-imports"
/// UPEG_PRESENTATION_GIT_LAB="$lab" UPEG_HOME="$lab/upeg-home" \
/// UPEG_TOOLKITS_DIR="$lab/upeg-home/toolkits" UPEG_WASM_DIR="$lab/upeg-home/wasm" \
/// UPEG_MCP_IMPORTS_DIR="$lab/upeg-home/mcp-imports" UPEG_PROJECT_MANIFEST_PATH=off \
/// UPEG_BINARY="$repo_root/target/debug/upeg" \
/// flutter test integration_test/presentation_git_test.dart -d macos
/// ```
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/boot.dart' as boot;
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/frb_generated.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/widgets/expanded_modal/outcome_block.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

const _labVariable = 'UPEG_PRESENTATION_GIT_LAB';
const _timeout = Duration(seconds: 90);

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  testWidgets(
    'Git Toolkit rows open the selected ref form with authoritative values',
    (tester) async {
      final robot = _GitPresentationRobot(tester);
      try {
        await robot.start();
        await robot.openRefList();
        await robot.openSelectedRef();
        await robot.runSelectedRef();
        await robot.expectAuthoritativeCliValues();
      } finally {
        await robot.close();
      }
    },
    timeout: const Timeout(Duration(minutes: 5)),
    skip: _environmentProblem() != null,
  );
}

String? _environmentProblem() {
  if (!Platform.isMacOS) {
    return 'Requires the native macOS Flutter runner.';
  }
  final required = [
    _labVariable,
    'UPEG_HOME',
    'UPEG_TOOLKITS_DIR',
    'UPEG_WASM_DIR',
    'UPEG_MCP_IMPORTS_DIR',
    'UPEG_BINARY',
  ];
  for (final name in required) {
    if ((Platform.environment[name] ?? '').isEmpty) {
      return 'Set $name; see this file header.';
    }
  }
  if (Platform.environment['UPEG_PROJECT_MANIFEST_PATH'] != 'off') {
    return 'Set UPEG_PROJECT_MANIFEST_PATH=off.';
  }
  return null;
}

final class _GitPresentationRobot {
  _GitPresentationRobot(this.tester);

  final WidgetTester tester;
  late final Directory _lab;
  late final Directory _repo;
  late final Directory _toolkits;
  late final String _branch;
  late final String _commit;
  bool _rustInitialized = false;

  Future<void> start() async {
    _lab = Directory(Platform.environment[_labVariable]!).absolute;
    _toolkits = Directory(Platform.environment['UPEG_TOOLKITS_DIR']!).absolute;
    _repo = Directory(
      '${_lab.path}${Platform.pathSeparator}git fixture with spaces',
    );
    _branch = 'feature/topic/v1';
    await _installToolkitAndCreateRepository();
    await RustLib.init();
    _rustInitialized = true;
    await _complete(boot.initApp());
    final tool = listTools(
      toolkit: 'git_refs',
    ).singleWhere((item) => item.id == 'git_refs.refs');
    expect(tool.invoker, InvokerDto.external_);
    await tester.pumpWidget(
      ProviderScope(
        child: MaterialApp(
          theme: UpegTheme.darkTheme(),
          home: ExpandedModalPage(tool: tool),
        ),
      ),
    );
    await tester.pump();
  }

  Future<void> _installToolkitAndCreateRepository() async {
    await _toolkits.create(recursive: true);
    final sourceRoot = _findSourceRoot();
    for (final name in ['git-refs.toml', 'git_refs.py']) {
      await File(
        '${sourceRoot.path}/examples/tools/$name',
      ).copy('${_toolkits.path}/$name');
    }
    await _repo.create(recursive: true);
    await _git(['init', '--initial-branch=trunk']);
    await _git(['config', 'user.email', 'presentation@example.test']);
    await _git(['config', 'user.name', 'Presentation fixture']);
    await File('${_repo.path}/note.txt').writeAsString('trunk\n');
    await _git(['add', 'note.txt']);
    await _git(['commit', '-m', 'trunk']);
    await _git(['switch', '-c', _branch]);
    await File('${_repo.path}/note.txt').writeAsString('branch\n');
    await _git(['commit', '-am', 'branch commit']);
    _commit = (await _git([
      'rev-parse',
      '--verify',
      '--end-of-options',
      'HEAD^{commit}',
    ])).trim();
  }

  Directory _findSourceRoot() {
    var cursor = Directory.current.absolute;
    while (true) {
      if (File('${cursor.path}/examples/tools/git-refs.toml').existsSync()) {
        return cursor;
      }
      final parent = cursor.parent;
      if (parent.path == cursor.path) {
        throw StateError('Cannot locate examples/tools/git-refs.toml');
      }
      cursor = parent;
    }
  }

  Future<String> _git(List<String> arguments) async {
    final result = await Process.run(
      'git',
      ['-C', _repo.path, ...arguments],
      stdoutEncoding: utf8,
      stderrEncoding: utf8,
    );
    expect(result.exitCode, 0, reason: '${result.stderr}\n${result.stdout}');
    return result.stdout as String;
  }

  Future<void> openRefList() async {
    await tester.enterText(find.byKey(const Key('field-repo')), _repo.path);
    await tester.pump();
    await _tapAndWait(
      find.byKey(const Key('expanded-modal-run-btn')),
      find.byKey(Key('presentation-table-row-$_branch')),
    );
  }

  Future<void> openSelectedRef() async {
    final row = find.byKey(Key('presentation-table-row-$_branch'));
    await tester.ensureVisible(row);
    await tester.tap(row);
    await tester.pump();
    await _tapAndWait(
      find.byKey(const Key('presentation-table-action-inspect')),
      find.byKey(const Key('field-ref')),
    );
    expect(_fieldValue('repo'), _repo.path);
    expect(_fieldValue('ref'), _branch);
  }

  Future<void> expectAuthoritativeCliValues() async {
    final upeg = Platform.environment['UPEG_BINARY']!;
    final validate = await Process.run(
      upeg,
      ['tool', 'validate', '${_toolkits.path}/git-refs.toml'],
      stdoutEncoding: utf8,
      stderrEncoding: utf8,
    );
    expect(
      validate.exitCode,
      0,
      reason: '${validate.stderr}\n${validate.stdout}',
    );
    final call = await Process.run(
      upeg,
      [
        'call',
        'git_refs.ref',
        '--json',
        '-a',
        'repo=${_repo.path}',
        '-a',
        'ref=$_branch',
      ],
      stdoutEncoding: utf8,
      stderrEncoding: utf8,
    );
    expect(call.exitCode, 0, reason: '${call.stderr}\n${call.stdout}');
    final envelope = jsonDecode(call.stdout as String) as Map<String, Object?>;
    final outputs = envelope['outputs'] as List<Object?>;
    final reference =
        (outputs.single as Map<String, Object?>)['value']
            as Map<String, Object?>;
    expect(reference['repo'], _repo.path);
    expect(reference['ref'], _branch);
    expect(reference['object'], _commit);
  }

  Future<void> runSelectedRef() async {
    final modal = find
        .ancestor(
          of: find.byKey(const Key('field-ref')).last,
          matching: find.byType(Scaffold),
        )
        .last;
    final run = find.descendant(
      of: modal,
      matching: find.byKey(const Key('expanded-modal-run-btn')),
    );
    final outcome = find.descendant(
      of: modal,
      matching: find.byKey(const Key('expanded-modal-outcome')),
    );
    await tester.pump();
    await _tapAndWait(run, outcome);
    final block = find.descendant(of: modal, matching: find.byType(OutcomeBlock));
    final result = tester.widget<OutcomeBlock>(block).outcome;
    expect(result.ok, isTrue);
    final reference = result.jsonValues['reference'] as Map<String, Object?>;
    expect(reference['repo'], _repo.path);
    expect(reference['ref'], _branch);
    expect(reference['object'], _commit);
  }

  String _fieldValue(String name) => tester
      .widget<TextFormField>(find.byKey(Key('field-$name')).last)
      .initialValue!;

  Future<void> _tapAndWait(Finder tap, Finder expected) async {
    await tester.ensureVisible(tap);
    await tester.tap(tap);
    await _waitFor(expected);
  }

  Future<void> _waitFor(Finder finder) async {
    final deadline = DateTime.now().add(_timeout);
    while (finder.evaluate().isEmpty && DateTime.now().isBefore(deadline)) {
      await tester.pump(const Duration(milliseconds: 50));
      for (final block in tester.widgetList<OutcomeBlock>(
        find.byType(OutcomeBlock),
      )) {
        if (!block.outcome.ok) {
          fail(
            'Native Tool failed: ${block.outcome.errorMessage}\n${block.outcome.errorDetailsText}',
          );
        }
      }
    }
    expect(finder, findsWidgets, reason: 'Timed out waiting for $finder');
  }

  Future<T> _complete<T>(Future<T> operation) async {
    var done = false;
    late T value;
    Object? failure;
    StackTrace? trace;
    unawaited(
      operation.then<void>(
        (result) {
          value = result;
          done = true;
        },
        onError: (Object error, StackTrace stack) {
          failure = error;
          trace = stack;
          done = true;
        },
      ),
    );
    final deadline = DateTime.now().add(_timeout);
    while (!done && DateTime.now().isBefore(deadline)) {
      await tester.pump(const Duration(milliseconds: 50));
    }
    if (!done) throw TimeoutException('Timed out waiting for native boot.');
    if (failure case final error?) Error.throwWithStackTrace(error, trace!);
    return value;
  }

  Future<void> close() async {
    await tester.pumpWidget(const SizedBox.shrink());
    if (_rustInitialized) boot.shutdown();
  }
}
