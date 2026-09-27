/// Native presentation E2E against the installed my-ecosystem CLI.
///
/// This test deliberately avoids Dart resolver/dispatcher overrides. It
/// proves the macOS runner inherits isolated runtime sources, Rust loads the
/// exported Toolkit, and production Flutter identifiers carry one external
/// mode change to the refreshed read.
///
/// Prepare a fresh lab before running; do not reuse a running desktop lab:
///
/// ```sh
/// lab="$(mktemp -d /private/tmp/ecosystem-upeg-presentation.XXXXXX)"
/// bash /Users/sol/dev/src/github.com/5pecia1/my-ecosystem/install.sh \
///   --source-root /Users/sol/dev/src/github.com/5pecia1/my-ecosystem \
///   --prefix "$lab/prefix" --home "$lab/ecosystem-home" --runtime none
/// mkdir -p "$lab/upeg-home/toolkits" "$lab/project2" "$lab/probe-skill"
/// # Put a minimal SKILL.md in "$lab/probe-skill", then register/add it.
/// ECOSYSTEM_HOME="$lab/ecosystem-home" "$lab/prefix/bin/ecosystem" \
///   toolkit export --output "$lab/upeg-home/toolkits/ecosystem.toml" \
///   --executable "$lab/prefix/bin/ecosystem"
/// ```
///
/// Run with the native macOS runner's inherited shell environment:
/// `UPEG_PRESENTATION_LAB="$lab" ECOSYSTEM_HOME="$lab/ecosystem-home" \
/// UPEG_HOME="$lab/upeg-home" UPEG_TOOLKITS_DIR="$lab/upeg-home/toolkits" \
/// UPEG_WASM_DIR="$lab/upeg-home/wasm" \
/// UPEG_MCP_IMPORTS_DIR="$lab/upeg-home/mcp-imports" \
/// UPEG_PROJECT_MANIFEST_PATH=off flutter test integration_test/presentation_toolkit_test.dart -d macos`
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
import 'package:upeg/src/theme/upeg_theme.dart';

const _labVariable = 'UPEG_PRESENTATION_LAB';
const _ecosystemHomeVariable = 'ECOSYSTEM_HOME';
const _upegHomeVariable = 'UPEG_HOME';
const _toolkitsVariable = 'UPEG_TOOLKITS_DIR';
const _wasmVariable = 'UPEG_WASM_DIR';
const _importsVariable = 'UPEG_MCP_IMPORTS_DIR';
const _manifestVariable = 'UPEG_PROJECT_MANIFEST_PATH';
const _projectDisabled = 'off';
const _probeSkill = 'presentation-probe';
const _modeManualLabel = '수동 호출';
const _operationTimeout = Duration(seconds: 90);
const _testTimeout = Duration(minutes: 5);

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets(
    'real ecosystem Toolkit refreshes skills after presentation mode apply',
    (tester) async {
      final robot = _PresentationToolkitRobot(tester);
      try {
        await robot.start();
        await robot.openProjectSkillsList();
        await robot.openPrefilledModePlan();
        await robot.previewAndApplyManualMode();
        await robot.expectExternalCliAndDiskState();
        await robot.returnToRefreshedSkillsList();
      } finally {
        await robot.close();
      }
    },
    timeout: const Timeout(_testTimeout),
    skip: _environmentProblem() != null,
  );
}

String? _environmentProblem() {
  if (!Platform.isMacOS) return 'Requires the native macOS Flutter runner.';
  final lab = Platform.environment[_labVariable];
  if (lab == null || lab.isEmpty) {
    return 'Set $_labVariable to a freshly prepared isolated lab; see this file header.';
  }
  for (final variable in [
    _ecosystemHomeVariable,
    _upegHomeVariable,
    _toolkitsVariable,
    _wasmVariable,
    _importsVariable,
  ]) {
    if ((Platform.environment[variable] ?? '').isEmpty) {
      return 'Set $variable for the native runner; see this file header.';
    }
  }
  if (Platform.environment[_manifestVariable] != _projectDisabled) {
    return 'Set $_manifestVariable=$_projectDisabled for this isolated test.';
  }
  return null;
}

final class _PresentationToolkitRobot {
  _PresentationToolkitRobot(this.tester);

  final WidgetTester tester;
  late final Directory _lab;
  late final Directory _upegHome;
  late final Directory _ecosystemHome;
  late final Directory _project;
  late final File _ecosystem;
  bool _rustInitialized = false;

