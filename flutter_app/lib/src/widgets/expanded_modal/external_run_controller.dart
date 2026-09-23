/// External-tool readiness and remote dispatch adapter for the modal.
library;

import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/features/host_attach/attach_client.dart';
import 'package:upeg/src/features/host_attach/host_attach_dispatch_provider.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/capability_provider.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/external_readiness_provider.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/host_attach_notice_body.dart';

const String _attachDispatchFailedCode = 'host_attach_dispatch_failed';

final class ExternalRunController {
  const ExternalRunController(this.ref);

  final WidgetRef ref;

  bool usesRemoteHost(ToolDto tool) =>
      tool.invoker == InvokerDto.external_ && ref.read(isWasmRuntimeProvider);

  Future<ExternalReadinessInspection> inspect(ToolDto tool) {
    if (tool.invoker != InvokerDto.external_) {
      return Future.value(const ExternalReadinessNotApplicable());
    }
    final target = readinessTargetFor(
      tool,
      boardKey: ref.read(currentBoardKeyProvider),
    );
    return ref.read(externalReadinessProvider(target).future);
  }

  Future<CanonicalToolResult> dispatchRemote(
    ToolDto tool,
    ToolArgs args,
  ) async {
    final result = await ref
        .read(hostAttachDispatchProvider.notifier)
        .run(ToolId.parse(tool.id), args);
    final outcome = switch (result) {
      AttachDispatchOk(:final result) => result,
      AttachDispatchToolError(:final error) => CanonicalToolResult(
        ok: false,
        outputs: const <CanonicalOutputEntry>[],
        error: error,
      ),
      _ => _failureFor(result),
    };
    invalidateReadinessOnMissing(tool, outcome);
    return outcome;
  }

  /// Refresh cached setup state when dispatch catches a prerequisite that
  /// changed after inspection. Invalidating never retries the command.
  void invalidateReadinessOnMissing(ToolDto tool, CanonicalToolResult outcome) {
    if (tool.invoker != InvokerDto.external_) return;
    final details = outcome.error?.details;
    if (details == null) return;
    try {
      final decoded = jsonDecode(details);
      if (decoded is! Map || decoded['readiness'] is! Map) return;
      final readiness = decoded['readiness'] as Map;
      if (readiness['status'] != 'missing_executable' &&
          readiness['status'] != 'missing_working_directory') {
        return;
      }
      final target = readinessTargetFor(
        tool,
        boardKey: ref.read(currentBoardKeyProvider),
      );
      ref.invalidate(externalReadinessProvider(target));
    } on FormatException {
      // Malformed tool-authored details remain a normal run failure.
    }
  }

  CanonicalToolResult _failureFor(AttachDispatchResult result) {
    final notice = hostAttachNoticeFor(result);
    final message = switch (notice) {
      HostAttachNotice(:final hintKey?) => tRead(ref, hintKey),
      HostAttachNotice(:final hintText?) => hintText,
      _ => tRead(ref, kHostAttachUnreachableHintKey),
    };
    return CanonicalToolResult(
      ok: false,
      outputs: const <CanonicalOutputEntry>[],
      error: CanonicalToolError(
        code: _attachDispatchFailedCode,
        message: message,
      ),
    );
  }
}
