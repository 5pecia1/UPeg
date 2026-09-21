/// Widget tests for the human-approval gate in front of a desktop
/// dispatch.
///
/// The contract under test is "nothing runs until the question is
/// answered": a gated tool asks before dispatching, a cancelled question
/// dispatches nothing, and a Chain that does not honor desktop approval
/// is explained rather than offered a button that would change nothing.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/approval_confirm_dialog.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';
import 'package:upeg/src/widgets/inline/generic_inline_pin_body.dart';

import '../test_helpers/dispatch_stream_fixture.dart';
import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/tool_fixture.dart';

const _success = CanonicalToolResult(ok: true, outputs: []);

/// A Chain that stops at a barrier this surface may lift.
final _desktopGatedTool = fixtureToolDto(
  id: 'fixture.gated_desktop',
  label: 'gated chain',
  pinKind: PinKindDto.chain,
  invoker: InvokerDto.chain,
  requiresApproval: true,
  approvalSurfaces: const <String>['cli', desktopApprovalSurfaceLabel],
);

/// A Chain whose manifest names other approvers only.
final _elsewhereGatedTool = fixtureToolDto(
  id: 'fixture.gated_elsewhere',
  label: 'cli-only chain',
  pinKind: PinKindDto.chain,
  invoker: InvokerDto.chain,
  requiresApproval: true,
  approvalSurfaces: const <String>['cli', 'tui'],
);

final _ungatedTool = fixtureToolDto(
  id: 'fixture.no_gate',
  label: 'no gate',
  inputFields: const <InputFieldDto>[],
);

/// Records every dispatch the widget under test starts.
final class _DispatchLog {
  final List<bool> approvals = <bool>[];

  Future<CanonicalToolResult> call({
    required ToolId toolId,
    required ToolArgs args,
    required bool approve,
  }) async {
    approvals.add(approve);
    return _success;
  }
}

Widget _modalHarness({
  required _DispatchLog log,
  required ToolDto tool,
  ApprovalConfirmFn? confirm,
}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      dispatchStreamFnProvider.overrideWithValue(stubDispatchStream(log.call)),
      if (confirm != null) approvalConfirmFnProvider.overrideWithValue(confirm),
    ],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: ExpandedModalPage(tool: tool),
    ),
  );
}

Widget _inlineHarness({required _DispatchLog log, required ToolDto tool}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      dispatchStreamFnProvider.overrideWithValue(stubDispatchStream(log.call)),
    ],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: Scaffold(
        body: SizedBox(
          width: 260,
          height: 220,
          child: GenericInlinePinBody(
            tool: tool,
            pinKey: (BoardKey.parse('dev'), ToolId.parse(tool.id)),
          ),
        ),
      ),
    ),
  );
}

