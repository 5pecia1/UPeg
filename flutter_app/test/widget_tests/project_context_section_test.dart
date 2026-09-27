library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/i18n.dart' show LocaleDto;
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/project_context_provider.dart';
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/widgets/project_context_section.dart';

const _tweaks = TweaksDto(
  theme: 'Light',
  accent: 'Green',
  showHoles: true,
  locale: 'En',
  localHttpHost: false,
);

class _ProjectApi implements ProjectContextApi {
  _ProjectApi({this.currentProject, this.error, this.conflicts = const []});
  ProjectDefinition? currentProject;
  final Object? error;
  final List<ProjectToolConflict> conflicts;
  final List<(String, String, ProjectToolChoice)> choices = [];
  int activations = 0;

  @override
  Future<ProjectDefinition?> current() async => currentProject;

  @override
  Future<ProjectDefinition> validateRoot(String root) async {
    if (error case final error?) throw error;
    return ProjectDefinition(
      root: root,
      name: 'Sample',
      boardCount: 2,
      toolkits: const ['toolkits/sample.toml'],
      conflicts: conflicts,
    );
  }

  @override
  Future<ProjectActivation> activate(String root) async {
    activations++;
    final project = await validateRoot(root);
    currentProject = project;
    return ProjectActivation(project: project);
  }

  @override
  Future<void> close() async => currentProject = null;

  @override
  Future<ProjectActivation> setToolChoice({
    required String root,
    required String toolId,
    required ProjectToolChoice choice,
  }) async {
    choices.add((root, toolId, choice));
    return ProjectActivation(project: await validateRoot(root));
  }
}

String _translate(String key, LocaleDto locale) => key;

String _translateArgs(
  String key,
  LocaleDto locale,
  List<String> argKeys,
  List<String> argVals,
) {
  return '$key ${argVals.join(' ')}';
}

Widget _harness(_ProjectApi api) => ProviderScope(
  overrides: [
    projectContextApiProvider.overrideWithValue(api),
    tweaksLoaderProvider.overrideWith(
      (ref) =>
          () => _tweaks,
    ),
    i18nTranslateOverride.overrideWithValue(_translate),
    i18nTranslateArgsOverride.overrideWithValue(_translateArgs),
  ],
  child: MaterialApp(home: Scaffold(body: const ProjectContextSection())),
);

void main() {
  testWidgets('invalid next project leaves the active project visible', (
    tester,
  ) async {
    final api = _ProjectApi(
      currentProject: const ProjectDefinition(
        root: '/existing',
        name: 'Existing',
        boardCount: 1,
        toolkits: [],
      ),
      error: ArgumentError('missing .upeg directory'),
    );
    await tester.pumpWidget(_harness(api));
    await tester.pumpAndSettle();

    await tester.enterText(
      find.byKey(const Key('project-context-root-input')),
      '/invalid',
    );
    await tester.tap(find.byKey(projectActivateButtonKey));
    await tester.pumpAndSettle();

    expect(api.currentProject?.root, '/existing');
    expect(find.byKey(const Key('project-context-error')), findsOneWidget);
    expect(api.activations, 0);
  });

  testWidgets('opens a validated project without changing process context', (
    tester,
  ) async {
    final api = _ProjectApi();
    await tester.pumpWidget(_harness(api));
    await tester.pumpAndSettle();

    await tester.enterText(
      find.byKey(const Key('project-context-root-input')),
      '/workspace/sample',
    );
    await tester.tap(find.byKey(projectActivateButtonKey));
    await tester.pumpAndSettle();

    expect(api.activations, 1);
    expect(find.textContaining('/workspace/sample'), findsWidgets);
  });

  testWidgets('switches projects and returns to the global context', (
    tester,
  ) async {
    final api = _ProjectApi();
    await tester.pumpWidget(_harness(api));
    await tester.pumpAndSettle();

    await tester.enterText(
      find.byKey(const Key('project-context-root-input')),
      '/workspace/first',
    );
    await tester.tap(find.byKey(projectActivateButtonKey));
    await tester.pumpAndSettle();
    expect(api.currentProject?.root, '/workspace/first');

    await tester.enterText(
      find.byKey(const Key('project-context-root-input')),
      '/workspace/second',
    );
    await tester.tap(find.byKey(projectActivateButtonKey));
    await tester.pumpAndSettle();
    expect(api.currentProject?.root, '/workspace/second');
    expect(api.activations, 2);

    await tester.tap(find.byKey(projectCloseButtonKey));
    await tester.pumpAndSettle();
    expect(api.currentProject, isNull);
    expect(
      tester
          .widget<TextField>(
            find.byKey(const Key('project-context-root-input')),
          )
          .controller
          ?.text,
      isEmpty,
    );
  });

  testWidgets('asks for an explicit source when a tool id conflicts', (
    tester,
  ) async {
    final api = _ProjectApi(
      conflicts: const [
        ProjectToolConflict(
          toolId: 'convert.sample',
          globalSource: 'global.toml',
          projectSource: 'project.toml',
        ),
      ],
    );
    await tester.pumpWidget(_harness(api));
    await tester.pumpAndSettle();

    await tester.enterText(
      find.byKey(const Key('project-context-root-input')),
      '/workspace/conflict',
    );
    await tester.tap(find.byKey(projectActivateButtonKey));
    await tester.pumpAndSettle();
    expect(api.activations, 0);
    expect(find.text('convert.sample'), findsOneWidget);

    await tester.tap(find.byType(RadioListTile<ProjectToolChoice>).last);
    await tester.tap(find.text('project.continue'));
    await tester.pumpAndSettle();
    expect(api.choices, [
      ('/workspace/conflict', 'convert.sample', ProjectToolChoice.project),
    ]);
    expect(api.activations, 1);
  });
}
