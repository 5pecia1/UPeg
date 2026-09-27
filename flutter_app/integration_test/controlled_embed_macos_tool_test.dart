/// macOS real-WebView integration probe for controlled-embed built-in-tool
/// execution.
///
/// Uses deterministic `data:` URLs only — no network dependency. Tests skip
/// gracefully on non-macOS hosts via early `return` pattern.
///
/// Requires `RustLib.init()` in `setUpAll` because the production runner
/// calls FRB `buildExecutionScripts`.
library;

import 'dart:convert';
import 'dart:io' as io;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/rust/frb_generated.dart';
import 'package:upeg/src/state/selector_bindings_provider.dart';
import 'package:upeg/src/widgets/controlled_embed/tile.dart';

/// Test-local constants.
const kSuccessText = 'OK';
const kInvalidSelector = '[';
const kRunButtonKey = Key('controlled-embed-run-btn');
const kErrorKey = Key('controlled-embed-error');

/// Build a deterministic `data:` URL from raw HTML body.
String _dataUrl(String body) {
  return Uri.dataFromString(
    body,
    mimeType: 'text/html',
    encoding: utf8,
  ).toString();
}

/// Fixture tool for controlled-embed probes.
ToolDto _fixtureTool() => const ToolDto(
  id: 'embed.macos_controlled_probe',
  toolkit: 'embed',
  label: 'macOS Controlled Probe',
  description: 'Integration probe for controlled-embed on macOS',
  tags: <String>[],
  pinKind: PinKindDto.controlledEmbed,
  pegboardUnits: PegboardUnitsDto.u2,
  invoker: InvokerDto.embed,
  inputFields: <InputFieldDto>[],
  outputFields: <OutputFieldDto>[],
  source: SourceDto.manual(),
  requiresApproval: false,
  approvalSurfaces: <String>[],
  effect: ToolEffectDto.unknown,
);

/// Fixture resolution wrapping a data URL.
EmbedResolutionDto _fixtureResolution(String url) =>
    EmbedResolutionDto(url: url);

/// Pump a ControlledEmbedTile into a testable harness.
Future<void> _pumpTile(
  WidgetTester tester,
  String url, {
  List<SelectorBindingDto>? bindings,
}) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        selectorBindingsLoaderProvider.overrideWithValue(
          (ToolId _) => bindings ?? const <SelectorBindingDto>[],
        ),
      ],
      child: MaterialApp(
        home: Scaffold(
          body: ControlledEmbedTile(
            pinKey: (
              BoardKey.parse('integration'),
              PinId.parse(_fixtureTool().id),
            ),
            tool: _fixtureTool(),
            resolution: _fixtureResolution(url),
          ),
        ),
      ),
    ),
  );
}

/// File-local robot for controlled-embed macOS integration probes.
final class _ControlledEmbedMacosRobot {
  _ControlledEmbedMacosRobot(this.tester);
  final WidgetTester tester;

  Future<void> pumpTile(
    String url, {
    List<SelectorBindingDto>? bindings,
  }) async {
    await _pumpTile(tester, url, bindings: bindings);
  }

  Future<void> tapRunAndWait() async {
    await tester.pump(const Duration(seconds: 1));
    await tester.tap(find.byKey(kRunButtonKey));
    await tester.pump();
    await tester.pump(const Duration(seconds: 2));
  }

  void expectOutputContains(String text) {
    expect(find.byKey(const Key('controlled-embed-outputs')), findsOneWidget);
    expect(find.textContaining(text), findsOneWidget);
  }

  void expectErrorMatches(String text) {
    expect(find.byKey(kErrorKey), findsOneWidget);
    expect(find.textContaining(text), findsOneWidget);
  }

  void expectNoError() {
    expect(find.byKey(kErrorKey), findsNothing);
  }

  void expectTextAbsent(String text) {
    expect(find.textContaining(text), findsNothing);
  }

  Future<void> resetTree() async {
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump();
  }
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  setUpAll(() async {
    if (io.Platform.isMacOS) {
      await RustLib.init();
    }
  });