void main() {
  group('Approval requirement detection', () {
    test('an_ungated_tool_requires_no_approval_prompt', () {
      expect(approvalRequirementFor(_ungatedTool), ApprovalRequirement.none);
    });

    test('prompts_when_desktop_is_an_approver', () {
      expect(
        approvalRequirementFor(_desktopGatedTool),
        ApprovalRequirement.askDesktop,
      );
    });

    test('only_explains_when_desktop_is_not_an_approver', () {
      expect(
        approvalRequirementFor(_elsewhereGatedTool),
        ApprovalRequirement.explainOtherSurfaces,
      );
    });
  });

  group('ExpandedModalPage approval gate', () {
    testWidgets('a_gated_tool_dispatches_only_after_approval', (tester) async {
      final log = _DispatchLog();
      await tester.pumpWidget(_modalHarness(log: log, tool: _desktopGatedTool));

      await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
      await tester.pumpAndSettle();

      expect(find.byKey(approvalConfirmDialogKey), findsOneWidget);
      expect(
        log.approvals,
        isEmpty,
        reason: 'Nothing runs while the approval prompt is awaiting an answer',
      );

      await tester.tap(find.byKey(approvalApproveButtonKey));
      await tester.pumpAndSettle();

      expect(log.approvals, <bool>[true]);
      expect(find.byKey(approvalConfirmDialogKey), findsNothing);
    });

    testWidgets('canceling_approval_dispatches_nothing', (tester) async {
      final log = _DispatchLog();
      await tester.pumpWidget(_modalHarness(log: log, tool: _desktopGatedTool));

      await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
      await tester.pumpAndSettle();
      await tester.tap(find.byKey(approvalCancelButtonKey));
      await tester.pumpAndSettle();

      expect(log.approvals, isEmpty);
      expect(find.byKey(approvalConfirmDialogKey), findsNothing);
      expect(find.byKey(const Key('expanded-modal-outcome')), findsNothing);
    });

    testWidgets(
      'when_desktop_is_not_an_approver_it_explains_without_dispatching',
      (tester) async {
        final log = _DispatchLog();
        await tester.pumpWidget(
          _modalHarness(log: log, tool: _elsewhereGatedTool),
        );

        await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
        await tester.pumpAndSettle();

        expect(find.byKey(approvalConfirmDialogKey), findsOneWidget);
        expect(
          find.byKey(approvalApproveButtonKey),
          findsNothing,
          reason:
              'Do not offer an approval button whose approval would not be honored',
        );
        expect(find.textContaining('cli, tui'), findsOneWidget);

        await tester.tap(find.byKey(approvalDismissButtonKey));
        await tester.pumpAndSettle();

        expect(log.approvals, isEmpty);
      },
    );

    testWidgets(
      'an_ungated_tool_dispatches_with_approve_false_without_prompting',
      (tester) async {
        final log = _DispatchLog();
        await tester.pumpWidget(_modalHarness(log: log, tool: _ungatedTool));

        await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
        await tester.pumpAndSettle();

        expect(find.byKey(approvalConfirmDialogKey), findsNothing);
        expect(log.approvals, <bool>[false]);
      },
    );
  });

  group('Inline pin approval gate', () {
    testWidgets('inline_Run_uses_the_same_approval_gate', (tester) async {
      final log = _DispatchLog();
      await tester.pumpWidget(
        _inlineHarness(log: log, tool: _desktopGatedTool),
      );
      await tester.pump();

      await tester.tap(find.byKey(inlineRunButtonKey));
      await tester.pumpAndSettle();

      expect(find.byKey(approvalConfirmDialogKey), findsOneWidget);
      expect(log.approvals, isEmpty);

      await tester.tap(find.byKey(approvalApproveButtonKey));
      await tester.pumpAndSettle();

      expect(log.approvals, <bool>[true]);
    });

    testWidgets(
      'a_gated_inline_tool_does_not_run_automatically_when_input_changes',
      (tester) async {
        final log = _DispatchLog();
        final gatedWithInput = fixtureToolDto(
          id: 'fixture.gated_input',
          label: 'gated input chain',
          pinKind: PinKindDto.chain,
          invoker: InvokerDto.chain,
          requiresApproval: true,
          approvalSurfaces: const <String>[desktopApprovalSurfaceLabel],
          inputFields: const <InputFieldDto>[
            InputFieldDto(
              key: 'value',
              label: 'Value',
              fieldType: InputFieldType.text(),
              required_: true,
            ),
          ],
        );
        await tester.pumpWidget(_inlineHarness(log: log, tool: gatedWithInput));
        await tester.pump();

        await tester.enterText(find.byKey(const Key('field-value')), 'typed');
        await tester.pump(inlineRunDebounce * 2);
        await tester.pumpAndSettle();

        expect(find.byKey(approvalConfirmDialogKey), findsNothing);
        expect(
          log.approvals,
          isEmpty,
          reason:
              'An approval gate requires a human answer, so execution is not automatic',
        );
        expect(
          find.byKey(inlineRunButtonKey),
          findsOneWidget,
          reason:
              'An explicit Run control is needed because execution is not automatic',
        );
      },
    );
  });
}
