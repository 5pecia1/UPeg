import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/running_tools_provider.dart';

void main() {
  test('같은 도구의 두 실행 중 하나만 끝나면 실행 중 상태를 유지한다', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final toolId = ToolId.parse('text.pair');
    final runningTools = container.read(runningToolsProvider.notifier);

    final firstLease = runningTools.begin(toolId);
    final secondLease = runningTools.begin(toolId);
    runningTools.end(firstLease);

    expect(container.read(runningToolsProvider), contains(toolId));

    runningTools.end(secondLease);
    expect(container.read(runningToolsProvider), isNot(contains(toolId)));
  });

  test('서로 다른 도구의 실행 상태는 각각 독립적으로 끝난다', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final firstToolId = ToolId.parse('text.pair');
    final secondToolId = ToolId.parse('num.sum');
    final runningTools = container.read(runningToolsProvider.notifier);
    final firstLease = runningTools.begin(firstToolId);
    final secondLease = runningTools.begin(secondToolId);

    runningTools.end(firstLease);

    expect(container.read(runningToolsProvider), isNot(contains(firstToolId)));
    expect(container.read(runningToolsProvider), contains(secondToolId));

    runningTools.end(secondLease);
    expect(container.read(runningToolsProvider), isEmpty);
  });

  test('이미 끝난 실행을 다시 끝내도 다른 실행 상태는 유지한다', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final toolId = ToolId.parse('text.pair');
    final runningTools = container.read(runningToolsProvider.notifier);
    final firstLease = runningTools.begin(toolId);
    final secondLease = runningTools.begin(toolId);

    runningTools.end(firstLease);
    runningTools.end(firstLease);

    expect(container.read(runningToolsProvider), contains(toolId));

    runningTools.end(secondLease);
    expect(container.read(runningToolsProvider), isEmpty);
  });

  test('초기화 이전의 늦은 종료는 새 실행 상태를 해제하지 않는다', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final toolId = ToolId.parse('text.pair');
    final runningTools = container.read(runningToolsProvider.notifier);
    final staleLease = runningTools.begin(toolId);

    runningTools.clear();
    final currentLease = runningTools.begin(toolId);
    runningTools.end(staleLease);

    expect(container.read(runningToolsProvider), contains(toolId));

    runningTools.end(currentLease);
    expect(container.read(runningToolsProvider), isEmpty);
  });

  test('공개된 실행 중 도구 목록은 외부에서 변경할 수 없다', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final toolId = ToolId.parse('text.pair');
    final runningTools = container.read(runningToolsProvider.notifier);
    final lease = runningTools.begin(toolId);
    final exposedState = container.read(runningToolsProvider);

    expect(exposedState, contains(toolId));
    expect(exposedState.clear, throwsUnsupportedError);
    expect(container.read(runningToolsProvider), contains(toolId));

    runningTools.end(lease);
    expect(container.read(runningToolsProvider), isEmpty);
  });

  test('provider를 다시 빌드하면 이전 lease의 늦은 종료를 무시한다', () {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    final toolId = ToolId.parse('text.pair');
    final runningTools = container.read(runningToolsProvider.notifier);
    final staleLease = runningTools.begin(toolId);

    container.invalidate(runningToolsProvider);
    expect(container.read(runningToolsProvider), isEmpty);
    final rebuiltRunningTools = container.read(runningToolsProvider.notifier);
    final currentLease = rebuiltRunningTools.begin(toolId);
    runningTools.end(staleLease);

    expect(container.read(runningToolsProvider), contains(toolId));

    rebuiltRunningTools.end(currentLease);
    expect(container.read(runningToolsProvider), isEmpty);
  });
}
