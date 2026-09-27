/// Native FRB/SQLite restart probe for two instances of one tool.
///
/// Run this file twice in separate Linux Flutter processes against one fresh
/// UPEG_HOME, first with UPEG_PIN_RESTART_PHASE=create, then with reload.
/// UPEG_PROJECT_MANIFEST_PATH=off isolates the built-in tool catalog.
/// From flutter_app/:
///   lab=$(mktemp -d /tmp/upeg-pin-restart.XXXXXXXX)
///   for phase in create reload; do
///     UPEG_HOME="$lab" UPEG_PROJECT_MANIFEST_PATH=off \
///       UPEG_PIN_RESTART_PHASE="$phase" xvfb-run -a flutter test \
///       integration_test/duplicate_pin_restart_integration_test.dart -d linux
///   done
library;

import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:upeg/src/rust/api/boot.dart' as boot;
import 'package:upeg/src/rust/api/last_outcomes.dart' as outcomes;
import 'package:upeg/src/rust/api/pegboard.dart' as pegboard;
import 'package:upeg/src/rust/api/tools.dart' as tools;
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/rust/frb_generated.dart';

const _toolId = 'num.hex_to_decimal';
const _phaseVariable = 'UPEG_PIN_RESTART_PHASE';
const _boardTitle = 'Native duplicate pin restart probe';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  test(
    'should preserve independent pin presets and outcomes across a native restart',
    () async {
      final home = Directory(Platform.environment['UPEG_HOME']!);
      final fixture = File('${home.path}/pin-restart-fixture.json');
      final phase = Platform.environment[_phaseVariable]!;

      await RustLib.init();
      try {
        await boot.initApp();
        if (phase == 'create') {
          expect(fixture.existsSync(), isFalse);
          final boardKey = pegboard.createBoard(title: _boardTitle);
          final first = pegboard.addPin(boardKey: boardKey, toolId: _toolId);
          final second = pegboard.addPin(boardKey: boardKey, toolId: _toolId);
          expect(first, isNot(second));

          pegboard.setPinArgsPreset(
            boardKey: boardKey,
            pinId: first,
            presetJson: '{"input":"0x10"}',
          );
          pegboard.setPinArgsPreset(
            boardKey: boardKey,
            pinId: second,
            presetJson: '{"input":"0x20"}',
          );
          _expectPins(boardKey, first, second);
          _expectDispatch(boardKey, first, '16');
          _expectDispatch(boardKey, second, '32');
          _expectStoredOutcomes(boardKey, first, second);

          fixture.writeAsStringSync(
            jsonEncode({
              'boardKey': boardKey,
              'first': first,
              'second': second,
            }),
            flush: true,
          );
        } else {
          final saved =
              jsonDecode(fixture.readAsStringSync()) as Map<String, dynamic>;
          final boardKey = saved['boardKey'] as String;
          final first = saved['first'] as String;
          final second = saved['second'] as String;
          expect(
            pegboard.listBoards().where((board) => board.key == boardKey),
            hasLength(1),
          );
          _expectPins(boardKey, first, second);
          _expectStoredOutcomes(boardKey, first, second);
          _expectDispatch(boardKey, first, '16');
          _expectDispatch(boardKey, second, '32');
        }
      } finally {
        boot.shutdown();
        RustLib.dispose();
      }
    },
    skip: _environmentProblem(),
  );
}

String? _environmentProblem() {
  if (!Platform.isLinux) return 'Requires the native Linux Flutter runner.';
  if (Platform.environment['UPEG_PROJECT_MANIFEST_PATH'] != 'off') {
    return 'Set UPEG_PROJECT_MANIFEST_PATH=off.';
  }
  if ((Platform.environment['UPEG_HOME'] ?? '').isEmpty) {
    return 'Set UPEG_HOME to a fresh temporary directory.';
  }
  if (!{'create', 'reload'}.contains(Platform.environment[_phaseVariable])) {
    return 'Set $_phaseVariable=create or reload.';
  }
  return null;
}

void _expectPins(String boardKey, String first, String second) {
  final pins = pegboard
      .loadLayoutSnapshot(boardKey: boardKey)
      .placements
      .where((pin) => pin.toolId == _toolId)
      .toList();
  expect(pins, hasLength(2));
  expect(
    {for (final pin in pins) pin.pinId: pin.argsPresetJson},
    {first: '{"input":"0x10"}', second: '{"input":"0x20"}'},
  );
}

void _expectDispatch(String boardKey, String pinId, String expected) {
  final result = tools.dispatchTool(
    toolId: _toolId,
    argsJson: '{}',
    boardKey: boardKey,
    pinId: pinId,
    approve: false,
  );
  expect(result.ok, isTrue);
  expect(result.primaryOutputText.trim(), expected);
  outcomes.recordLastOutcome(
    boardKey: boardKey,
    pinId: pinId,
    toolId: _toolId,
    result: result,
  );
}

void _expectStoredOutcomes(String boardKey, String first, String second) {
  final stored = outcomes.loadLastOutcomes(boardKey: boardKey);
  expect(
    {
      for (final outcome in stored)
        outcome.pinId: outcome.result.primaryOutputText.trim(),
    },
    {first: '16', second: '32'},
  );
}