  group('macOS real WebView controlled-embed', () {
    testWidgets('macOS_real_WebView_runs_a_data_uri_tool_and_displays_output', (
      tester,
    ) async {
      if (!io.Platform.isMacOS) {
        debugPrint(
          'skipping macOS controlled embed probe on ${io.Platform.operatingSystem}',
        );
        return;
      }

      final robot = _ControlledEmbedMacosRobot(tester);

      // data: URL with a button that sets window._peg_output and a target div.
      final url = _dataUrl('''
<!DOCTYPE html>
<html>
<body>
  <button id="trigger" onclick="document.getElementById('result').textContent='OK';">go</button>
  <div id="result"></div>
</body>
</html>
''');

      final bindings = <SelectorBindingDto>[
        const SelectorBindingDto(
          role: BindingRoleDto.trigger,
          field: '',
          selector: '#trigger',
          triggerAction: ControlledEmbedTriggerActionDto.click,
        ),
        const SelectorBindingDto(
          role: BindingRoleDto.output,
          field: 'result',
          selector: '#result',
          triggerAction: ControlledEmbedTriggerActionDto.click,
        ),
      ];

      await robot.pumpTile(url, bindings: bindings);
      await robot.tapRunAndWait();
      robot.expectOutputContains(kSuccessText);
      // Verify trigger-output pattern works without null-result error
      // (trigger returns undefined, but runner uses fire-and-forget mode)
      robot.expectTextAbsent('Result of JavaScript execution returned');
    });

    testWidgets(
      'macOS_real_WebView_shows_an_invalid_selector_exception_in_the_error_strip',
      (tester) async {
        if (!io.Platform.isMacOS) {
          debugPrint(
            'skipping macOS controlled embed probe on ${io.Platform.operatingSystem}',
          );
          return;
        }

        final robot = _ControlledEmbedMacosRobot(tester);

        // data: URL with minimal body — the trigger selector '[' is invalid CSS,
        // forcing a JS error at querySelector time.
        final url = _dataUrl('''
<!DOCTYPE html>
<html><body><div>hello</div></body></html>
''');

        final bindings = <SelectorBindingDto>[
          const SelectorBindingDto(
            role: BindingRoleDto.trigger,
            field: '',
            selector: kInvalidSelector,
            triggerAction: ControlledEmbedTriggerActionDto.click,
          ),
        ];

        await robot.pumpTile(url, bindings: bindings);
        await robot.tapRunAndWait();
        robot.expectErrorMatches(kInvalidSelector);
      },
    );

    testWidgets(
      'macOS_real_WebView_recovers_outputs_with_a_successful_rerun_after_an_error',
      (tester) async {
        if (!io.Platform.isMacOS) {
          debugPrint(
            'skipping macOS controlled embed probe on ${io.Platform.operatingSystem}',
          );
          return;
        }

        final robot = _ControlledEmbedMacosRobot(tester);

        // Phase 1: invalid selector to force error.
        final badUrl = _dataUrl('''
<!DOCTYPE html>
<html><body><div>hello</div></body></html>
''');

        final badBindings = <SelectorBindingDto>[
          const SelectorBindingDto(
            role: BindingRoleDto.trigger,
            field: '',
            selector: kInvalidSelector,
            triggerAction: ControlledEmbedTriggerActionDto.click,
          ),
        ];

        await robot.pumpTile(badUrl, bindings: badBindings);
        await robot.tapRunAndWait();
        robot.expectErrorMatches(kInvalidSelector);

        // Reset the tree to mount a fresh tile with valid bindings.
        await robot.resetTree();

        // Phase 2: valid selector to verify recovery.
        final goodUrl = _dataUrl('''
<!DOCTYPE html>
<html>
<body>
  <button id="trigger" onclick="document.getElementById('result').textContent='OK';">go</button>
  <div id="result"></div>
</body>
</html>
''');

        final goodBindings = <SelectorBindingDto>[
          const SelectorBindingDto(
            role: BindingRoleDto.trigger,
            field: '',
            selector: '#trigger',
            triggerAction: ControlledEmbedTriggerActionDto.click,
          ),
          const SelectorBindingDto(
            role: BindingRoleDto.output,
            field: 'result',
            selector: '#result',
            triggerAction: ControlledEmbedTriggerActionDto.click,
          ),
        ];

        await robot.pumpTile(goodUrl, bindings: goodBindings);
        await robot.tapRunAndWait();
        robot.expectNoError();
        robot.expectOutputContains(kSuccessText);
      },
    );
  });
}
