import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/inline_draft_provider.dart';
import 'package:upeg/src/state/pin_provider.dart' show PinKey;
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

void main() {
  group('InlineDraftStore', () {
    test('같은 도구의 입력도 보드가 다르면 서로 격리한다', () {
      final store = InlineDraftStore();
      final toolId = ToolId.parse('text.pair');
      final firstKey = (BoardKey.parse('first'), toolId);
      final secondKey = (BoardKey.parse('second'), toolId);
      final draft = ToolArgs.fromJsonObject(const <String, Object?>{
        'left': '첫 번째 보드',
      });

      store.set(firstKey, draft);

      expect(store.read(firstKey), draft);
      expect(store.read(secondKey), isNull);
    });

    test('핀 식별자로 저장한 입력을 정확히 지운다', () {
      final store = InlineDraftStore();
      final PinKey pinKey = (BoardKey.parse('dev'), ToolId.parse('text.pair'));
      store.set(
        pinKey,
        ToolArgs.fromJsonObject(const <String, Object?>{'left': '임시 값'}),
      );

      store.clear(pinKey);

      expect(store.read(pinKey), isNull);
    });
  });
}
