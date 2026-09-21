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
    test('bespokeRegistry_returns_null_for_an_unregistered_tool_id', () {
      expect(bespokeFor(ToolId.parse('definitely.not_registered')), isNull);
    });

    test('bespokeRegistry_resolves_the_hex_to_dec_id', () {
      expect(bespokeFor(ToolId.parse(hexToDecToolId)), isNotNull);
    });

    test('bespokeRegistry_registers_only_the_one_live_preview_tool', () {
      expect(bespokeRegistry.keys.map((id) => id.value), [hexToDecToolId]);
    });

    test('run_once_and_copy_tools_use_the_generic_form', () {
      for (final id in const [
        'id.uuid_v7',
        'id.uuid_v4',
        'id.nanoid',
        'security.password_generate',
      ]) {
        expect(
          bespokeFor(ToolId.parse(id)),
          isNull,
          reason: '$id is fully served by the generic form + host Run/copy',
        );
      }
    });
  });
}
