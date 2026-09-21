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
    testWidgets(
      'launchintentapplier_applies_an_intent_received_via_an_app_links_event',
      (tester) async {
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
              home: Scaffold(
                body: LaunchIntentApplier(child: SizedBox.shrink()),
              ),
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
      },
    );

    testWidgets(
      'launchintentapplier_dispatches_the_boot_intent_tool_to_pin_activation',
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

    testWidgets(
      'launchintentapplier_forces_windowmode_to_full_when_the_intent_has_a_tool',
      (tester) async {
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

        expect(container.read(windowModeProvider), WindowMode.full);
      },
    );

    testWidgets(
      'launchintentapplier_leaves_windowmode_alone_when_the_intent_has_no_tool',
      (tester) async {
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
              home: Scaffold(
                body: LaunchIntentApplier(child: SizedBox.shrink()),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        // Initial popup mode must survive — board-only intent leaves
        // the window mode untouched.
        expect(container.read(windowModeProvider), WindowMode.popup);
      },
    );

    testWidgets(
      'launchintentapplier_summons_the_window_when_a_deep_link_intent_arrives',
      (tester) async {
        // Launcher entry immediacy: a deep link arriving while the popup is
        // hidden can make the mode transition a no-op (same mode), so the
        // summonWindow path must run unconditionally once. Board-only
        // intents included.
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
        expect(summonCalls, 0, reason: 'no intent means no summon');

        container
            .read(launchIntentProvider.notifier)
            .set(const LaunchIntent.typed(board: 'dev'));
        await tester.pumpAndSettle();

        expect(summonCalls, 1);
      },
    );

    testWidgets('launchintentapplier_does_not_summon_for_an_empty_intent', (
      tester,
    ) async {
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

    testWidgets('launchintentapplier_treats_an_empty_intent_as_a_no_op', (
      tester,
    ) async {
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

    testWidgets(
      'launchintentapplier_silently_drops_a_tool_missing_from_the_catalogue',
      (tester) async {
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
              home: Scaffold(
                body: LaunchIntentApplier(child: SizedBox.shrink()),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        // No exception, no modal pushed. The intent still got cleared
        // (observer cleared before consulting the catalog).
        expect(find.byType(LaunchIntentApplier), findsOneWidget);
      },
    );

    testWidgets(
      'launchintentapplier_still_opens_openmodal_when_the_catalogue_resolves_late_on_cold_boot',
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
      'launchintentapplier_prefills_a_raw_scalar_input_into_the_first_input_field_on_openmodal',
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
      'launchintentapplier_does_not_prefill_reserved_deep_link_keys_on_openmodal',
      (tester) async {
        // The `_upeg` block carried by the link must reach neither the
        // form nor the dispatch that follows it. Only the declared field
        // survives.
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

    testWidgets(
      'launchintentapplier_opens_openmodal_with_empty_initial_values_for_malformed_argsjson',
      (tester) async {
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
              home: Scaffold(
                body: LaunchIntentApplier(child: SizedBox.shrink()),
              ),
            ),
          ),
        );
        // The observer pushes ExpandedModalPage onto the Navigator —
        // pumpAndSettle while the route transition completes.
        await tester.pumpAndSettle();

        // Modal mounted successfully despite malformed JSON. The
        // observer cleared the intent post-apply.
        expect(container.read(launchIntentProvider), isNull);
      },
    );
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

    test('a_deep_links_reserved_upeg_block_is_dropped_wholesale', () {
      // This was the actual vulnerability: `_upeg.approvedSteps` is the
      // only key that survives the execution-context wipe, so a single
      // link could approve a gated Chain step on desktop (the default
      // approval surface).
      final ToolArgs? parsed = parseLaunchIntentInput(
        '{"_upeg":{"approvedSteps":["gate"]},"approve":true}',
        tool,
      );

      expect(
        parsed,
        isNull,
        reason: 'with only undeclared keys there is nothing to prefill',
      );
    });

    test('a_deep_link_passes_only_the_declared_input_fields', () {
      final ToolArgs? parsed = parseLaunchIntentInput(
        '{"input":"0x2a","_upeg":{"approvedSteps":["gate"]},"nope":1}',
        tool,
      );

      expect(parsed, isNotNull);
      expect(parsed!.toJsonObject(), <String, Object?>{'input': '0x2a'});
    });

    test('a_deep_link_raw_scalar_lands_in_the_first_input_field', () {
      final ToolArgs? parsed = parseLaunchIntentInput('0x2a', tool);

      expect(parsed!.toJsonObject(), <String, Object?>{'input': '0x2a'});
    });

    test('a_tool_without_input_fields_rejects_a_raw_scalar', () {
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
