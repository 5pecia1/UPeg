/// Real CLI -> Desktop HTTP -> Rust dispatcher -> Flutter WebView acceptance.
///
/// Run this file on a desktop target in its own process with an empty temporary
/// UPEG_HOME, UPEG_PROJECT_MANIFEST_PATH=off and UPEG_TEST_CLI_PATH pointing to
/// this checkout's built CLI. Toolkit/WASM/MCP directory overrides must be
/// absent or point inside that temporary root. The embedded host exits with
/// the test process; sending `upeg host stop` would kill the test itself.
/// The ordinary integration suite skips this opt-in test. Use
/// `just flutter-controlled-embed-linux-test` to supply the isolated environment
/// and a matching CLI binary; an opted-in run keeps strict isolation checks.
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:upeg/src/features/controlled_embed/execution_bridge.dart';
import 'package:upeg/src/features/controlled_embed/session_service.dart';
import 'package:upeg/src/features/controlled_embed/webview_session.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/boot.dart' as boot;
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/rust/frb_generated.dart';
import 'package:upeg/src/widgets/controlled_embed/session_host.dart';

import '../test/shared/controlled_embed_counter_fixture.dart';

const _toolkitId = 'cli_webview_acceptance';
const _toolId = '$_toolkitId.counter';
const _homeVariable = 'UPEG_HOME';
const _cliVariable = 'UPEG_TEST_CLI_PATH';
const _projectVariable = 'UPEG_PROJECT_MANIFEST_PATH';
const _projectDisabled = 'off';
const _toolkitsDirectory = 'toolkits';
const _frameStep = Duration(milliseconds: 20);
const _operationTimeout = Duration(seconds: 75);
const _testTimeout = Duration(minutes: 4);
const _snapshotDecodeLimit = 2;
const _desktopInput = 'GUI 입력';
const _cliInput = 'CLI 입력';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets(
    'cli_uses_the_same_real_webview_state_and_integer_result_as_desktop_and_does_not_fall_back_after_disconnect',
    (tester) async {
      final robot = _CliSessionRobot(tester);
      try {
        await robot.startIsolatedDesktop();
        await robot.dispatchFromDesktopAndExpectFirstResult();
        await robot.dispatchFromCliAndExpectSharedResult();
        await robot.disconnectAndExpectUnavailableWithoutPageChanges();
      } finally {
        await robot.close();
      }
    },
    timeout: const Timeout(_testTimeout),
    skip:
        !(Platform.isLinux || Platform.isMacOS || Platform.isWindows) ||
        !Platform.environment.containsKey(_cliVariable),
  );
}

final class _CliSessionRobot {
  _CliSessionRobot(this.tester);

  final WidgetTester tester;
  final ControlledEmbedSessionService _sessions = ControlledEmbedSessionService(
    createSession: NativeControlledEmbedWebViewSession.create,
    normalizeResult: normalizeControlledEmbedResult,
  );
  ControlledEmbedCounterFixture? _fixture;
  ControlledEmbedExecutionBridge? _bridge;
  ControlledEmbedSessionEntry? _initialEntry;
  late Directory _home;
  late String _cliPath;
  late String _endpoint;
  bool _rustInitialized = false;

  Future<void> startIsolatedDesktop() async {
    await _expectIsolatedEnvironment();
    _fixture = await ControlledEmbedCounterFixture.start();
    await _writeToolkit(_fixture!.uri);
    await RustLib.init();
    _rustInitialized = true;

    final initialTweaks = loadTweaks();
    saveTweaks(
      tweaks: TweaksDto(
        theme: initialTweaks.theme,
        accent: initialTweaks.accent,
        showHoles: initialTweaks.showHoles,
        locale: initialTweaks.locale,
        localHttpHost: true,
      ),
    );
    final report = await _complete(boot.initApp());
    expect(report.hostState, isA<boot.HostStateDto_Embedded>());
    _endpoint = (report.hostState as boot.HostStateDto_Embedded).endpoint;
    expect(listTools(toolkit: _toolkitId).map((tool) => tool.id), [_toolId]);

    await tester.pumpWidget(
      MaterialApp(
        builder: (context, child) => ControlledEmbedSessionHostView(
          sessions: _sessions,
          child: child ?? const SizedBox.shrink(),
        ),
        home: const Scaffold(body: SizedBox.expand()),
      ),
    );
    _expectHostVisible();
    _bridge = ControlledEmbedExecutionBridge(sessions: _sessions);
    await _complete(_bridge!.ready);
  }

