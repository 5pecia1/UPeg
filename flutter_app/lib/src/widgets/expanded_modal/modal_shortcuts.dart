part of '../../pages/expanded_modal_page.dart';

/// Batch O2 (I13 F3): modal-wide shortcut intent for "+ pin". Routed
/// through `Shortcuts`/`Actions` so bespoke forms (which bind F1+F2
/// only) let F3 bubble up to the modal shell.
class _PinIntent extends Intent {
  const _PinIntent();
}

class _CopyIntent extends Intent {
  const _CopyIntent();
}

class _RunIntent extends Intent {
  const _RunIntent();
}

class _CloseIntent extends Intent {
  const _CloseIntent();
}

/// Pick the first useful value for the modal's Copy shortcut.
String? _copyTextForOutcome(CanonicalToolResult? outcome) {
  if (outcome == null) return null;
  final candidates = [
    outcome.errorMessage,
    outcome.primaryOutputText,
    outcome.outputs.isEmpty ? null : outcome.canonicalJsonText(),
  ];
  for (final candidate in candidates) {
    if (candidate != null && candidate.isNotEmpty) return candidate;
  }
  return null;
}
