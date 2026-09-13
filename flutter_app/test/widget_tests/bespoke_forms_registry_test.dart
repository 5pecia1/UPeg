/// Tests for the bespoke-forms registry lookup.
///
/// The registry's qualification rule is "live preview only" — every
/// other tool renders through the generic form. These tests pin that
/// rule so a regression (a tool forking a widget for a shortcut, a
/// default, or a copy button) lights up the suite.
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/widgets/expanded_modal/bespoke_forms/registry.dart';

void main() {
  group('bespokeRegistry', () {
    test('bespokeRegistry는_등록되지_않은_tool_id에_대해_null을_반환한다', () {
      expect(bespokeFor(ToolId.parse('definitely.not_registered')), isNull);
    });

    test('bespokeRegistry는_hex_to_dec_id를_해석한다', () {
      expect(bespokeFor(ToolId.parse(hexToDecToolId)), isNotNull);
    });

    test('bespokeRegistry는_라이브_미리보기_도구_하나만_등록한다', () {
      expect(bespokeRegistry.keys.map((id) => id.value), [hexToDecToolId]);
    });

    test('한번_실행하고_복사하는_도구들은_generic_form을_쓴다', () {
      for (final id in const [
        'id.uuid_v7',
        'id.uuid_v4',
        'id.nanoid',
        'security.password_generate',
      ]) {
        expect(
          bespokeFor(ToolId.parse(id)),
          isNull,
          reason: '$id는 generic form + host의 Run/복사로 충분하다',
        );
      }
    });
  });
}
