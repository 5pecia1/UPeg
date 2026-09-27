import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';
import 'package:upeg/src/state/diagnostics_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/rust/api/dispatch_stream.dart';
import 'package:upeg/src/state/presentation_resolver_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

import '../test_helpers/dispatch_stream_fixture.dart';
import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/tool_fixture.dart';

class _BoardNotifier extends CurrentBoardNotifier {
  @override
  BoardKey? build() => BoardKey.parse('board-a');
}

const _rowAction = PresentationActionDto(
  id: 'mode_plan',
  scope: ActionScopeDto.row,
  label: 'Plan',
  targetTool: 'catalog.plan',
  bindings: [],
);

const _applyAction = PresentationActionDto(
  id: 'mode_apply',
  scope: ActionScopeDto.result,
  label: 'Apply',
  targetTool: 'catalog.apply',
  onSuccess: ActionSuccessDto.refreshOrigin,
  bindings: [],
);

const _presentation = ToolPresentationDto(
  version: 1,
  rows: '/rows',
  rowKey: '/id',
  columns: [PresentationColumnDto(label: 'Name', pointer: '/name')],
  actions: [_rowAction],
);

CanonicalToolResult _jsonResult(String value) => CanonicalToolResult(
  ok: true,
  outputs: [
    CanonicalOutputEntry(
      id: 'data',
      label: 'Data',
      kind: 'json',
      value: CanonicalOutputValue.json(value: value),
    ),
  ],
);

class _RunDiagnostics implements DiagnosticsApi {
  String? runId;
  @override
  Future<List<DiagnosticSummary>> list({required int limit}) async =>
      runId == null
      ? []
      : [
          DiagnosticSummary(
            id: 'saved-failure',
            runId: runId!,
            occurredAtMs: 1,
            source: 'external',
            errorCode: 'failed',
            errorMessage: 'failed',
            status: 'failed',
          ),
        ];
  @override
  Future<DiagnosticReport?> show(String id) async => null;
  @override
  Future<String> export(String id, {required bool debug}) async =>
      '{"id":"$id"}';
}

