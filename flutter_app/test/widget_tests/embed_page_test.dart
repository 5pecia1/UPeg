import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/embed_page.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/embed_resolver_provider.dart';
import 'package:upeg/src/widgets/embed_iframe_factory.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/expanded_modal/webview_panel.dart';

import '../test_helpers/i18n_test_catalog.dart';

ToolDto _embedTool() => ToolDto(
  id: 'embed.test',
  toolkit: 'embed',
  label: 'Embed Test',
  description: '',
  tags: const [],
  inputFields: const [],
  outputFields: const [],
  pinKind: PinKindDto.embed,
  invoker: InvokerDto.embed,
  pegboardUnits: PegboardUnitsDto.u2,
  source: const SourceDto.static_(),
  requiresApproval: false,
  approvalSurfaces: const <String>[],
  effect: ToolEffectDto.unknown,
);

class _NoopIframeFactory extends IframeFactory {
  const _NoopIframeFactory();
  @override
  String register(String url, {void Function()? onLoad}) => 'noop-$url';
}

EmbedResolutionDto? _fakeResolveOk({
  required ToolId toolId,
  required ToolArgs args,
}) => const EmbedResolutionDto(url: 'https://example.test/embed');

EmbedResolutionDto? _fakeResolveNull({
  required ToolId toolId,
  required ToolArgs args,
}) => null;

void main() {
  final originalBuilder = desktopWebViewBuilder;
  setUp(() {
    embedIframeFactory = const _NoopIframeFactory();
    desktopWebViewBuilder = (url, userAgent, onControllerReady) =>
        SizedBox.expand(key: const Key('webview-panel-inappwebview'));
  });
  tearDown(() {
    embedIframeFactory = const DartUiWebIframeFactory();
    desktopWebViewBuilder = originalBuilder;
  });

  testWidgets('EmbedPage_calls_resolveEmbedUrl_once_on_mount', (tester) async {
    var calls = 0;
    EmbedResolutionDto? countingResolve({
      required ToolId toolId,
      required ToolArgs args,
    }) {
      calls++;
      return const EmbedResolutionDto(url: 'https://example.test/embed');
    }

    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          ...i18nTestOverrides,
          resolveEmbedFnProvider.overrideWithValue(countingResolve),
        ],
        child: MaterialApp(
          home: EmbedPage(
            tool: _embedTool(),
            debugTargetOverride: const WebViewTarget.inAppWebView(),
          ),
        ),
      ),
    );
    await tester.pump();
    expect(calls, 1);
  });

  testWidgets('EmbedPage_closes_on_Esc', (tester) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          ...i18nTestOverrides,
          resolveEmbedFnProvider.overrideWithValue(_fakeResolveOk),
        ],
        child: MaterialApp(
          home: Builder(
            builder: (ctx) => Scaffold(
              body: TextButton(
                onPressed: () => EmbedPage.open(ctx, _embedTool()),
                child: const Text('open'),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.text('open'));
    await tester.pumpAndSettle();
    expect(find.byType(EmbedPage), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.byType(EmbedPage), findsNothing);
  });

  testWidgets('EmbedPage_renders_hint_text_when_resolve_returns_null', (
    tester,
  ) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          ...i18nTestOverrides,
          resolveEmbedFnProvider.overrideWithValue(_fakeResolveNull),
        ],
        child: MaterialApp(
          home: EmbedPage(
            tool: _embedTool(),
            debugTargetOverride: const WebViewTarget.inAppWebView(),
          ),
        ),
      ),
    );
    await tester.pump();
    expect(find.textContaining('embed url not available'), findsOneWidget);
  });
}
