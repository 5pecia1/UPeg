/// Unit tests for the pure popup activation routing
/// (lib/src/popup/popup_activation_route.dart): which `pinActivationFor`
/// verdicts run inline inside the popup and which switch to the full
/// dashboard.
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/popup/popup_activation_route.dart';
import 'package:upeg/src/rust/api/pin_activation.dart';
import 'package:upeg/src/rust/api/tools.dart';

import '../test_helpers/tool_fixture.dart';

void main() {
  group('decidePopupActivationRoute', () {
    test('immediate_dispatch_verdict_routes_to_inline_run', () {
      final route = decidePopupActivationRoute(
        activation: const PinActivationDto.dispatchImmediate(
          toolId: 'id.uuid_v7',
        ),
        tool: fixtureToolDto(id: 'id.uuid_v7'),
      );

      expect(route, isA<PopupRunInline>());
      expect(route.toolId, ToolId.parse('id.uuid_v7'));
    });

    test('form_required_verdict_routes_to_full_switch', () {
      final route = decidePopupActivationRoute(
        activation: const PinActivationDto.openModal(
          toolId: 'num.hex_to_decimal',
        ),
        tool: fixtureToolDto(id: 'num.hex_to_decimal'),
      );

      expect(route, isA<PopupOpenFull>());
      expect(route.toolId, ToolId.parse('num.hex_to_decimal'));
    });

    test('embed_verdict_routes_to_full_switch', () {
      final route = decidePopupActivationRoute(
        activation: const PinActivationDto.openEmbed(toolId: 'web.docs'),
        tool: fixtureToolDto(id: 'web.docs', pinKind: PinKindDto.embed),
      );

      expect(route, isA<PopupOpenFull>());
      expect(route.toolId, ToolId.parse('web.docs'));
    });

    test('memo_create_action_routes_to_full_instead_of_inline', () {
      // memo.create shape: Action pin with a keyboard-shortcut source —
      // no headless dispatcher, the board owns the side effect.
      final route = decidePopupActivationRoute(
        activation: const PinActivationDto.dispatchImmediate(
          toolId: 'memo.create',
        ),
        tool: fixtureToolDto(
          id: 'memo.create',
          pinKind: PinKindDto.action,
          source: const SourceDto.shortcut(keys: 'Cmd+Shift+N'),
        ),
      );

      expect(route, isA<PopupOpenFull>());
    });

    test('tool_without_provider_routes_to_full_instead_of_inline', () {
      // eth.gas shape: Live pin, http invoker, static source — cannot
      // actually run, so the full surface renders the honest state.
      final route = decidePopupActivationRoute(
        activation: const PinActivationDto.dispatchImmediate(toolId: 'eth.gas'),
        tool: fixtureToolDto(
          id: 'eth.gas',
          pinKind: PinKindDto.live,
          invoker: InvokerDto.http,
          source: const SourceDto.static_(),
        ),
      );

      expect(route, isA<PopupOpenFull>());
    });

    test('immediate_dispatch_routes_inline_even_when_catalog_unresolved', () {
      final route = decidePopupActivationRoute(
        activation: const PinActivationDto.dispatchImmediate(
          toolId: 'id.uuid_v7',
        ),
        tool: null,
      );

      expect(route, isA<PopupRunInline>());
    });
  });
}