  Future<void> _expectIsolatedEnvironment() async {
    final home = Platform.environment[_homeVariable];
    final cliPath = Platform.environment[_cliVariable];
    expect(
      home,
      isNotNull,
      reason: 'Set $_homeVariable to an empty temporary directory.',
    );
    expect(home, isNotEmpty);
    expect(
      cliPath,
      isNotNull,
      reason: 'Set $_cliVariable to the built upeg CLI.',
    );
    expect(cliPath, isNotEmpty);
    _home = Directory(home!).absolute;
    _cliPath = File(cliPath!).absolute.path;
    expect(await _home.exists(), isTrue);
    expect(
      await _home.list().isEmpty,
      isTrue,
      reason: 'The test requires its own empty config root.',
    );
    expect(await File(_cliPath).exists(), isTrue);
    expect(Platform.environment[_projectVariable], _projectDisabled);
    for (final (variable, directory) in [
      ('UPEG_TOOLKITS_DIR', _toolkitsDirectory),
      ('UPEG_WASM_DIR', 'wasm'),
      ('UPEG_MCP_IMPORTS_DIR', 'mcp-imports'),
    ]) {
      final override = Platform.environment[variable];
      if (override != null) {
        expect(
          Directory(override).absolute.path,
          '${_home.path}${Platform.pathSeparator}$directory',
        );
      }
    }
  }

  Future<void> _writeToolkit(Uri page) async {
    final directory = Directory(
      '${_home.path}${Platform.pathSeparator}$_toolkitsDirectory',
    );
    await directory.create();
    await File(
      '${directory.path}${Platform.pathSeparator}counter.toml',
    ).writeAsString('''
id = "$_toolkitId"
[[tools]]
id = "counter"
pin = "ControlledEmbed"
pegboard_units = "U2T"
invoker = "Embed"
embed_url = "$page"
surfaces = ["desktop", "cli", "http"]
primary_output_id = "counter"
inputs = [
  { name = "step", type = "integer", required = true },
  { name = "text", type = "string", required = true },
]
outputs = [
  { name = "echo", type = "string" },
  { name = "counter", type = "integer", label = "Count" },
]
controlled_embed = { bindings = [
  { role = "input", field = "step", selector = "#step" },
  { role = "input", field = "text", selector = "#text" },
  { role = "trigger", field = "", selector = "#trigger" },
  { role = "output", field = "echo", selector = "#echo" },
  { role = "output", field = "counter", selector = "#counter" },
] }
''');
  }

  Future<void> dispatchFromDesktopAndExpectFirstResult() async {
    final result = await _complete(
      dispatchToolAsync(
        toolId: _toolId,
        argsJson: jsonEncode({'step': 1, 'text': _desktopInput}),
        approve: false,
      ),
    );
    expect(result.ok, isTrue, reason: result.error?.message);
    expect(result.primaryOutputId, 'counter');
    expect(result.outputs.map((entry) => entry.id), ['echo', 'counter']);
    expect(result.outputs.last.kind, 'integer');
    expect(
      result.outputs.last.value,
      const CanonicalOutputValue.integer(value: 1),
    );
    _initialEntry = _sessions.entryForTool(ToolId.parse(_toolId));
    expect(_initialEntry, isNotNull);
    await _expectPageState(counter: 1, text: _desktopInput);
  }

  Future<void> dispatchFromCliAndExpectSharedResult() async {
    final process = await _runCli(_cliInput);
    expect(process.exitCode, 0, reason: '${process.stderr}\n${process.stdout}');
    expect(process.stderr.toString(), contains('attached to host $_endpoint'));
    final result = jsonDecode(process.stdout as String) as Map<String, Object?>;
    expect(result['ok'], isTrue);
    expect(result['primary_output_id'], 'counter');
    final outputs = (result['outputs'] as List<Object?>)
        .cast<Map<String, Object?>>();
    expect(outputs.map((entry) => entry['id']), ['echo', 'counter']);
    expect(outputs.first['value'], _cliInput);
    expect(outputs.last['kind'], 'integer');
    expect(outputs.last['value'], 2);
    expect(_sessions.entryForTool(ToolId.parse(_toolId)), same(_initialEntry));
    await _expectPageState(counter: 2, text: _cliInput);
  }

