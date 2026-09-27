/// Active project context for the desktop UI.
///
/// The native layer owns discovery, manifest parsing, tool conflict policy,
/// and runtime replacement.  Flutter only presents that state and asks the
/// native layer to validate, activate, close, or persist an explicit choice.
/// Keeping this seam injectable makes the UI testable before FRB bindings are
/// regenerated for the project API.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/rust/api/project.dart' as frb;
import 'package:upeg/src/state/boards_provider.dart';
import 'package:upeg/src/state/current_board_provider.dart';
import 'package:upeg/src/state/layout_provider.dart';
import 'package:upeg/src/state/tag_provider.dart';
import 'package:upeg/src/state/tools_provider.dart';

enum ProjectToolChoice { global, project }

final class ProjectToolConflict {
  const ProjectToolConflict({
    required this.toolId,
    required this.globalSource,
    required this.projectSource,
    this.selected,
  });

  final String toolId;
  final String globalSource;
  final String projectSource;
  final ProjectToolChoice? selected;

  bool get needsChoice => selected == null;

  ProjectToolConflict withChoice(ProjectToolChoice value) =>
      ProjectToolConflict(
        toolId: toolId,
        globalSource: globalSource,
        projectSource: projectSource,
        selected: value,
      );
}

final class ProjectDefinition {
  const ProjectDefinition({
    required this.root,
    required this.name,
    required this.boardCount,
    required this.toolkits,
    this.conflicts = const [],
  });

  final String root;
  final String name;
  final int boardCount;
  final List<String> toolkits;
  final List<ProjectToolConflict> conflicts;
}

final class ProjectActivation {
  const ProjectActivation({
    required this.project,
    this.loadedToolIds = const [],
    this.failed = const [],
  });

  final ProjectDefinition project;
  final List<String> loadedToolIds;
  final List<String> failed;
}

/// Typed native boundary. Tests replace this provider with a small in-memory
/// fake while production delegates to FRB below.
abstract interface class ProjectContextApi {
  Future<ProjectDefinition?> current();
  Future<ProjectDefinition> validateRoot(String root);
  Future<ProjectActivation> activate(String root);
  Future<ProjectActivation> setToolChoice({
    required String root,
    required String toolId,
    required ProjectToolChoice choice,
  });
  Future<void> close();
}

final projectContextApiProvider = Provider<ProjectContextApi>(
  (ref) => const FrbProjectContextApi(),
);

/// Production FRB adapter. All mutation and validation remain native; Dart
/// only maps generated transport DTOs into the UI's compact presentation
/// models.
final class FrbProjectContextApi implements ProjectContextApi {
  const FrbProjectContextApi();

  @override
  Future<ProjectDefinition?> current() async {
    final dto = frb.currentProjectDefinition();
    return dto == null ? null : _definition(dto);
  }

  @override
  Future<ProjectDefinition> validateRoot(String root) async =>
      _definition(frb.validateProjectRoot(root: root));

  @override
  Future<ProjectActivation> activate(String root) async {
    final activation = frb.activateProject(root: root);
    return _activation(activation);
  }

  @override
  Future<void> close() async => frb.closeProject();

  @override
  Future<ProjectActivation> setToolChoice({
    required String root,
    required String toolId,
    required ProjectToolChoice choice,
  }) async => _activation(
    frb.setProjectToolChoice(
      root: root,
      toolId: toolId,
      choice: _choiceDto(choice),
    ),
  );

  ProjectActivation _activation(frb.ProjectActivationDto activation) {
    // A successful native activation always installs its definition. Reading
    // it here gives the UI the complete board/toolkit details without
    // duplicating that state in the activation transport DTO.
    final project = frb.currentProjectDefinition();
    if (project == null) {
      throw StateError(
        'Native project activation completed without a context.',
      );
    }
    return ProjectActivation(
      project: _definition(project),
      loadedToolIds: activation.loadedToolIds,
      failed: activation.failed,
    );
  }
}

ProjectDefinition _definition(frb.ProjectDefinitionDto dto) =>
    ProjectDefinition(
      root: dto.root,
      name: dto.name,
      boardCount: dto.boardCount,
      toolkits: dto.toolkits
          .map((toolkit) => toolkit.path)
          .toList(growable: false),
      conflicts: dto.conflicts.map(_conflict).toList(growable: false),
    );

ProjectToolConflict _conflict(frb.ProjectConflictDto dto) =>
    ProjectToolConflict(
      toolId: dto.toolId,
      globalSource: dto.globalSource,
      projectSource: dto.projectSource,
      selected: switch (dto.choice) {
        frb.ProjectToolChoiceDto.global => ProjectToolChoice.global,
        frb.ProjectToolChoiceDto.project => ProjectToolChoice.project,
        null => null,
      },
    );

frb.ProjectToolChoiceDto _choiceDto(ProjectToolChoice choice) =>
    switch (choice) {
      ProjectToolChoice.global => frb.ProjectToolChoiceDto.global,
      ProjectToolChoice.project => frb.ProjectToolChoiceDto.project,
    };

/// The currently active project, or `null` for the global desktop context.
final projectContextProvider =
    AsyncNotifierProvider<ProjectContextNotifier, ProjectDefinition?>(
      ProjectContextNotifier.new,
    );

class ProjectContextNotifier extends AsyncNotifier<ProjectDefinition?> {
  @override
  Future<ProjectDefinition?> build() =>
      ref.watch(projectContextApiProvider).current();

  Future<ProjectDefinition> validateRoot(String root) {
    return ref.read(projectContextApiProvider).validateRoot(root);
  }

  Future<ProjectActivation> activate(String root) async {
    final activation = await ref.read(projectContextApiProvider).activate(root);
    state = AsyncData(activation.project);
    _refreshProjectBoundViews();
    return activation;
  }

  Future<ProjectActivation> setToolChoice({
    required String root,
    required String toolId,
    required ProjectToolChoice choice,
  }) async {
    final activation = await ref
        .read(projectContextApiProvider)
        .setToolChoice(root: root, toolId: toolId, choice: choice);
    state = AsyncData(activation.project);
    _refreshProjectBoundViews();
    return activation;
  }

  Future<void> close() async {
    await ref.read(projectContextApiProvider).close();
    state = const AsyncData(null);
    _refreshProjectBoundViews();
  }

  void _refreshProjectBoundViews() {
    ref.invalidate(toolsProvider);
    ref.invalidate(boardsProvider);
    ref.invalidate(layoutProvider);
    ref.invalidate(tagOptionsForBoardProvider);
    ref.read(currentBoardKeyProvider.notifier).clear();
  }
}
