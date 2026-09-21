import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/features/host_attach/attach_client.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/i18n.dart' show LocaleDto;
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/live_outcome_provider.dart' show LiveDispatchFn;
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_canvas.dart';
import 'package:upeg/src/widgets/host_attach_notice_body.dart';
import 'package:upeg/src/widgets/inline/generic_inline_pin_body.dart';

import '../test_helpers/dispatch_stream_fixture.dart';
import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/tool_fixture.dart';

const _statusMessages = <String, Map<LocaleDto, String>>{
  'empty.pegboard.no_pins': {
    LocaleDto.en: 'no pins on this board yet',
    LocaleDto.ko: '이 보드에는 아직 핀이 없습니다',
  },
  'a11y.inline.running': {
    LocaleDto.en: 'Running tool',
    LocaleDto.ko: '도구 실행 중',
  },
  'inline.dispatch_failed': {
    LocaleDto.en: 'Unable to run this tool.',
    LocaleDto.ko: '이 도구를 실행할 수 없습니다.',
  },
};

TweaksDto _tweaks(LocaleDto locale) => TweaksDto(
  theme: 'Dark',
  accent: 'Cyan',
  showHoles: true,
  locale: switch (locale) {
    LocaleDto.en => 'En',
    LocaleDto.ko => 'Ko',
  },
  localHttpHost: false,
);

final class _LocalizedStatusRobot {
  _LocalizedStatusRobot(this.tester);

  final WidgetTester tester;
  late final ProviderContainer _container;

  Future<void> _pump(Widget child, {LiveDispatchFn? dispatch}) async {
    _container = ProviderContainer(
      overrides: [
        ...i18nTestOverrides,
        tweaksLoaderProvider.overrideWithValue(() => _tweaks(LocaleDto.en)),
        tweaksSaverProvider.overrideWithValue((_) {}),
        if (dispatch != null) ...dispatchOverrides(dispatch),
      ],
    );
    addTearDown(_container.dispose);
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: _container,
        child: MaterialApp(
          theme: UpegTheme.darkTheme(),
          home: Scaffold(body: SizedBox(width: 360, height: 200, child: child)),
        ),
      ),
    );
    await tester.pumpAndSettle();
  }

  Future<void> pumpEmptyBoard() => _pump(
    debugBoardCanvasGrid(
      snapshot: const LayoutSnapshotDto(
        boardKey: 'dev',
        boardCols: 6,
        placements: <PlacementDto>[],
      ),
      onPinTap: (_) {},
    ),
  );

  Future<void> pumpInline(LiveDispatchFn dispatch) {
    final tool = fixtureToolDto(
      id: 'fixture.localized_status',
      description: 'Exposes dispatch status for localization tests.',
      source: const SourceDto.manual(),
    );
    return _pump(
      GenericInlinePinBody(
        tool: tool,
        pinKey: (BoardKey.parse('dev'), ToolId.parse(tool.id)),
      ),
      dispatch: dispatch,
    );
  }

  Future<void> pumpNotice(AttachDispatchResult result) =>
      _pump(HostAttachNoticeBody(notice: hostAttachNoticeFor(result)!));

  Future<void> setLocale(LocaleDto locale) async {
    await _container.read(tweaksProvider.notifier).save(_tweaks(locale));
    await tester.pump();
  }

  Future<void> run() async {
    await tester.tap(find.byKey(inlineRunButtonKey));
    await tester.pump();
  }

  Future<void> settle() => tester.pumpAndSettle();

  void expectTextVisible(String text) {
    expect(find.text(text), findsOneWidget);
  }

  void expectTextNotVisible(String text) {
    expect(find.text(text), findsNothing);
  }

  void expectRunningSemanticsVisible(String label) {
    expect(find.byKey(inlineLoadingIndicatorKey), findsOneWidget);
    expect(find.bySemanticsLabel(label), findsOneWidget);
    final semantics = tester.widget<Semantics>(
      find.byWidgetPredicate(
        (widget) => widget is Semantics && widget.properties.label == label,
      ),
    );
    expect(semantics.properties.liveRegion, isTrue);
  }

  void expectRunningSemanticsNotVisible(String label) {
    expect(find.bySemanticsLabel(label), findsNothing);
  }

  void expectLoadingNotVisible() {
    expect(find.byKey(inlineLoadingIndicatorKey), findsNothing);
  }
}

