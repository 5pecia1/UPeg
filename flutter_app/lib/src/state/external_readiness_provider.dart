/// Cached, non-executing readiness inspection for External tools.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/features/host_attach/attach_client.dart';
import 'package:upeg/src/features/host_attach/host_attach_config.dart';
import 'package:upeg/src/features/host_attach/host_attach_config_provider.dart';
import 'package:upeg/src/features/host_attach/host_attach_dispatch_provider.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/readiness.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/capability_provider.dart';

typedef LocalReadinessInspector =
    Future<ExternalReadinessDto?> Function({
      required String toolId,
      String? boardKey,
    });

final localReadinessInspectorProvider = Provider<LocalReadinessInspector>(
  (ref) => inspectToolReadiness,
);

class HostReadinessUnreachable implements Exception {
  const HostReadinessUnreachable();
}

class HostReadinessUnauthorized implements Exception {
  const HostReadinessUnauthorized();
}

class HostReadinessMalformed implements Exception {
  const HostReadinessMalformed();
}

typedef ExternalReadinessTarget = ({
  ToolId toolId,
  bool isExternal,
  String? boardKey,
});

ExternalReadinessTarget readinessTargetFor(
  ToolDto tool, {
  BoardKey? boardKey,
}) => (
  toolId: ToolId.parse(tool.id),
  isExternal: tool.invoker == InvokerDto.external_,
  boardKey: boardKey?.value,
);

ExternalReadinessTarget externalReadinessTarget(ToolId toolId) =>
    (toolId: toolId, isExternal: true, boardKey: null);

sealed class ExternalReadinessInspection {
  const ExternalReadinessInspection();
}

final class ExternalReadinessNotApplicable extends ExternalReadinessInspection {
  const ExternalReadinessNotApplicable();
}

final class ExternalReadinessHostUnpaired extends ExternalReadinessInspection {
  const ExternalReadinessHostUnpaired();
}

final class ExternalReadinessInspected extends ExternalReadinessInspection {
  const ExternalReadinessInspected(this.readiness);

  final ExternalReadinessDto readiness;

  bool get blocksRun =>
      readiness.status == ExternalReadinessStatusDto.missingExecutable ||
      readiness.status == ExternalReadinessStatusDto.missingWorkingDirectory;
}

final externalReadinessProvider =
    FutureProvider.family<ExternalReadinessInspection, ExternalReadinessTarget>(
      (ref, target) async {
        if (!target.isExternal) return const ExternalReadinessNotApplicable();
        if (!ref.watch(isWasmRuntimeProvider)) {
          final readiness = await ref.read(localReadinessInspectorProvider)(
            toolId: target.toolId.value,
            boardKey: target.boardKey,
          );
          return readiness == null
              ? const ExternalReadinessNotApplicable()
              : ExternalReadinessInspected(readiness);
        }
        if (!ref.watch(hostAttachConfigProvider).isConfigured) {
          return const ExternalReadinessHostUnpaired();
        }
        final result = await ref
            .read(attachClientProvider)
            .inspectReadiness(toolId: target.toolId, boardKey: target.boardKey);
        return _inspectionFromHost(result);
      },
      retry: (_, _) => null,
    );

/// Cached readiness for the paired host's global inventory. It intentionally
/// ignores the selected board and the Flutter process platform.
typedef HostExternalReadinessTarget = ({
  ToolId toolId,
  HostAttachConfig config,
});

final hostExternalReadinessProvider =
    FutureProvider.family<
      ExternalReadinessInspection,
      HostExternalReadinessTarget
    >((ref, target) async {
      if (!target.config.isConfigured) {
        return const ExternalReadinessHostUnpaired();
      }
      final result = await ref
          .read(attachClientProvider)
          .inspectReadiness(toolId: target.toolId);
      return _inspectionFromHost(result);
    }, retry: (_, _) => null);

ExternalReadinessInspection _inspectionFromHost(AttachReadinessResult result) =>
    switch (result) {
      AttachReadinessOk(:final readiness) => ExternalReadinessInspected(
        readiness,
      ),
      AttachReadinessNotApplicable() => const ExternalReadinessNotApplicable(),
      AttachReadinessUnreachable() => throw const HostReadinessUnreachable(),
      AttachReadinessUnauthorized() => throw const HostReadinessUnauthorized(),
      AttachReadinessMalformed() => throw const HostReadinessMalformed(),
    };

void recheckExternalReadiness(WidgetRef ref, ExternalReadinessTarget target) {
  ref.invalidate(externalReadinessProvider(target));
}
