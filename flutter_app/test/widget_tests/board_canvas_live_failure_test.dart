/// A Live pin whose polling has never succeeded shows the safe failure line
/// (the Inline pin's copy) in its body instead of looking idle; a pin with an
/// earlier success keeps that result, marked stale.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart' show ToolId;
import 'package:upeg/src/platform/window_visibility.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/capability_provider.dart';
import 'package:upeg/src/state/live_outcome_provider.dart'
    show LiveDispatchFn, liveDispatchToolFnProvider;
import 'package:upeg/src/state/pin_provider.dart' show PinKey;
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_canvas.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart' show ToolArgs;
import 'package:upeg/src/widgets/pin.dart' show PinTapCallback;

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/pin_persistence_overrides.dart';
import '../test_helpers/tool_fixture.dart';

const String _toolId = 'demo.live_failure';

/// Stands in for a backend message that can carry a path or URL.
const String _rawError = '/private/raw-error.txt';

const Duration _pollInterval = Duration(seconds: 60);

final String _failureCopy = i18nEn('inline.dispatch_failed');

CanonicalToolResult _failure() => const CanonicalToolResult(
  ok: false,
  outputs: <CanonicalOutputEntry>[],
  error: CanonicalToolError(code: 'dispatch_failed', message: _rawError),
);

CanonicalToolResult _success(String value) => CanonicalToolResult(
  ok: true,
  primaryOutputId: 'result',
  outputs: <CanonicalOutputEntry>[
    CanonicalOutputEntry(
      id: 'result',
      kind: 'string',
      value: CanonicalOutputValue.string(value: value),
    ),
  ],
);

/// Answers [outcomes] in order, then repeats the last one, counting polls.
class _ScriptedDispatch {
  _ScriptedDispatch(this.outcomes);

  final List<CanonicalToolResult> outcomes;
  int calls = 0;

  Future<CanonicalToolResult> call({
    PinKey? pinKey,
    required ToolId toolId,
    required ToolArgs args,
  }) async {
    final index = calls < outcomes.length ? calls : outcomes.length - 1;
    calls += 1;
    return outcomes[index];
  }
}

Future<void> _pumpLiveBoard(
  WidgetTester tester, {
  required LiveDispatchFn dispatch,
  PinTapCallback? onPinTap,
}) async {
  final tool = fixtureToolDto(
    id: _toolId,
    label: 'Live probe',
    pinKind: PinKindDto.live,
    source: SourceDto.timer(
      intervalMs: BigInt.from(_pollInterval.inMilliseconds),
    ),
  );
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        ...i18nTestOverrides,
        ...pinPersistenceTestOverrides,
        // Never reach the real `windowManager` from a test.
        windowVisibilitySourceProvider.overrideWithValue(null),
        toolsLoaderProvider.overrideWith(
          (ref) =>
              () => [tool],
        ),
        liveDispatchToolFnProvider.overrideWithValue(dispatch),
        isWasmRuntimeProvider.overrideWithValue(false),
      ],
      child: MaterialApp(
        theme: UpegTheme.darkTheme(),
        home: Scaffold(
          body: SizedBox(
            width: 1200,
            height: 600,
            child: debugBoardCanvasGrid(
              snapshot: const LayoutSnapshotDto(
                boardKey: 'dev',
                boardCols: 6,
                placements: <PlacementDto>[
                  PlacementDto(
                    toolId: _toolId,
                    pinId: _toolId,
                    x: 0,
                    y: 0,
                    w: 1,
                    h: 1,
                  ),
                ],
              ),
              onPinTap: onPinTap ?? (_) {},
            ),
          ),
        ),
      ),
    ),
  );
  // Load the tool catalog, then let the cold-start poll land.
  await tester.pumpAndSettle();
}

void main() {
  group('Live pin failure body', () {
    testWidgets('a_first_failure_shows_the_safe_copy_and_hides_the_raw_error', (
      tester,
    ) async {
      await _pumpLiveBoard(
        tester,
        dispatch: ({pinKey, required toolId, required args}) async =>
            _failure(),
      );

      expect(find.text(_failureCopy), findsOneWidget);
      expect(find.textContaining(_rawError), findsNothing);
    });

    testWidgets('the_failure_copy_reaches_assistive_technology', (
      tester,
    ) async {
      final semantics = tester.ensureSemantics();
      await _pumpLiveBoard(
        tester,
        dispatch: ({pinKey, required toolId, required args}) async =>
            _failure(),
      );

      expect(find.bySemanticsLabel(_failureCopy), findsOneWidget);
      semantics.dispose();
    });

    testWidgets('tapping_the_failure_body_runs_the_pin_again', (tester) async {
      final taps = <PlacementDto>[];
      await _pumpLiveBoard(
        tester,
        dispatch: ({pinKey, required toolId, required args}) async =>
            _failure(),
        onPinTap: taps.add,
      );

      await tester.tap(find.text(_failureCopy));

      expect(taps.map((placement) => placement.pinId), [_toolId]);
    });

    testWidgets('an_earlier_success_keeps_the_stale_result_not_the_failure', (
      tester,
    ) async {
      final script = _ScriptedDispatch([_success('42'), _failure()]);
      await _pumpLiveBoard(tester, dispatch: script.call);
      expect(find.text('42'), findsOneWidget);

      await tester.pump(_pollInterval);
      await tester.pump();

      expect(script.calls, 2, reason: 'the second poll is the failing one');
      expect(find.text('42'), findsOneWidget);
      expect(find.text(_failureCopy), findsNothing);
    });

    testWidgets('a_later_success_replaces_the_failure_copy', (tester) async {
      final script = _ScriptedDispatch([_failure(), _success('7')]);
      await _pumpLiveBoard(tester, dispatch: script.call);
      expect(find.text(_failureCopy), findsOneWidget);

      await tester.pump(_pollInterval);
      await tester.pump();

      expect(find.text(_failureCopy), findsNothing);
      expect(find.text('7'), findsOneWidget);
    });
  });
}
