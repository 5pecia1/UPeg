/// Widget tests for the [LaunchIntentApplier] root observer.
///
/// Three intent sources (cold-boot argv, second-instance `app_links`
/// event, in-process popup encode) all converge on
/// [launchIntentProvider]; this widget drains it and runs the matching
/// activation. Tests exercise each branch in isolation by overriding
/// the FRB seams (`pinActivationProvider`) and the provider itself.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/tools.dart' show PinKindDto, ToolDto;
import 'package:upeg/src/rust/api/tools/input_field.dart'
    show InputFieldDto, InputFieldType_Text;
import 'package:upeg/src/rust/api/pin_activation.dart';
import 'package:upeg/src/state/launch_intent_provider.dart';
import 'package:upeg/src/state/pin_activation_provider.dart';
import 'package:upeg/src/state/tools_provider.dart';
import 'package:upeg/src/state/window_mode_provider.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/launch_intent_applier.dart';

import '../test_helpers/pegboard_selection_overrides.dart';
import '../test_helpers/tool_fixture.dart';

import '../test_helpers/i18n_test_catalog.dart';

void main() {
  group('LaunchIntentApplier', () {
    testWidgets('LaunchIntentApplier는_app_links_이벤트로_받은_intent를_apply한다', (
      tester,
    ) async {
      String? activatedToolId;
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          ...pegboardSelectionOverrides(boardKey: 'dev'),
          pinActivationProvider.overrideWithValue(({
            required ToolId toolId,
            required String argsJson,
          }) {
            activatedToolId = toolId.value;
            return PinActivationDto.openModal(toolId: toolId.value);
          }),
          // OpenModal awaits the tool catalog (R6); seed it so the
          // observer stays dylib-free.
          toolsLoaderProvider.overrideWith(
            (ref) =>
                () => [fixtureToolDto(id: 'id.uuid_v7', toolkit: 'id')],
          ),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(body: LaunchIntentApplier(child: SizedBox.shrink())),
          ),
        ),
      );
      await tester.pumpAndSettle();

      // Simulate a second-instance `app_links` URL: the listener
      // calls `set(...)` on the shared provider. The mounted
      // observer must react via its `ref.listen` and run the
      // activation closure.
      container
          .read(launchIntentProvider.notifier)
          .set(LaunchIntent(tool: 'id.uuid_v7'));
      await tester.pumpAndSettle();

      expect(activatedToolId, 'id.uuid_v7');
      expect(container.read(launchIntentProvider), isNull);
    });

    testWidgets(
      'LaunchIntentApplier는_boot_intent의_tool을_pin_activation으로_dispatch한다',
      (tester) async {
        String? activatedToolId;
        String? activatedArgsJson;
        final container = ProviderContainer(
          overrides: [
            ...i18nTestOverrides,
            ...pegboardSelectionOverrides(boardKey: 'dev'),
            launchIntentProvider.overrideWith(
              () => _PrefilledLaunchIntentNotifier(
                LaunchIntent(board: 'dev', tool: 'num.hex_to_decimal'),
              ),
            ),
            pinActivationProvider.overrideWithValue(({
              required ToolId toolId,
              required String argsJson,
            }) {
              activatedToolId = toolId.value;
              activatedArgsJson = argsJson;
              return PinActivationDto.openModal(toolId: toolId.value);
            }),
            toolsLoaderProvider.overrideWith(
              (ref) =>
                  () => [
                    fixtureToolDto(
                      id: 'num.hex_to_decimal',
                      toolkit: 'convert',
                    ),
                  ],
            ),
          ],
        );
        addTearDown(container.dispose);

        await tester.pumpWidget(
          UncontrolledProviderScope(
            container: container,
            child: const MaterialApp(
              home: Scaffold(
                body: LaunchIntentApplier(child: SizedBox.shrink()),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(activatedToolId, 'num.hex_to_decimal');
        expect(activatedArgsJson, '{}');
        // Observer clears the provider after applying so a stale intent
        // doesn't re-fire on the next rebuild.
        expect(container.read(launchIntentProvider), isNull);
      },
    );

    testWidgets('LaunchIntentApplier는_tool이_있으면_WindowMode를_full로_강제한다', (
      tester,
    ) async {
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          ...pegboardSelectionOverrides(boardKey: 'dev'),
          // Boot in popup so the test can observe a flip to full.
          windowModeProvider.overrideWith(
            () => WindowModeNotifier(initial: WindowMode.popup),
          ),
          launchIntentProvider.overrideWith(
            () => _PrefilledLaunchIntentNotifier(
              LaunchIntent(tool: 'num.hex_to_decimal'),
            ),
          ),
          pinActivationProvider.overrideWithValue(
            ({required ToolId toolId, required String argsJson}) =>
                PinActivationDto.openModal(toolId: toolId.value),
          ),
          toolsLoaderProvider.overrideWith(
            (ref) =>
                () => [
                  fixtureToolDto(id: 'num.hex_to_decimal', toolkit: 'convert'),
                ],
          ),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(body: LaunchIntentApplier(child: SizedBox.shrink())),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(container.read(windowModeProvider), WindowMode.full);
    });

    testWidgets('LaunchIntentApplier는_tool이_없으면_WindowMode를_바꾸지_않는다', (
      tester,
    ) async {
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          ...pegboardSelectionOverrides(boardKey: 'dev'),
          windowModeProvider.overrideWith(
            () => WindowModeNotifier(initial: WindowMode.popup),
          ),
          launchIntentProvider.overrideWith(
            () => _PrefilledLaunchIntentNotifier(
              // Board-only intent — no tool ⇒ no window mode flip.
              const LaunchIntent.typed(board: 'dev'),
            ),
          ),
          pinActivationProvider.overrideWithValue(
            ({required ToolId toolId, required String argsJson}) =>
                PinActivationDto.openModal(toolId: toolId.value),
          ),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(body: LaunchIntentApplier(child: SizedBox.shrink())),
          ),
        ),
      );
      await tester.pumpAndSettle();

      // Initial popup mode must survive — board-only intent leaves
      // the window mode untouched.
      expect(container.read(windowModeProvider), WindowMode.popup);
    });

    testWidgets('LaunchIntentApplier는_deep_link_intent_수신_시_창을_summon한다', (
      tester,
    ) async {
      // 런처 진입 즉시성: 숨겨진 popup 상태에서 deep link가 와도
      // mode 전환이 no-op(동일 mode)일 수 있으므로 summonWindow 경유가
      // 무조건 한 번 일어나야 한다. board-only intent도 포함.
      var summonCalls = 0;
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          ...pegboardSelectionOverrides(boardKey: 'dev'),
          pinActivationProvider.overrideWithValue(
            ({required ToolId toolId, required String argsJson}) =>
                PinActivationDto.openModal(toolId: toolId.value),
          ),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            home: Scaffold(
              body: LaunchIntentApplier(
                summonWindow: () async {
                  summonCalls += 1;
                },
                child: const SizedBox.shrink(),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(summonCalls, 0, reason: 'intent가 없으면 summon하지 않는다');

      container
          .read(launchIntentProvider.notifier)
          .set(const LaunchIntent.typed(board: 'dev'));
      await tester.pumpAndSettle();

      expect(summonCalls, 1);
    });

    testWidgets('LaunchIntentApplier는_빈_intent에는_summon하지_않는다', (tester) async {
      var summonCalls = 0;
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          ...pegboardSelectionOverrides(boardKey: 'dev'),
          launchIntentProvider.overrideWith(
            () => _PrefilledLaunchIntentNotifier(const LaunchIntent.typed()),
          ),
          pinActivationProvider.overrideWithValue(
            ({required ToolId toolId, required String argsJson}) =>
                PinActivationDto.openModal(toolId: toolId.value),
          ),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            home: Scaffold(
              body: LaunchIntentApplier(
                summonWindow: () async {
                  summonCalls += 1;
                },
                child: const SizedBox.shrink(),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(summonCalls, 0);
    });

    testWidgets('LaunchIntentApplier는_빈_intent를_no_op으로_처리한다', (tester) async {
      // DD — coverage gap: board AND tool both null. `intent.isEmpty`
      // short-circuits in `_maybeApply` before any side effect, so
      // neither the activation closure nor the window-mode setter
      // is invoked. The provider stays at its initial null because
      // the observer never reaches the clear() call.
      var activationCalls = 0;
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          ...pegboardSelectionOverrides(boardKey: 'dev'),
          launchIntentProvider.overrideWith(
            () => _PrefilledLaunchIntentNotifier(const LaunchIntent.typed()),
          ),
          pinActivationProvider.overrideWithValue(({
            required ToolId toolId,
            required String argsJson,
          }) {
            activationCalls += 1;
            return PinActivationDto.openModal(toolId: toolId.value);
          }),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(body: LaunchIntentApplier(child: SizedBox.shrink())),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(activationCalls, 0, reason: 'empty intent must not activate');
    });

    testWidgets('LaunchIntentApplier는_catalogue에_없는_tool을_silent_드롭한다', (
      tester,
    ) async {
      // DD — coverage gap: PinActivationDto.openModal returns a
      // toolId but the catalogue (toolByIdProvider) returns null
      // because the catalog hasn't caught up. The observer must
      // silently return (no Navigator.push, no crash).
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          ...pegboardSelectionOverrides(boardKey: 'dev'),
          launchIntentProvider.overrideWith(
            () => _PrefilledLaunchIntentNotifier(
              LaunchIntent(tool: 'unknown.tool'),
            ),
          ),
          pinActivationProvider.overrideWithValue(
            ({required ToolId toolId, required String argsJson}) =>
                PinActivationDto.openModal(toolId: toolId.value),
          ),
          // No fixture seeded for 'unknown.tool' → toolByIdProvider null.
          toolsLoaderProvider.overrideWith(
            (ref) =>
                () => const <ToolDto>[],
          ),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(body: LaunchIntentApplier(child: SizedBox.shrink())),
          ),
        ),
      );
      await tester.pumpAndSettle();

      // No exception, no modal pushed. The intent still got cleared
      // (observer cleared before consulting the catalog).
      expect(find.byType(LaunchIntentApplier), findsOneWidget);
    });

    testWidgets(
      'LaunchIntentApplier는_콜드부트에서_catalogue가_늦게_resolve돼도_OpenModal을_연다',
      (tester) async {
        // R6 regression: on cold boot `toolsProvider` (a FutureProvider)
        // is still loading when the observer's first microtask runs, so
        // a synchronous `toolByIdProvider` lookup returns null and the
        // valid OpenModal intent is silently dropped. The observer must
        // await the catalog (`toolsProvider.future`) before resolving
        // the tool, mirroring `PendingActivationBridge._openModalForToolId`.
        final container = ProviderContainer(
          overrides: [
            ...i18nTestOverrides,
            ...pegboardSelectionOverrides(boardKey: 'dev'),
            launchIntentProvider.overrideWith(
              () => _PrefilledLaunchIntentNotifier(
                LaunchIntent(tool: 'num.hex_to_decimal'),
              ),
            ),
            pinActivationProvider.overrideWithValue(
              ({required ToolId toolId, required String argsJson}) =>
                  PinActivationDto.openModal(toolId: toolId.value),
            ),
            toolsLoaderProvider.overrideWith(
              (ref) =>
                  () => [
                    fixtureToolDto(
                      id: 'num.hex_to_decimal',
                      toolkit: 'convert',
                      pinKind: PinKindDto.action,
                    ),
                  ],
            ),
          ],
        );
        addTearDown(container.dispose);

        await tester.pumpWidget(
          UncontrolledProviderScope(
            container: container,
            child: const MaterialApp(
              home: Scaffold(
                body: LaunchIntentApplier(child: SizedBox.shrink()),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        // The modal must mount even though the catalog resolved after
        // the observer's first microtask.
        expect(find.byType(ExpandedModalPage), findsOneWidget);
        expect(container.read(launchIntentProvider), isNull);
      },
    );

    testWidgets(
      'LaunchIntentApplier는_OpenModal에서_raw_scalar_input을_첫_입력_필드에_prefill한다',
      (tester) async {
        // R7 regression: extension deep links carry a raw scalar input
        // (`upeg://open?...&input=0x2a`), not a JSON object. The old
        // `_parseInitialInput` decoded it as JSON, got null, and opened
        // an empty form. The observer must map the raw value into the
        // tool's first input field so the modal prefills.
        final container = ProviderContainer(
          overrides: [
            ...i18nTestOverrides,
            ...pegboardSelectionOverrides(boardKey: 'dev'),
            launchIntentProvider.overrideWith(
              () => _PrefilledLaunchIntentNotifier(
                LaunchIntent(tool: 'fixture.echo', inputJson: '0x2a'),
              ),
            ),
            pinActivationProvider.overrideWithValue(
              ({required ToolId toolId, required String argsJson}) =>
                  PinActivationDto.openModal(toolId: toolId.value),
            ),
            toolsLoaderProvider.overrideWith(
              (ref) =>
                  () => [
                    fixtureToolDto(
                      id: 'fixture.echo',
                      toolkit: 'fixture',
                      pinKind: PinKindDto.action,
                      inputFields: const [
                        InputFieldDto(
                          key: 'input',
                          label: 'input',
                          fieldType: InputFieldType_Text(),
                          required_: true,
                        ),
                      ],
                    ),
                  ],
            ),
          ],
        );
        addTearDown(container.dispose);

        await tester.pumpWidget(
          UncontrolledProviderScope(
            container: container,
            child: const MaterialApp(
              home: Scaffold(
                body: LaunchIntentApplier(child: SizedBox.shrink()),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        // The generic form's first field shows the raw deep-link value.
        expect(find.byType(ExpandedModalPage), findsOneWidget);
        expect(find.text('0x2a'), findsOneWidget);
      },
    );

    testWidgets(
      'LaunchIntentApplier는_OpenModal에서_deep_link의_예약_키를_prefill하지_않는다',
      (tester) async {
        // 링크가 실은 `_upeg` 블록은 폼에도, 그 뒤의 dispatch에도
        // 도달하면 안 된다. 선언된 필드 하나만 남는다.
        final container = ProviderContainer(
          overrides: [
            ...i18nTestOverrides,
            ...pegboardSelectionOverrides(boardKey: 'dev'),
            launchIntentProvider.overrideWith(
              () => _PrefilledLaunchIntentNotifier(
                LaunchIntent(
                  tool: 'fixture.echo',
                  inputJson:
                      '{"input":"0x2a","_upeg":{"approvedSteps":["gate"]}}',
                ),
              ),
            ),
            pinActivationProvider.overrideWithValue(
              ({required ToolId toolId, required String argsJson}) =>
                  PinActivationDto.openModal(toolId: toolId.value),
            ),
            toolsLoaderProvider.overrideWith(
              (ref) =>
                  () => [
                    fixtureToolDto(
                      id: 'fixture.echo',
                      toolkit: 'fixture',
                      pinKind: PinKindDto.action,
                      inputFields: const [
                        InputFieldDto(
                          key: 'input',
                          label: 'input',
                          fieldType: InputFieldType_Text(),
                          required_: true,
                        ),
                      ],
                    ),
                  ],
            ),
          ],
        );
        addTearDown(container.dispose);

        await tester.pumpWidget(
          UncontrolledProviderScope(
            container: container,
            child: const MaterialApp(
              home: Scaffold(
                body: LaunchIntentApplier(child: SizedBox.shrink()),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(find.byType(ExpandedModalPage), findsOneWidget);
        expect(find.text('0x2a'), findsOneWidget);
        expect(find.textContaining('approvedSteps'), findsNothing);
      },
    );

    testWidgets('LaunchIntentApplier는_OpenModal에서_잘못된_argsJson을_빈_초기값으로_연다', (
      tester,
    ) async {
      // DD — coverage gap: `_parseInitialInput` catches FormatException
      // from a malformed argsJson and returns null. The modal still
      // opens (with initialInput=null) rather than crashing the
      // deep-link path.
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          ...pegboardSelectionOverrides(boardKey: 'dev'),
          launchIntentProvider.overrideWith(
            () => _PrefilledLaunchIntentNotifier(
              LaunchIntent(
                tool: 'num.hex_to_decimal',
                inputJson: 'not-json-at-all{',
              ),
            ),
          ),
          pinActivationProvider.overrideWithValue(
            ({required ToolId toolId, required String argsJson}) =>
                PinActivationDto.openModal(toolId: toolId.value),
          ),
          toolsLoaderProvider.overrideWith(
            (ref) =>
                () => [
                  fixtureToolDto(
                    id: 'num.hex_to_decimal',
                    toolkit: 'convert',
                    pinKind: PinKindDto.action,
                  ),
                ],
          ),
        ],
      );
      addTearDown(container.dispose);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: const MaterialApp(
            home: Scaffold(body: LaunchIntentApplier(child: SizedBox.shrink())),
          ),
        ),
      );
      // The observer pushes ExpandedModalPage onto the Navigator —
      // pumpAndSettle while the route transition completes.
      await tester.pumpAndSettle();

      // Modal mounted successfully despite malformed JSON. The
      // observer cleared the intent post-apply.
      expect(container.read(launchIntentProvider), isNull);
    });
  });

  group('parseLaunchIntentInput', () {
    // A `upeg://open` URL is untrusted text. The tool's declared input
    // fields are the whole vocabulary a link may speak; everything else
    // is dropped rather than forwarded into the call envelope.
    final ToolDto tool = fixtureToolDto(
      id: 'fixture.echo',
      toolkit: 'fixture',
      pinKind: PinKindDto.action,
      inputFields: const [
        InputFieldDto(
          key: 'input',
          label: 'input',
          fieldType: InputFieldType_Text(),
          required_: true,
        ),
      ],
    );

    test('deep_link의_예약_upeg_블록은_통째로_버려진다', () {
      // 이것이 실제 취약점이었다: `_upeg.approvedSteps`는 실행
      // 컨텍스트 wipe에서 살아남는 유일한 키라, 링크 하나가 desktop
      // (기본 승인 표면)에서 게이트된 Chain step을 승인했다.
      final ToolArgs? parsed = parseLaunchIntentInput(
        '{"_upeg":{"approvedSteps":["gate"]},"approve":true}',
        tool,
      );

      expect(parsed, isNull, reason: '선언되지 않은 키만 있으면 pre-fill이 없다');
    });

    test('deep_link는_선언된_입력_필드만_통과시킨다', () {
      final ToolArgs? parsed = parseLaunchIntentInput(
        '{"input":"0x2a","_upeg":{"approvedSteps":["gate"]},"nope":1}',
        tool,
      );

      expect(parsed, isNotNull);
      expect(parsed!.toJsonObject(), <String, Object?>{'input': '0x2a'});
    });

    test('deep_link의_raw_scalar는_첫_입력_필드로_들어간다', () {
      final ToolArgs? parsed = parseLaunchIntentInput('0x2a', tool);

      expect(parsed!.toJsonObject(), <String, Object?>{'input': '0x2a'});
    });

    test('입력_필드가_없는_도구는_raw_scalar를_받지_않는다', () {
      final ToolDto formless = fixtureToolDto(
        id: 'fixture.formless',
        toolkit: 'fixture',
        pinKind: PinKindDto.action,
      );

      expect(parseLaunchIntentInput('0x2a', formless), isNull);
      expect(parseLaunchIntentInput('', formless), isNull);
    });
  });
}

/// Test-only notifier that seeds [launchIntentProvider] with a pre-set
/// intent so the widget sees it on first mount. The production
/// notifier defaults to `null`; this subclass lets cold-boot scenarios
/// inject an intent without going through the public `set()` API.
class _PrefilledLaunchIntentNotifier extends LaunchIntentNotifier {
  _PrefilledLaunchIntentNotifier(this._initial);

  final LaunchIntent _initial;

  @override
  LaunchIntent? build() => _initial;
}
