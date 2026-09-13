/// Inline "provider not configured" state for a pin that advertises live
/// external data but has no wired-up provider (see
/// [toolNeedsProviderConfig]).
///
/// Shows an honest "needs setup" badge instead of a runnable affordance
/// so the user is not misled into thinking the pin is fetching data —
/// and, since credential wiring is CLI-only today, points at the exact
/// `upeg credential add …` command with a copy button so the state is
/// not a dead end.
///
/// All copy flows through `t()` + the Rust En/Ko catalog
/// (`upeg-pegboard-ui/src/i18n.rs`, `pin.provider_not_configured.*`).
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';

/// Catalog keys. Exported so tests assert against the constants instead
/// of re-typing magic strings.
const String providerNotConfiguredLabelKey =
    'pin.provider_not_configured.label';
const String providerNotConfiguredHintKey = 'pin.provider_not_configured.hint';
const String providerNotConfiguredCliHintKey =
    'pin.provider_not_configured.cli_hint';
const String providerNotConfiguredCopyTooltipKey =
    'pin.provider_not_configured.copy_tooltip';

/// Widget keys so tests can locate the state without depending on copy.
const Key providerNotConfiguredBodyKey = Key('provider-not-configured-body');
const Key providerNotConfiguredCommandKey = Key(
  'provider-not-configured-command',
);

/// CLI verb pieces for the fix-it command. The credential name is a
/// runtime parameter — no provider id is ever hardcoded here (generic
/// over example).
const String _cliBinary = 'upeg';
const String _cliCredentialAddSubcommand = 'credential add';

/// Placeholder shown when the concrete credential name is unknown at
/// this call site. Mirrors clap's `<NAME>`-style value syntax.
const String credentialNamePlaceholder = '<name>';

/// Assemble the CLI command that registers a credential reference
/// (`upeg credential add <name>`). Pass the manifest-level credential
/// name when the calling context knows it; `null` falls back to the
/// generic placeholder.
String credentialAddCommand([String? credentialName]) =>
    '$_cliBinary $_cliCredentialAddSubcommand '
    '${credentialName ?? credentialNamePlaceholder}';

class ProviderNotConfiguredBody extends ConsumerWidget {
  const ProviderNotConfiguredBody({this.credentialName, super.key});

  /// Manifest-level credential name for the current provider context,
  /// used to assemble the copyable CLI command. `null` renders the
  /// generic `<name>` placeholder.
  final String? credentialName;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
    final command = credentialAddCommand(credentialName);
    return Padding(
      key: providerNotConfiguredBodyKey,
      padding: UpegSizing.pinBodyPadding,
      // FittedBox(scaleDown): the pin body cell is small and fixed; the
      // added CLI hint must shrink instead of tripping a RenderFlex
      // overflow.
      child: FittedBox(
        fit: BoxFit.scaleDown,
        alignment: Alignment.topLeft,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(Icons.settings_outlined, size: 12, color: tokens.fg4),
                const SizedBox(width: 4),
                Text(
                  t(ref, providerNotConfiguredLabelKey),
                  style: TextStyle(
                    fontFamily: upegMonoFontFamily,
                    fontFamilyFallback: upegMonoFontFamilyFallback,
                    fontSize: 11,
                    color: tokens.fg3,
                    fontWeight: FontWeight.w500,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 2),
            Text(
              t(ref, providerNotConfiguredHintKey),
              style: TextStyle(
                fontFamily: upegMonoFontFamily,
                fontFamilyFallback: upegMonoFontFamilyFallback,
                fontSize: 9.5,
                color: tokens.fg4,
                height: 1.3,
              ),
            ),
            const SizedBox(height: 6),
            Text(
              t(ref, providerNotConfiguredCliHintKey),
              style: TextStyle(
                fontFamily: upegMonoFontFamily,
                fontFamilyFallback: upegMonoFontFamilyFallback,
                fontSize: 9.5,
                color: tokens.fg4,
                height: 1.3,
              ),
            ),
            const SizedBox(height: 2),
            Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  command,
                  key: providerNotConfiguredCommandKey,
                  style: TextStyle(
                    fontFamily: upegMonoFontFamily,
                    fontFamilyFallback: upegMonoFontFamilyFallback,
                    fontSize: 9.5,
                    color: tokens.fg3,
                    height: 1.3,
                  ),
                ),
                const SizedBox(width: 2),
                CopyToClipboardButton(
                  textToCopy: command,
                  writer: ref.watch(clipboardWriterProvider),
                  tooltip: t(ref, providerNotConfiguredCopyTooltipKey),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
