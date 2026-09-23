/// Reusable recovery UI for an External tool's prerequisite inspection.
///
/// The caller owns inspection and rechecking; this widget only presents the
/// typed result. Keeping it free of runtime/HTTP dependencies lets the same
/// guidance render for a local tool and a paired host tool.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:url_launcher/url_launcher.dart';

import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/readiness.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/copy_to_clipboard_button.dart';

const Key externalReadinessGuidanceKey = Key('external-readiness-guidance');
const Key externalReadinessRecheckKey = Key('external-readiness-recheck');
const Key externalReadinessGuideKey = Key('external-readiness-guide');

typedef GuideLauncher = Future<bool> Function(Uri uri);

final externalReadinessGuideLauncherProvider = Provider<GuideLauncher>(
  (ref) =>
      (uri) => launchUrl(uri, mode: LaunchMode.externalApplication),
);

class ExternalReadinessGuidance extends ConsumerWidget {
  const ExternalReadinessGuidance({
    required this.readiness,
    required this.onRecheck,
    this.compact = false,
    super.key,
  });

  final ExternalReadinessDto readiness;
  final VoidCallback onRecheck;
  final bool compact;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    if (readiness.status == ExternalReadinessStatusDto.ready) {
      return const SizedBox.shrink();
    }
    final tokens = context.upeg;
    final labelKey = switch (readiness.status) {
      ExternalReadinessStatusDto.ready => '',
      ExternalReadinessStatusDto.missingExecutable =>
        'readiness.missing_executable.label',
      ExternalReadinessStatusDto.missingWorkingDirectory =>
        'readiness.missing_working_directory.label',
      ExternalReadinessStatusDto.uncheckedCredentialPath =>
        'readiness.unchecked_credential_path.label',
    };
    final hintKey = switch (readiness.status) {
      ExternalReadinessStatusDto.ready => '',
      ExternalReadinessStatusDto.missingExecutable =>
        'readiness.missing_executable.hint',
      ExternalReadinessStatusDto.missingWorkingDirectory =>
        'readiness.missing_working_directory.hint',
      ExternalReadinessStatusDto.uncheckedCredentialPath =>
        'readiness.unchecked_credential_path.hint',
    };
    return Container(
      key: externalReadinessGuidanceKey,
      padding: compact ? const EdgeInsets.all(8) : const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: tokens.surface2,
        border: Border.all(color: tokens.line),
        borderRadius: BorderRadius.circular(UpegSizing.radius1),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            t(ref, labelKey),
            style: TextStyle(color: tokens.fg2, fontWeight: FontWeight.w600),
          ),
          const SizedBox(height: 4),
          Text(
            t(ref, hintKey),
            style: TextStyle(color: tokens.fg3, fontSize: 11),
          ),
          if (readiness.command case final command?) ...[
            const SizedBox(height: 6),
            Text(command, style: _mono(tokens)),
          ],
          if (!compact && readiness.workingDirectory != null) ...[
            const SizedBox(height: 4),
            Text(readiness.workingDirectory!, style: _mono(tokens)),
          ],
          if (!compact && readiness.instructions != null) ...[
            const SizedBox(height: 8),
            Text(
              readiness.instructions!,
              style: TextStyle(color: tokens.fg3, fontSize: 11),
            ),
          ],
          if (!compact &&
              readiness.status == ExternalReadinessStatusDto.missingExecutable)
            for (final command in readiness.installCommands) ...[
              const SizedBox(height: 6),
              Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Flexible(child: Text(command, style: _mono(tokens))),
                  CopyToClipboardButton(
                    textToCopy: command,
                    writer: ref.watch(clipboardWriterProvider),
                    tooltip: t(ref, 'readiness.copy_command'),
                  ),
                ],
              ),
            ],
          const SizedBox(height: 6),
          Wrap(
            spacing: 8,
            children: [
              TextButton(
                key: externalReadinessRecheckKey,
                onPressed: onRecheck,
                child: Text(t(ref, 'readiness.recheck')),
              ),
              if (!compact && _guideUri(readiness.guideUrl) != null)
                TextButton(
                  key: externalReadinessGuideKey,
                  onPressed: () =>
                      _openGuide(context, ref, _guideUri(readiness.guideUrl)!),
                  child: Text(t(ref, 'readiness.open_guide')),
                ),
            ],
          ),
        ],
      ),
    );
  }
}

TextStyle _mono(UpegTokens tokens) => TextStyle(
  fontFamily: upegMonoFontFamily,
  fontFamilyFallback: upegMonoFontFamilyFallback,
  color: tokens.fg3,
  fontSize: 10.5,
);

Uri? _guideUri(String? value) {
  final uri = value == null ? null : Uri.tryParse(value);
  return uri != null && (uri.scheme == 'https' || uri.scheme == 'http')
      ? uri
      : null;
}

Future<void> _openGuide(BuildContext context, WidgetRef ref, Uri uri) async {
  final opened = await ref.read(externalReadinessGuideLauncherProvider)(uri);
  if (!context.mounted || opened) return;
  ScaffoldMessenger.of(context).showSnackBar(
    SnackBar(content: Text(tRead(ref, 'readiness.guide_unavailable'))),
  );
}