  Future<void> disconnectAndExpectUnavailableWithoutPageChanges() async {
    await _complete(_bridge!.close());
    final process = await _runCli('실행되지 않을 입력');
    expect(process.exitCode, isNot(0));
    expect(process.stderr.toString(), contains('attached to host $_endpoint'));
    final result = jsonDecode(process.stdout as String) as Map<String, Object?>;
    expect(result['ok'], isFalse);
    expect(
      (result['error'] as Map<String, Object?>)['code'],
      'controlled_embed_unavailable',
    );
    await _expectPageState(counter: 2, text: _cliInput);
  }

  Future<ProcessResult> _runCli(String text) => _complete(
    Process.run(
      _cliPath,
      [
        'call',
        _toolId,
        jsonEncode({'step': 1, 'text': text}),
        '--json',
      ],
      workingDirectory: _home.path,
      stdoutEncoding: utf8,
      stderrEncoding: utf8,
      environment: {
        _homeVariable: _home.path,
        _projectVariable: _projectDisabled,
      },
    ),
  );

  Future<void> _expectPageState({
    required int counter,
    required String text,
  }) async {
    Object? snapshot = await _complete(
      _initialEntry!.browser.runResultJs('''
JSON.stringify({
  counter: window.counter,
  step: document.getElementById('step').value,
  text: document.getElementById('text').value,
  echo: document.getElementById('echo').textContent,
  output: document.getElementById('counter').textContent
})
'''),
    );
    for (var step = 0; step < _snapshotDecodeLimit; step++) {
      if (snapshot is String) snapshot = jsonDecode(snapshot);
    }
    expect(snapshot, {
      'counter': counter,
      'step': '1',
      'text': text,
      'echo': text,
      'output': '$counter',
    });
    expect(_sessions.entries, hasLength(1));
    expect(_fixture!.pageLoads, 1);
    final stored = _initialEntry!.lastResult;
    expect(stored?.ok, isTrue);
    expect(stored?.primaryOutputId, 'counter');
    expect(stored?.outputs.last.id, 'counter');
    expect(stored?.outputs.last.kind, 'integer');
    expect(stored?.outputs.last.label, 'Count');
    expect(
      stored?.outputs.last.value,
      CanonicalOutputValue.integer(value: counter),
    );
    expect(
      stored?.outputs.first.value,
      CanonicalOutputValue.string(value: text),
    );
    expect(_initialEntry!.phase, ControlledEmbedSessionPhase.ready);
  }

  Future<T> _complete<T>(Future<T> operation) async {
    var completed = false;
    late T value;
    Object? failure;
    StackTrace? failureTrace;
    unawaited(
      operation.then<void>(
        (result) {
          value = result;
          completed = true;
        },
        onError: (Object error, StackTrace trace) {
          failure = error;
          failureTrace = trace;
          completed = true;
        },
      ),
    );
    final deadline = DateTime.now().add(_operationTimeout);
    while (!completed && DateTime.now().isBefore(deadline)) {
      await tester.pump(_frameStep);
    }
    if (!completed) {
      throw TimeoutException('Desktop/CLI WebView operation did not complete.');
    }
    if (failure case final error?) {
      Error.throwWithStackTrace(error, failureTrace!);
    }
    return value;
  }

  void _expectHostVisible() =>
      expect(find.byKey(kControlledEmbedSessionHostKey), findsOneWidget);

  void _expectHostNotVisible() =>
      expect(find.byKey(kControlledEmbedSessionHostKey), findsNothing);

  Future<void> close() async {
    await _bridge?.close();
    await tester.pumpWidget(const SizedBox.shrink());
    _expectHostNotVisible();
    await _sessions.close();
    await _fixture?.close();
    if (_rustInitialized) boot.shutdown();
  }
}
