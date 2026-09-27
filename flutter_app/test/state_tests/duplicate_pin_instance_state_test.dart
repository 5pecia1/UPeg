import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/inline_draft_provider.dart';
import 'package:upeg/src/state/running_tools_provider.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

void main() {
  test('same-tool placements keep draft and running state independent', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final board = BoardKey.parse('dev');
    final first = (board, PinId.parse('pin-a'));
    final second = (board, PinId.parse('pin-b'));
    final drafts = container.read(inlineDraftProvider);
    final running = container.read(runningToolsProvider.notifier);

    drafts.set(
      first,
      ToolArgs.fromJsonObject(const <String, Object?>{'left': 'first'}),
    );
    drafts.set(
      second,
      ToolArgs.fromJsonObject(const <String, Object?>{'left': 'second'}),
    );
    final firstLease = running.begin(first);

    expect(drafts.read(first)?.encodeJson(), contains('first'));
    expect(drafts.read(second)?.encodeJson(), contains('second'));
    expect(container.read(pinIsRunningProvider(first)), isTrue);
    expect(container.read(pinIsRunningProvider(second)), isFalse);

    running.end(firstLease);
    expect(container.read(pinIsRunningProvider(first)), isFalse);
  });
}
