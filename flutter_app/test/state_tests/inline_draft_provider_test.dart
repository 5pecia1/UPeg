import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/inline_draft_provider.dart';
import 'package:upeg/src/state/pin_provider.dart' show PinKey;
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

void main() {
  group('InlineDraftStore', () {
    test('isolates drafts for the same tool across different boards', () {
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

    test('clears exactly the draft stored under a pin key', () {
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
