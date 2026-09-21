import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';

void main() {
  group('ToolId', () {
    test('ToolId_parses_canonical_full_id_into_value_object', () {
      final id = ToolId.parse('num.hex_to_decimal');

      expect(id.value, 'num.hex_to_decimal');
      expect(id.toString(), 'num.hex_to_decimal');
    });

    test('ToolId_rejects_id_without_toolkit_separator', () {
      expect(() => ToolId.parse('hex_to_dec'), throwsFormatException);
    });

    test('ToolId_rejects_id_with_surrounding_whitespace', () {
      expect(() => ToolId.parse(' num.hex_to_decimal '), throwsFormatException);
    });

    test('ToolId_compares_equal_for_identical_strings', () {
      expect(ToolId.parse('id.uuid_v7'), ToolId.parse('id.uuid_v7'));
    });
  });

  group('BoardKey', () {
    test('BoardKey_parses_non_empty_key_into_value_object', () {
      final key = BoardKey.parse('dev');

      expect(key.value, 'dev');
      expect(key.toString(), 'dev');
    });

    test('BoardKey_rejects_empty_key', () {
      expect(() => BoardKey.parse(''), throwsFormatException);
    });

    test('BoardKey_rejects_key_with_surrounding_whitespace', () {
      expect(() => BoardKey.parse(' dev '), throwsFormatException);
    });
  });
}
