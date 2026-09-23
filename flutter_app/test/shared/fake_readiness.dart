import 'package:upeg/src/features/host_attach/attach_client.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

final class FakeReadinessAttachClient implements AttachClient {
  FakeReadinessAttachClient({
    this.readinessResult = const AttachReadinessNotApplicable(),
  });

  AttachReadinessResult readinessResult;
  int inspectCalls = 0;
  ToolId? lastToolId;
  String? lastBoardKey;

  @override
  Future<AttachReadinessResult> inspectReadiness({
    required ToolId toolId,
    String? boardKey,
  }) async {
    inspectCalls++;
    lastToolId = toolId;
    lastBoardKey = boardKey;
    return readinessResult;
  }

  @override
  Future<HealthzResult> checkHealth() async => const HealthzUnreachable();

  @override
  Future<AttachListResult> listTools() async => const AttachListUnreachable();

  @override
  Future<AttachDispatchResult> dispatch({
    required ToolId toolId,
    required ToolArgs args,
    String? boardKey,
  }) async => const AttachDispatchUnreachable();
}
