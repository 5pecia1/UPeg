/// Human-approval gate in front of a desktop dispatch.
///
/// A Chain Tool can declare a step that refuses to run until a person
/// says yes ([ToolDto.requiresApproval]), and the manifest names which
/// surfaces' "yes" the Chain honors ([ToolDto.approvalSurfaces]). Both
/// facts are on the DTO *before* dispatch precisely so the question can
/// be asked in front of the run instead of arriving as a failed result
/// the person cannot answer. See `docs/architecture/chain.md`.
///
/// Two outcomes, and the difference matters:
///
///  * the desktop is an honored approver — ask, and on yes dispatch with
///    the typed `approve` flag;
///  * it is not — do not offer a button that would do nothing. Say which
///    surfaces can approve, and dispatch nothing.
///
/// SoC: this file owns the question. The run flows
/// (`pages/expanded_modal_page.dart`, `widgets/inline/…`) own the
/// dispatch and read the verdict.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/no_transition_dialog.dart';

/// Surface label this app dispatches as. Matches
/// `upeg_core::Surface::Desktop`'s wire label, which is what
/// [ToolDto.approvalSurfaces] carries.
const String desktopApprovalSurfaceLabel = 'desktop';

/// Separator between surface labels in the "who can approve" sentence.
const String _approvalSurfaceSeparator = ', ';

const Key approvalConfirmDialogKey = Key('approval-confirm-dialog');
const Key approvalApproveButtonKey = Key('approval-approve-btn');
const Key approvalCancelButtonKey = Key('approval-cancel-btn');
const Key approvalDismissButtonKey = Key('approval-dismiss-btn');

const String _approvalTitleKey = 'modal.approval.title';
const String _approvalBodyKey = 'modal.approval.body';
const String _approvalApproveKey = 'modal.approval.approve';
const String _approvalCancelKey = 'modal.approval.cancel';
const String _approvalDeniedBodyKey = 'modal.approval.denied_body';
const String _approvalDismissKey = 'modal.approval.dismiss';

const String _toolPlaceholder = 'tool';
const String _surfacesPlaceholder = 'surfaces';

/// What this surface must do before dispatching a tool.
enum ApprovalRequirement {
  /// No barrier: dispatch straight away.
  none,

  /// Gated, and a desktop "yes" is honored: ask first.
  askDesktop,

  /// Gated, but this surface's "yes" is not honored: explain, run
  /// nothing.
  explainOtherSurfaces,
}

/// Read the tool's approval facts into the one decision a run flow needs.
ApprovalRequirement approvalRequirementFor(ToolDto tool) {
  if (!tool.requiresApproval) return ApprovalRequirement.none;
  return tool.approvalSurfaces.contains(desktopApprovalSurfaceLabel)
      ? ApprovalRequirement.askDesktop
      : ApprovalRequirement.explainOtherSurfaces;
}

/// Answer to the approval gate.
sealed class ApprovalVerdict {
  const ApprovalVerdict();
}

/// Dispatch may proceed. [approve] is the typed flag to hand Rust — true
/// only when a person approved a gated run.
final class ApprovalGranted extends ApprovalVerdict {
  const ApprovalGranted({required this.approve});

  final bool approve;
}

/// Nothing dispatches: the person cancelled, or this surface's approval
/// would not have been honored anyway.
final class ApprovalWithheld extends ApprovalVerdict {
  const ApprovalWithheld();
}

/// Test seam for the confirmation dialog, the same shape as
/// `settingsOverlayLauncherProvider`: a widget test stubs the answer
/// instead of pumping a real route.
typedef ApprovalConfirmFn =
    Future<ApprovalVerdict> Function(BuildContext context, ToolDto tool);

final approvalConfirmFnProvider = Provider<ApprovalConfirmFn>(
  (ref) => showApprovalConfirmDialog,
);

/// The gate every desktop run flow passes through.
///
/// An ungated tool never sees a dialog and dispatches with
/// `approve: false` — the flag is not "am I allowed to run", it is "a
/// person answered the barrier", and an ungated tool has no barrier to
/// answer.
Future<ApprovalVerdict> resolveApprovalBeforeDispatch({
  required BuildContext context,
  required WidgetRef ref,
  required ToolDto tool,
}) {
  if (approvalRequirementFor(tool) == ApprovalRequirement.none) {
    return Future<ApprovalVerdict>.value(const ApprovalGranted(approve: false));
  }
  return ref.read(approvalConfirmFnProvider)(context, tool);
}

/// Production [ApprovalConfirmFn]: a modal the person must answer.
///
/// Dismissing the barrier (tap-outside, Esc) is a "no" — the safe
/// reading of silence in front of a gated run.
Future<ApprovalVerdict> showApprovalConfirmDialog(
  BuildContext context,
  ToolDto tool,
) async {
  final verdict = await showNoTransitionDialog<ApprovalVerdict>(
    context: context,
    builder: (_) => _ApprovalConfirmDialog(tool: tool),
  );
  return verdict ?? const ApprovalWithheld();
}

class _ApprovalConfirmDialog extends ConsumerWidget {
  const _ApprovalConfirmDialog({required this.tool});

  final ToolDto tool;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
    final canApprove =
        approvalRequirementFor(tool) == ApprovalRequirement.askDesktop;
    final body = canApprove
        ? t(ref, _approvalBodyKey, {_toolPlaceholder: tool.label})
        : t(ref, _approvalDeniedBodyKey, {
            _toolPlaceholder: tool.label,
            _surfacesPlaceholder: tool.approvalSurfaces.join(
              _approvalSurfaceSeparator,
            ),
          });
    return AlertDialog(
      key: approvalConfirmDialogKey,
      backgroundColor: tokens.surface,
      title: Text(
        t(ref, _approvalTitleKey),
        style: TextStyle(color: tokens.fg, fontSize: 14),
      ),
      content: Text(
        body,
        style: TextStyle(color: tokens.fg2, fontSize: 12, height: 1.5),
      ),
      actions: canApprove
          ? <Widget>[
              TextButton(
                key: approvalCancelButtonKey,
                onPressed: () => Navigator.of(
                  context,
                ).pop<ApprovalVerdict>(const ApprovalWithheld()),
                child: Text(t(ref, _approvalCancelKey)),
              ),
              FilledButton(
                key: approvalApproveButtonKey,
                onPressed: () => Navigator.of(
                  context,
                ).pop<ApprovalVerdict>(const ApprovalGranted(approve: true)),
                child: Text(t(ref, _approvalApproveKey)),
              ),
            ]
          : <Widget>[
              FilledButton(
                key: approvalDismissButtonKey,
                onPressed: () => Navigator.of(
                  context,
                ).pop<ApprovalVerdict>(const ApprovalWithheld()),
                child: Text(t(ref, _approvalDismissKey)),
              ),
            ],
    );
  }
}