void main() {
  test('should keep exact English fallbacks and complete EN/KO fake rows', () {
    expect(emptyBoardHint, 'no pins on this board yet');
    expect(inlineSafeDispatchErrorMessage, 'Unable to run this tool.');
    expect(kHostUnavailableDefaultHint, 'host temporarily unavailable');
    for (final entry in _statusMessages.entries) {
      expect(i18nTestCatalog[entry.key], entry.value, reason: entry.key);
    }
  });

  testWidgets('should localize the empty board hint when the locale changes', (
    tester,
  ) async {
    final robot = _LocalizedStatusRobot(tester);
    await robot.pumpEmptyBoard();
    robot.expectTextVisible(emptyBoardHint);
    robot.expectTextNotVisible('이 보드에는 아직 핀이 없습니다');

    await robot.setLocale(LocaleDto.ko);
    robot.expectTextVisible('이 보드에는 아직 핀이 없습니다');
    robot.expectTextNotVisible(emptyBoardHint);

    await robot.setLocale(LocaleDto.en);
    robot.expectTextVisible(emptyBoardHint);
    robot.expectTextNotVisible('이 보드에는 아직 핀이 없습니다');
  });

  testWidgets(
    'should localize live running semantics without restarting the run',
    (tester) async {
      final pending = Completer<CanonicalToolResult>();
      var calls = 0;
      final robot = _LocalizedStatusRobot(tester);
      await robot.pumpInline(({required toolId, required args}) {
        calls += 1;
        return pending.future;
      });
      try {
        await robot.run();
        robot.expectRunningSemanticsVisible('Running tool');
        robot.expectRunningSemanticsNotVisible('도구 실행 중');

        await robot.setLocale(LocaleDto.ko);
        robot.expectRunningSemanticsVisible('도구 실행 중');
        robot.expectRunningSemanticsNotVisible('Running tool');
        expect(calls, 1);
      } finally {
        pending.complete(
          const CanonicalToolResult(
            ok: true,
            outputs: <CanonicalOutputEntry>[],
          ),
        );
        await robot.settle();
      }
      robot.expectLoadingNotVisible();
      robot.expectRunningSemanticsNotVisible('도구 실행 중');
      robot.expectRunningSemanticsNotVisible('Running tool');
    },
  );

  testWidgets(
    'should localize a safe dispatch failure only when presenting it',
    (tester) async {
      const privateDetail = '/private/localized-status.txt';
      final robot = _LocalizedStatusRobot(tester);
      await robot.pumpInline(({required toolId, required args}) async {
        throw Exception(privateDetail);
      });
      await robot.run();
      await robot.settle();
      robot.expectTextVisible(inlineSafeDispatchErrorMessage);
      robot.expectTextNotVisible(privateDetail);
      robot.expectLoadingNotVisible();

      await robot.setLocale(LocaleDto.ko);
      robot.expectTextVisible('이 도구를 실행할 수 없습니다.');
      robot.expectTextNotVisible(inlineSafeDispatchErrorMessage);
      robot.expectTextNotVisible(privateDetail);

      await robot.setLocale(LocaleDto.en);
      robot.expectTextVisible(inlineSafeDispatchErrorMessage);
      robot.expectTextNotVisible('이 도구를 실행할 수 없습니다.');
    },
  );

  const externalFailures =
      <
        ({
          String name,
          AttachDispatchResult result,
          String message,
          String labelKey,
        })
      >[
        (
          name: 'server-authored unavailable hints',
          result: AttachDispatchUnavailable(
            'Server maintenance: retry in 30 seconds.',
          ),
          message: 'Server maintenance: retry in 30 seconds.',
          labelKey: kHostAttachUnreachableLabelKey,
        ),
        (
          name: 'tool-authored error messages',
          result: AttachDispatchToolError(
            CanonicalToolError(
              code: 'external.invalid_input',
              message: 'The tool rejected field customer_id.',
            ),
          ),
          message: 'The tool rejected field customer_id.',
          labelKey: kHostAttachToolErrorLabelKey,
        ),
        (
          name: 'tool-authored messages that resemble catalog keys',
          result: AttachDispatchToolError(
            CanonicalToolError(
              code: 'external.custom_error',
              message: 'inline.dispatch_failed',
            ),
          ),
          message: 'inline.dispatch_failed',
          labelKey: kHostAttachToolErrorLabelKey,
        ),
      ];
  for (final failure in externalFailures) {
    testWidgets('should preserve ${failure.name} across locale changes', (
      tester,
    ) async {
      final robot = _LocalizedStatusRobot(tester);
      await robot.pumpNotice(failure.result);
      robot.expectTextVisible(i18nEn(failure.labelKey));
      robot.expectTextVisible(failure.message);

      await robot.setLocale(LocaleDto.ko);
      robot.expectTextVisible(i18nKo(failure.labelKey));
      robot.expectTextNotVisible(i18nEn(failure.labelKey));
      robot.expectTextVisible(failure.message);
    });
  }
}
