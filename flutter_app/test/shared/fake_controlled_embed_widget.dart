import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:upeg/src/features/controlled_embed/providers.dart';
import 'package:upeg/src/features/controlled_embed/session_service.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/embed.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/widgets/controlled_embed/runner.dart';
import 'package:upeg/src/widgets/controlled_embed/settings.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

import 'fake_controlled_embed_session.dart';

typedef LegacyControlledEmbedToolExecutor =
    Future<CanonicalToolResult> Function({
      required ToolId toolId,
      required ToolArgs args,
      String? boardKey,
    });

/// Captures the public dispatcher boundary and uses the actual session service
/// when a test needs normal/debug page ownership and live execution events.
class ControlledEmbedWidgetFixture {
  ControlledEmbedWidgetFixture() {
    sessions = ControlledEmbedSessionService(
      createSession: factory.create,
      normalizeResult: (toolId, result, wait) =>
          normalizeResult(toolId, result, wait),
      runner: const ControlledEmbedRunner(settleDelay: Duration.zero),
    );
  }

  final factory = FakeControlledEmbedSessionFactory();
  late final ControlledEmbedSessionService sessions;
  final calls = <({ToolId toolId, ToolArgs args, String? boardKey})>[];
  LegacyControlledEmbedToolExecutor? executor;
  ControlledEmbedResultNormalizer normalizeResult = (_, result, _) => result;
  bool nativeSupported = true;
  String? boardKey;
  String url = 'https://example.test/';
  ResolvedBrowserSettings? settings;
  List<SelectorBindingDto> bindings = const [
    SelectorBindingDto(
      role: BindingRoleDto.trigger,
      field: '',
      selector: '#go',
      triggerAction: ControlledEmbedTriggerActionDto.click,
    ),
    SelectorBindingDto(
      role: BindingRoleDto.output,
      field: 'result',
      selector: '#result',
      triggerAction: ControlledEmbedTriggerActionDto.click,
    ),
  ];

  List<Override> get overrides => [
    controlledEmbedNativeSupportedProvider.overrideWithValue(nativeSupported),
    controlledEmbedSessionFactoryProvider.overrideWithValue(factory.create),
    controlledEmbedSessionsProvider.overrideWithValue(sessions),
    controlledEmbedToolExecutorProvider.overrideWithValue(run),
    currentBoardKeyProvider.overrideWith(
      () => _FixedCurrentBoardNotifier(boardKey),
    ),
  ];

  Future<CanonicalToolResult> run({
    required ToolId toolId,
    required ToolArgs args,
    String? boardKey,
    String? pinId,
  }) async {
    calls.add((toolId: toolId, args: args, boardKey: boardKey));
    final custom = executor;
    if (custom != null) {
      return custom(toolId: toolId, args: args, boardKey: boardKey);
    }
    final outcome = await sessions.execute(
      spec: ControlledEmbedSessionSpec(
        pinKey: boardKey == null || pinId == null
            ? null
            : (BoardKey.parse(boardKey), PinId.parse(pinId)),
        toolId: toolId,
        url: url,
        settings: settings,
      ),
      bindings: bindings,
      inputs: {
        for (final entry in args.toJsonObject().entries)
          entry.key: entry.value.toString(),
      },
      cancellation: ControlledEmbedCancellation(),
    );
    return outcome.canonicalResult;
  }
}

class _FixedCurrentBoardNotifier extends CurrentBoardNotifier {
  _FixedCurrentBoardNotifier(this.key);

  final String? key;

  @override
  BoardKey? build() => key == null ? null : BoardKey.parse(key!);
}

class RecordingClipboardWriterForTile extends ClipboardWriter {
  final writes = <String>[];

  @override
  Future<void> write(String text) async => writes.add(text);
}