void main() {
  testWidgets('a completed failed run keeps its exact diagnostic link', (
    tester,
  ) async {
    final api = _RunDiagnostics();
    final tool = fixtureToolDto(
      id: 'demo.failure',
      effect: ToolEffectDto.read,
      presentation: const ToolPresentationDto(
        version: 1,
        columns: [],
        actions: [],
      ),
    );
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          ...i18nTestOverrides,
          currentBoardKeyProvider.overrideWith(_BoardNotifier.new),
          diagnosticsApiProvider.overrideWithValue(api),
          dispatchStreamFnProvider.overrideWithValue(({
            PinKey? pinKey,
            required toolId,
            required args,
            required approve,
            required runId,
          }) async* {
            api.runId = runId.value;
            yield const DispatchStreamEventDto.done(
              result: CanonicalToolResult(
                ok: false,
                outputs: [],
                error: CanonicalToolError(code: 'failed', message: 'failed'),
              ),
            );
          }),
        ],
        child: MaterialApp(
          theme: UpegTheme.darkTheme(),
          home: ExpandedModalPage(tool: tool),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(api.runId, isNotNull);
    expect(find.byKey(Key('diagnostic-run-${api.runId}')), findsOneWidget);
  });

  testWidgets('inputless read presentation opens and queries once', (
    tester,
  ) async {
    final overview = fixtureToolDto(
      id: 'ecosystem.overview',
      effect: ToolEffectDto.read,
      inputFields: const [
        InputFieldDto(
          key: 'project',
          label: 'Project',
          fieldType: InputFieldType_Text(),
          required_: false,
        ),
      ],
      presentation: const ToolPresentationDto(
        version: 1,
        columns: [],
        actions: [],
      ),
    );
    final calls = <Map<String, Object?>>[];

    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          ...i18nTestOverrides,
          currentBoardKeyProvider.overrideWith(_BoardNotifier.new),
          presentationRowsResolverProvider.overrideWithValue(
            ({required toolId, required outputsJson}) =>
                const PresentationRowsDto(
                  rows: [],
                  diagnostics: [],
                  rowActionsEnabled: true,
                ),
          ),
          dispatchStreamFnProvider.overrideWithValue(
            stubDispatchStream(({
              required toolId,
              required args,
              required approve,
            }) async {
              calls.add(args.toJsonObject());
              return _jsonResult('{"rows":[]}');
            }),
          ),
        ],
        child: MaterialApp(
          theme: UpegTheme.darkTheme(),
          home: ExpandedModalPage(tool: overview),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(calls, const [<String, Object?>{}]);
  });

  testWidgets(
    'row and result actions prefill forms and refresh the exact read once',
    (tester) async {
      final list = fixtureToolDto(
        id: 'catalog.list',
        effect: ToolEffectDto.read,
        inputFields: const [
          InputFieldDto(
            key: 'project',
            label: 'Project',
            fieldType: InputFieldType_Text(),
            required_: true,
          ),
        ],
        presentation: _presentation,
      );
      final plan = fixtureToolDto(
        id: 'catalog.plan',
        inputFields: const [
          InputFieldDto(
            key: 'id',
            label: 'Id',
            fieldType: InputFieldType_Text(),
            required_: true,
          ),
          InputFieldDto(
            key: 'mode',
            label: 'Mode',
            fieldType: InputFieldType_Text(),
            required_: true,
          ),
        ],
        presentation: const ToolPresentationDto(
          version: 1,
          columns: [],
          actions: [_applyAction],
        ),
      );
      final apply = fixtureToolDto(
        id: 'catalog.apply',
        effect: ToolEffectDto.write,
        inputFields: plan.inputFields,
      );
      final calls = <({String tool, Map<String, Object?> args})>[];

      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            ...i18nTestOverrides,
            currentBoardKeyProvider.overrideWith(_BoardNotifier.new),
            toolsLoaderProvider.overrideWith(
              (ref) =>
                  () => [list, plan, apply],
            ),
            presentationRowsResolverProvider.overrideWithValue(
              ({required toolId, required outputsJson}) =>
                  toolId == 'catalog.list'
                  ? const PresentationRowsDto(
                      rows: [
                        PresentationRowDto(
                          key: 'row-1',
                          valueJson: '{"id":"row-1","name":"One"}',
                          cellsJson: ['"One"'],
                        ),
                      ],
                      diagnostics: [],
                      rowActionsEnabled: true,
                    )
                  : const PresentationRowsDto(
                      rows: [],
                      diagnostics: [],
                      rowActionsEnabled: true,
                    ),
            ),
            presentationBindingsResolverProvider.overrideWithValue(
              ({
                required toolId,
                required actionId,
                required currentInputsJson,
                selectedRowJson,
                required outputsJson,
              }) => ActionBindingResolutionDto(
                valuesJson: actionId == 'mode_plan'
                    ? '{"id":"row-1","mode":"plan"}'
                    : '{"id":"row-1","mode":"apply"}',
                diagnostics: const [],
                unboundRequiredInputs: const [],
              ),
            ),
            dispatchStreamFnProvider.overrideWithValue(
              stubDispatchStream(({
                required toolId,
                required args,
                required approve,
              }) async {
                calls.add((tool: toolId.value, args: args.toJsonObject()));
                return _jsonResult('{"rows":[]}');
              }),
            ),
          ],
          child: MaterialApp(
            theme: UpegTheme.darkTheme(),
            home: ExpandedModalPage(
              tool: list,
              initialInput: ToolArgs.fromJsonObject(const {
                'project': '/workspace/original',
              }),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
      await tester.pumpAndSettle();
      final row = find.byKey(const Key('presentation-table-row-row-1'));
      await tester.ensureVisible(row);
      await tester.tap(row);
      await tester.pump();
      await tester.tap(
        find.byKey(const Key('presentation-table-action-mode_plan')),
      );
      await tester.pumpAndSettle();

      expect(calls, hasLength(1));
      String? fieldText(String key) => tester
          .widget<TextFormField>(find.byKey(Key('field-$key')).last)
          .initialValue;
      expect(fieldText('id'), 'row-1');
      expect(fieldText('mode'), 'plan');

      await tester.tap(find.byKey(const Key('expanded-modal-run-btn')).last);
      await tester.pumpAndSettle();
      final applyAction = find.byKey(
        const Key('presentation-result-action-mode_apply'),
      );
      await tester.ensureVisible(applyAction);
      await tester.tap(applyAction);
      await tester.pumpAndSettle();

      expect(calls, hasLength(2));
      expect(fieldText('mode'), 'apply');

      await tester.tap(find.byKey(const Key('expanded-modal-run-btn')).last);
      await tester.pumpAndSettle();

      expect(calls, hasLength(4));
      expect(calls.map((call) => call.tool), [
        'catalog.list',
        'catalog.plan',
        'catalog.apply',
        'catalog.list',
      ]);
      expect(calls.last.args, {'project': '/workspace/original'});
    },
  );
}
