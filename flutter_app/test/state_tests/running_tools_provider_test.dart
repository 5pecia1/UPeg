import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/running_tools_provider.dart';

void main() {
  test('keeps_running_state_when_only_one_of_two_runs_of_same_tool_ends', () {
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

  test('run_states_of_different_tools_end_independently', () {
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

  test('ending_an_already_ended_run_keeps_other_run_states', () {
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

  test('late_end_from_before_clear_does_not_release_new_run_state', () {
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

  test('exposed_running_tools_list_cannot_be_mutated_from_outside', () {
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

  test('rebuilt_provider_ignores_late_end_of_previous_lease', () {
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
