import 'dart:async';

import 'package:upeg/src/features/controlled_embed/execution_bridge.dart';
import 'package:upeg/src/rust/api/webview.dart';

class FakeWebViewExecutionApi implements WebViewExecutionApi {
  final providerId = BigInt.from(7);
  final eventsController = StreamController<WebViewExecutionEventDto>();
  final completions = <(BigInt, BigInt, WebViewExecutionCompletionDto)>[];
  final completionController =
      StreamController<WebViewExecutionCompletionDto>.broadcast();
  int unregisterCalls = 0;

  @override
  BigInt create() => providerId;

  @override
  Stream<WebViewExecutionEventDto> events(BigInt providerId) =>
      eventsController.stream;

  @override
  bool complete(
    BigInt providerId,
    BigInt requestId,
    WebViewExecutionCompletionDto result,
  ) {
    completions.add((providerId, requestId, result));
    completionController.add(result);
    return unregisterCalls == 0;
  }

  @override
  bool unregister(BigInt providerId) {
    unregisterCalls++;
    return true;
  }

  Future<void> close() async {
    await eventsController.close();
    await completionController.close();
  }
}
