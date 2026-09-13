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
    test('즉시_dispatch_판정은_인라인_실행으로_라우팅한다', () {
      final route = decidePopupActivationRoute(
        activation: const PinActivationDto.dispatchImmediate(
          toolId: 'id.uuid_v7',
        ),
        tool: fixtureToolDto(id: 'id.uuid_v7'),
      );

      expect(route, isA<PopupRunInline>());
      expect(route.toolId, ToolId.parse('id.uuid_v7'));
    });

    test('폼_필요_판정은_full_전환으로_라우팅한다', () {
      final route = decidePopupActivationRoute(
        activation: const PinActivationDto.openModal(
          toolId: 'num.hex_to_decimal',
        ),
        tool: fixtureToolDto(id: 'num.hex_to_decimal'),
      );

      expect(route, isA<PopupOpenFull>());
      expect(route.toolId, ToolId.parse('num.hex_to_decimal'));
    });

    test('embed_판정은_full_전환으로_라우팅한다', () {
      final route = decidePopupActivationRoute(
        activation: const PinActivationDto.openEmbed(toolId: 'web.docs'),
        tool: fixtureToolDto(id: 'web.docs', pinKind: PinKindDto.embed),
      );

      expect(route, isA<PopupOpenFull>());
      expect(route.toolId, ToolId.parse('web.docs'));
    });

    test('메모_생성_액션은_인라인_대신_full로_라우팅한다', () {
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

    test('프로바이더_미설정_도구는_인라인_대신_full로_라우팅한다', () {
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

    test('카탈로그_미해석_상태에서도_즉시_dispatch는_인라인으로_라우팅한다', () {
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
