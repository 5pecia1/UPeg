library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/features/controlled_embed/execution_bridge.dart';
import 'package:upeg/src/features/controlled_embed/session_service.dart';
import 'package:upeg/src/features/controlled_embed/webview_session.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

final controlledEmbedNativeSupportedProvider = Provider<bool>(
  (ref) => !kIsWeb && defaultTargetPlatform != TargetPlatform.fuchsia,
);

final controlledEmbedSessionFactoryProvider =
    Provider<ControlledEmbedSessionFactory>(
      (ref) => NativeControlledEmbedWebViewSession.create,
    );

final controlledEmbedSessionsProvider = Provider<ControlledEmbedSessionService>(
  (ref) {
    final service = ControlledEmbedSessionService(
      createSession: ref.watch(controlledEmbedSessionFactoryProvider),
      normalizeResult: normalizeControlledEmbedResult,
    );
    ref.onDispose(() => unawaited(service.close()));
    return service;
  },
);

final controlledEmbedExecutionApiProvider = Provider<WebViewExecutionApi>(
  (ref) => const FrbWebViewExecutionApi(),
);

final controlledEmbedBridgeProvider = Provider<ControlledEmbedExecutionBridge>((
  ref,
) {
  final bridge = ControlledEmbedExecutionBridge(
    sessions: ref.watch(controlledEmbedSessionsProvider),
    api: ref.watch(controlledEmbedExecutionApiProvider),
  );
  ref.onDispose(() => unawaited(bridge.close()));
  return bridge;
});

typedef ControlledEmbedToolExecutor =
    Future<CanonicalToolResult> Function({
      required ToolId toolId,
      required ToolArgs args,
      String? boardKey,
      String? pinId,
    });

/// Async is required: the Rust worker waits for this Dart isolate's WebView.
/// The Rust dispatcher applies board presets, validates the call and converts
/// raw browser values according to the tool's canonical output declaration.
final controlledEmbedToolExecutorProvider =
    Provider<ControlledEmbedToolExecutor>(
      (ref) => ({required toolId, required args, boardKey, pinId}) async {
        await ref.read(controlledEmbedBridgeProvider).ready;
        return dispatchToolAsync(
          toolId: toolId.value,
          argsJson: args.encodeJson(),
          boardKey: boardKey,
          pinId: pinId,
          approve: false,
        );
      },
    );
