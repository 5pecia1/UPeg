/// Widget tests for the expanded-modal async Run flow (T6).
///
/// The modal dispatches through [dispatchStreamFnProvider], whose last
/// event is the canonical result. While a dispatch is in flight the Run
/// button shows a spinner and refuses to re-fire; the footer derives its
/// `source` line from the tool's real [SourceDto] instead of the old
/// hardcoded `static · #[upeg::tool]` claim.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';
import 'package:upeg/src/state/running_tools_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/inline/generic_inline_pin_body.dart';

import '../test_helpers/dispatch_stream_fixture.dart';
import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/tool_fixture.dart';

const _emptySuccess = CanonicalToolResult(ok: true, outputs: []);

final _toollessTool = fixtureToolDto(
  id: 'fixture.no_args',
  toolkit: 'fixture',
  label: 'no args',
  description: 'runs without fields',
  inputFields: const <InputFieldDto>[],
);

Widget _harness({required StubDispatch dispatch, ToolDto? tool}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      dispatchStreamFnProvider.overrideWithValue(stubDispatchStream(dispatch)),
    ],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: ExpandedModalPage(tool: tool ?? _toollessTool),
    ),
  );
}

void main() {
  group('ExpandedModalPage async run', () {
    testWidgets('실행_중_Run_버튼은_비활성화된다', (tester) async {
      final completer = Completer<CanonicalToolResult>();
      var calls = 0;
      await tester.pumpWidget(
        _harness(
          dispatch: ({required toolId, required args, required approve}) {
            calls++;
            return completer.future;
          },
        ),
      );

      await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
      await tester.pump();

      // The in-flight spinner is visible while the dispatch is pending.
      expect(
        find.byKey(const Key('expanded-modal-run-progress')),
        findsOneWidget,
      );

      // A second tap while running must not fire a second dispatch.
      await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
      await tester.pump();
      expect(calls, 1);

      // Settling the dispatch clears the loading state.
      completer.complete(_emptySuccess);
      await tester.pumpAndSettle();
      expect(
        find.byKey(const Key('expanded-modal-run-progress')),
        findsNothing,
      );
      expect(calls, 1);
    });

    testWidgets('실행이_끝나면_결과가_표시된다', (tester) async {
      await tester.pumpWidget(
        _harness(
          dispatch:
              ({required toolId, required args, required approve}) async =>
                  _emptySuccess,
        ),
      );

      await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
      await tester.pumpAndSettle();

      expect(find.byKey(const Key('expanded-modal-outcome')), findsOneWidget);
      expect(find.text('ok'), findsOneWidget);
    });

    testWidgets('inline과 modal 실행이 겹치면 하나가 끝나도 실행 중 상태를 유지한다', (tester) async {
      final inlineCompleter = Completer<CanonicalToolResult>();
      final modalCompleter = Completer<CanonicalToolResult>();
      var inlineCalls = 0;
      var modalCalls = 0;
      final tool = fixtureToolDto(
        id: 'fixture.shared_run',
        source: const SourceDto.manual(),
        inputFields: const <InputFieldDto>[
          InputFieldDto(
            key: 'value',
            label: 'Value',
            fieldType: InputFieldType.text(),
            required_: true,
          ),
        ],
      );
      final toolId = ToolId.parse(tool.id);
      // Both surfaces now share one dispatch seam, so the stub routes by
      // call order: the inline pin runs first, the modal second.
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          dispatchStreamFnProvider.overrideWithValue(
            stubDispatchStream(({
              required toolId,
              required args,
              required approve,
            }) {
              if (inlineCalls == 0) {
                inlineCalls += 1;
                return inlineCompleter.future;
              }
              modalCalls += 1;
              return modalCompleter.future;
            }),
          ),
        ],
      );
      addTearDown(container.dispose);
      final navigatorKey = GlobalKey<NavigatorState>();
      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            navigatorKey: navigatorKey,
            theme: UpegTheme.darkTheme(),
            home: Scaffold(
              body: SizedBox(
                width: 240,
                height: 200,
                child: GenericInlinePinBody(
                  tool: tool,
                  pinKey: (BoardKey.parse('dev'), toolId),
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pump();

      await tester.enterText(find.byKey(const Key('field-value')), 'inline');
      await tester.pump();
      await tester.tap(find.byKey(inlineRunButtonKey));
      await tester.pump();
      expect(inlineCalls, 1);
      expect(container.read(runningToolsProvider), contains(toolId));

      unawaited(
        navigatorKey.currentState!.push<void>(
          MaterialPageRoute<void>(
            builder: (_) => ExpandedModalPage(tool: tool),
          ),
        ),
      );
      await tester.pumpAndSettle();
      await tester.enterText(find.byKey(const Key('field-value')), 'modal');
      await tester.pump();
      await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
      await tester.pump();
      expect(modalCalls, 1);
      expect(container.read(runningToolsProvider), contains(toolId));

      modalCompleter.complete(_emptySuccess);
      await tester.pumpAndSettle();
      expect(container.read(runningToolsProvider), contains(toolId));

      inlineCompleter.complete(_emptySuccess);
      await tester.pump();
      expect(container.read(runningToolsProvider), isNot(contains(toolId)));
    });
  });

  group('ExpandedModalPage footer source', () {
    testWidgets('footer는_실제_invoker를_표시한다', (tester) async {
      final wasmTool = fixtureToolDto(
        id: 'fixture.wasm_tool',
        toolkit: 'fixture',
        label: 'wasm tool',
        invoker: InvokerDto.wasm,
        source: const SourceDto.manual(),
      );
      await tester.pumpWidget(
        _harness(
          dispatch:
              ({required toolId, required args, required approve}) async =>
                  _emptySuccess,
          tool: wasmTool,
        ),
      );

      // Real invoker + real source metadata, never the old false claim.
      // The source line is a RichText, so scan rich text explicitly.
      expect(find.text('invoker: wasm'), findsOneWidget);
      expect(find.text('source: manual', findRichText: true), findsOneWidget);
      expect(
        find.textContaining('#[upeg::tool]', findRichText: true),
        findsNothing,
      );
    });

    testWidgets('footer_source는_static_tool에서_static을_표시한다', (tester) async {
      final staticTool = fixtureToolDto(
        id: 'fixture.static_tool',
        toolkit: 'fixture',
        label: 'static tool',
        invoker: InvokerDto.static_,
        source: const SourceDto.static_(),
      );
      await tester.pumpWidget(
        _harness(
          dispatch:
              ({required toolId, required args, required approve}) async =>
                  _emptySuccess,
          tool: staticTool,
        ),
      );

      expect(find.text('source: static', findRichText: true), findsOneWidget);
      expect(
        find.textContaining('#[upeg::tool]', findRichText: true),
        findsNothing,
      );
    });
  });
}
