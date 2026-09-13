import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';

void main() {
  group('ToolId', () {
    test('ToolId는_정규_full_id를_값_객체로_파싱한다', () {
      final id = ToolId.parse('num.hex_to_decimal');

      expect(id.value, 'num.hex_to_decimal');
      expect(id.toString(), 'num.hex_to_decimal');
    });

    test('ToolId는_도구킷_구분자가_없는_id를_거부한다', () {
      expect(() => ToolId.parse('hex_to_dec'), throwsFormatException);
    });

    test('ToolId는_앞뒤_공백이_있는_id를_거부한다', () {
      expect(() => ToolId.parse(' num.hex_to_decimal '), throwsFormatException);
    });

    test('ToolId는_같은_문자열이면_같은_값으로_비교된다', () {
      expect(ToolId.parse('id.uuid_v7'), ToolId.parse('id.uuid_v7'));
    });
  });

  group('BoardKey', () {
    test('BoardKey는_비어있지_않은_key를_값_객체로_파싱한다', () {
      final key = BoardKey.parse('dev');

      expect(key.value, 'dev');
      expect(key.toString(), 'dev');
    });

    test('BoardKey는_빈_key를_거부한다', () {
      expect(() => BoardKey.parse(''), throwsFormatException);
    });

    test('BoardKey는_앞뒤_공백이_있는_key를_거부한다', () {
      expect(() => BoardKey.parse(' dev '), throwsFormatException);
    });
  });
}