  Future<void> start() async {
    _lab = Directory(Platform.environment[_labVariable]!).absolute;
    _upegHome = Directory(Platform.environment[_upegHomeVariable]!).absolute;
    _ecosystemHome = Directory(
      Platform.environment[_ecosystemHomeVariable]!,
    ).absolute;
    _project = Directory('${_lab.path}${Platform.pathSeparator}project2');
    _ecosystem = File(
      '${_lab.path}${Platform.pathSeparator}prefix${Platform.pathSeparator}bin'
      '${Platform.pathSeparator}ecosystem',
    );
    await _expectPreparedLab();

    await RustLib.init();
    _rustInitialized = true;
    await _complete(boot.initApp());

    final loaded = listTools(toolkit: 'ecosystem');
    expect(
      loaded.map((tool) => tool.id),
      containsAll(const [
        'ecosystem.projects',
        'ecosystem.skills',
        'ecosystem.mode_plan',
        'ecosystem.mode_apply',
      ]),
      reason: 'Rust must load the exported real Toolkit before rendering.',
    );
    expect(
      loaded.every((tool) => tool.invoker == InvokerDto.external_),
      isTrue,
      reason: 'The test must exercise External CLI dispatch, not built-ins.',
    );

    final projects = loaded.singleWhere(
      (tool) => tool.id == 'ecosystem.projects',
    );
    await tester.pumpWidget(
      ProviderScope(
        child: MaterialApp(
          theme: UpegTheme.darkTheme(),
          home: ExpandedModalPage(tool: projects),
        ),
      ),
    );
    await tester.pump();
    expect(find.byKey(const Key('generic-form-empty')), findsOneWidget);
  }

  Future<void> openProjectSkillsList() async {
    await _tapAndWait(
      find.byKey(const Key('expanded-modal-run-btn')),
      find.byKey(Key('presentation-table-row-${_project.path}')),
    );
    final projectRow = find.byKey(
      Key('presentation-table-row-${_project.path}'),
    );
    await tester.tap(projectRow);
    await tester.pump();
    await _tapAndWait(
      find.byKey(const Key('presentation-table-action-skills')),
      find.byKey(const Key('field-project')),
    );
    expect(_fieldValue('project'), _project.path);
  }

  Future<void> openPrefilledModePlan() async {
    await _tapAndWait(
      find.byKey(const Key('expanded-modal-run-btn')).last,
      find.byKey(const Key('presentation-table-row-$_probeSkill')),
    );
    final probeRow = find.byKey(
      const Key('presentation-table-row-$_probeSkill'),
    );
    await tester.ensureVisible(probeRow);
    await tester.tap(probeRow);
    await tester.pump();
    await _tapAndWait(
      find.byKey(const Key('presentation-table-action-mode_plan')),
      find.byKey(const Key('field-mode')),
    );
    expect(_fieldValue('project'), _project.path);
    expect(_fieldValue('skill'), _probeSkill);
  }

  Future<void> previewAndApplyManualMode() async {
    await tester.tap(find.byKey(const Key('field-mode')).last);
    await tester.pump();
    // Flutter's option menu has no per-option key; its declared Toolkit label
    // is stable public accessibility text.
    await tester.tap(find.text(_modeManualLabel).last);
    await tester.pump();

    await _tapAndWait(
      find.byKey(const Key('expanded-modal-run-btn')).last,
      find.byKey(const Key('presentation-result-action-apply')),
    );
    await _tapAndWait(
      find.byKey(const Key('presentation-result-action-apply')),
      find.byKey(const Key('field-expected_state')),
    );
    expect(_fieldValue('project'), _project.path);
    expect(_fieldValue('skill'), _probeSkill);
    expect(_fieldValue('expected_state'), isNotEmpty);

    await _tapRunForField('expected_state');
  }

  Future<void> expectExternalCliAndDiskState() async {
    final result = await Process.run(
      _ecosystem.path,
      ['skills', 'status', '--project', _project.path, '--rows'],
      environment: {_ecosystemHomeVariable: _ecosystemHome.path},
      stdoutEncoding: utf8,
      stderrEncoding: utf8,
    );
    expect(result.exitCode, 0, reason: '${result.stderr}\n${result.stdout}');
    final rows =
        (jsonDecode(result.stdout as String) as Map<String, Object?>)['skills']
            as List<Object?>;
    final probe = rows.cast<Map<String, Object?>>().singleWhere(
      (row) => row['id'] == _probeSkill,
    );
    expect(probe['mode'], 'manual');
    final projectManifest = await File(
      '${_project.path}${Platform.pathSeparator}ecosystem.json',
    ).readAsString();
    final projectLock = await File(
      '${_project.path}${Platform.pathSeparator}ecosystem.lock.json',
    ).readAsString();
    final activationPolicy = await File(
      '${_project.path}${Platform.pathSeparator}.agents${Platform.pathSeparator}skills'
      '${Platform.pathSeparator}$_probeSkill${Platform.pathSeparator}agents'
      '${Platform.pathSeparator}openai.yaml',
    ).readAsString();
    expect(projectManifest, contains('"mode": "manual"'));
    expect(projectLock, contains('"mode": "manual"'));
    expect(
      activationPolicy,
      contains('allow_implicit_invocation: false'),
      reason:
          'The real External apply must update its sibling activation policy.',
    );
  }

