/// Draft saves resolve their storage namespace from the process context at
/// the moment they run. Every project context change therefore has to write
/// pending drafts first, while the old namespace is still active.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/state/inline_draft_provider.dart';
import 'package:upeg/src/state/project_context_provider.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

import '../test_helpers/pegboard_selection_overrides.dart';

const _project = ProjectDefinition(
  root: '/project',
  name: 'Project',
  boardCount: 1,
  toolkits: [],
);
const _activation = ProjectActivation(project: _project);
final _pin = (BoardKey.parse('dev'), PinId.parse('pin-a'));
final _tool = ToolId.parse('text.pair');

/// Writes native-boundary calls into the same log as draft saves, so the
/// order between the two is observable.
final class _LoggingProjectApi implements ProjectContextApi {
  _LoggingProjectApi(this.log);

  final List<String> log;

  @override
  Future<ProjectDefinition?> current() async => null;

  @override
  Future<ProjectDefinition> validateRoot(String root) async => _project;

  @override
  Future<ProjectActivation> activate(String root) async {
    log.add('activate');
    return _activation;
  }

  @override
  Future<ProjectActivation> setToolChoice({
    required String root,
    required String toolId,
    required ProjectToolChoice choice,
  }) async {
    log.add('setToolChoice');
    return _activation;
  }

  @override
  Future<void> close() async => log.add('close');
}

/// A container whose store holds one unsaved edit (its save timer is pending).
ProviderContainer _containerWithPendingDraft(List<String> log) {
  final container = ProviderContainer(
    overrides: [
      ...pegboardSelectionOverrides(),
      projectContextApiProvider.overrideWithValue(_LoggingProjectApi(log)),
      pinDraftLoadProvider.overrideWithValue((_) => const []),
      pinDraftSaveProvider.overrideWithValue(
        (pin, _, _) => log.add('save ${pin.$2.value}'),
      ),
    ],
  );
  addTearDown(container.dispose);
  container
      .read(inlineDraftProvider)
      .set(
        _pin,
        ToolArgs.fromJsonObject(const <String, Object?>{'left': 'typed'}),
        toolId: _tool,
      );
  return container;
}

void main() {
  group('ProjectContextNotifier draft flush', () {
    test('activate_saves_pending_drafts_before_the_native_call', () async {
      final log = <String>[];
      final container = _containerWithPendingDraft(log);

      await container
          .read(projectContextProvider.notifier)
          .activate('/project');

      expect(log, ['save pin-a', 'activate']);
    });

    test('setToolChoice_saves_pending_drafts_before_the_native_call', () async {
      final log = <String>[];
      final container = _containerWithPendingDraft(log);

      await container
          .read(projectContextProvider.notifier)
          .setToolChoice(
            root: '/project',
            toolId: 'text.pair',
            choice: ProjectToolChoice.project,
          );

      expect(log, ['save pin-a', 'setToolChoice']);
    });

    test('close_saves_pending_drafts_before_the_native_call', () async {
      final log = <String>[];
      final container = _containerWithPendingDraft(log);

      await container.read(projectContextProvider.notifier).close();

      expect(log, ['save pin-a', 'close']);
    });
  });
}