  Future<void> returnToRefreshedSkillsList() async {
    // Apply refreshes the read that opened mode-plan (ecosystem.skills). Close
    // only apply and plan so skills renders the CLI's new mode, not pre-apply output.
    await _closeTopModalForField('expected_state');
    await _closeTopModalForField('mode');
    await _waitFor(
      find.byKey(const Key('presentation-table-row-$_probeSkill')),
    );
    expect(find.text(_modeManualLabel), findsAtLeastNWidgets(1));
  }

  Future<void> _expectPreparedLab() async {
    expect(await _lab.exists(), isTrue);
    expect(await _upegHome.exists(), isTrue);
    expect(await _ecosystemHome.exists(), isTrue);
    expect(await _project.exists(), isTrue);
    expect(await _ecosystem.exists(), isTrue);
    expect(
      Platform.environment[_toolkitsVariable],
      '${_upegHome.path}${Platform.pathSeparator}toolkits',
    );
    expect(
      Platform.environment[_wasmVariable],
      '${_upegHome.path}${Platform.pathSeparator}wasm',
    );
    expect(
      Platform.environment[_importsVariable],
      '${_upegHome.path}${Platform.pathSeparator}mcp-imports',
    );
    final toolkit = File(
      '${_upegHome.path}${Platform.pathSeparator}toolkits${Platform.pathSeparator}ecosystem.toml',
    );
    expect(await toolkit.exists(), isTrue);
    final source = await toolkit.readAsString();
    expect(source, contains(_ecosystem.path));
    expect(source, isNot(contains('__ECOSYSTEM_EXECUTABLE__')));
  }

  String _fieldValue(String key) => tester
      .widget<TextFormField>(find.byKey(Key('field-$key')).last)
      .initialValue!;

  Future<void> _tapAndWait(Finder tap, Finder expected) async {
    await tester.ensureVisible(tap);
    await tester.tap(tap);
    await _waitFor(expected);
  }

  Finder _modalForField(String field) => find
      .ancestor(
        of: find.byKey(Key('field-$field')).last,
        matching: find.byType(Scaffold),
      )
      .last;

  Future<void> _tapRunForField(String field) async {
    final modal = _modalForField(field);
    final run = find.descendant(
      of: modal,
      matching: find.byKey(const Key('expanded-modal-run-btn')),
    );
    await tester.ensureVisible(run);
    await tester.tap(run);
    await _waitFor(
      find.descendant(
        of: modal,
        matching: find.byKey(const Key('expanded-modal-outcome')),
      ),
    );
  }

  Future<void> _closeTopModalForField(String field) async {
    // Each live child has a uniquely keyed production form field. Pop through
    // its Navigator context so an obscured card or OS input cannot target a
    // background presentation route.
    final context = tester.element(find.byKey(Key('field-$field')).last);
    Navigator.of(context).pop();
    await tester.pump();
  }

  Future<void> _waitFor(Finder finder) async {
    final deadline = DateTime.now().add(_operationTimeout);
    while (finder.evaluate().isEmpty && DateTime.now().isBefore(deadline)) {
      await tester.pump(const Duration(milliseconds: 50));
    }
    expect(finder, findsWidgets, reason: 'Timed out waiting for $finder');
  }

  Future<T> _complete<T>(Future<T> operation) async {
    var complete = false;
    late T value;
    Object? failure;
    StackTrace? trace;
    unawaited(
      operation.then<void>(
        (result) {
          value = result;
          complete = true;
        },
        onError: (Object error, StackTrace stack) {
          failure = error;
          trace = stack;
          complete = true;
        },
      ),
    );
    final deadline = DateTime.now().add(_operationTimeout);
    while (!complete && DateTime.now().isBefore(deadline)) {
      await tester.pump(const Duration(milliseconds: 50));
    }
    if (!complete) throw TimeoutException('Timed out waiting for native boot.');
    if (failure case final error?) Error.throwWithStackTrace(error, trace!);
    return value;
  }

  Future<void> close() async {
    await tester.pumpWidget(const SizedBox.shrink());
    if (_rustInitialized) boot.shutdown();
  }
}
