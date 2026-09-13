/// Inline "unsupported on this surface" state for a pin whose tool cannot
/// physically dispatch in-process on the current runtime (see
/// `upeg-core`'s `dispatch_capability`).
///
/// Generalizes the honest-state pattern of [ProviderNotConfiguredBody] and
/// the controlled-embed `_InlineUnsupportedNotice`: instead of a runnable
/// affordance the user gets an honest badge plus a reason-specific hint, so
/// they are not misled into thinking the pin can run here (it runs on
/// desktop).
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/capability.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

/// Catalog key for the badge copy — tests assert through `t()` + this key.
const String surfaceUnsupportedLabelKey = 'surface.unsupported.label';

/// Catalog key of the secondary line shown when the reason is one a
/// paired host could solve but no host is configured yet. Points the
/// user at the settings pairing.
const String surfaceUnsupportedAttachHintKey =
    'surface.unsupported.attach_hint';

/// Widget key so tests can locate the state without depending on copy.
const Key surfaceUnsupportedBodyKey = Key('surface-unsupported-body');

/// Whether pairing with a host daemon could make this reason runnable.
///
/// Every wasm/PWA-unsupported reason is host-attach-solvable: a paired
/// native daemon links the full loader (subprocess/http/…/wasm-plugin) AND
/// every native-gated Function dispatcher, so it can run net/media/eth
/// tools the browser physically cannot — including `nativeOnlyTool`, whose
/// dispatcher is simply compiled out of the wasm build. The honest-error
/// path still covers the case where the daemon reports the tool isn't on
/// its surface.
bool hostAttachCanSolve(UnsupportedReasonDto reason) {
  return switch (reason) {
    UnsupportedReasonDto.noProcessSpawn ||
    UnsupportedReasonDto.noLoaderRuntime ||
    UnsupportedReasonDto.noWasmHost ||
    UnsupportedReasonDto.nativeOnlyTool => true,
  };
}

/// Reason-specific hint catalog key. Every [UnsupportedReasonDto] maps
/// to a stable key (mirrors the reason `label()` vocabulary in
/// `upeg-core`); the En/Ko copy lives in the Rust catalog.
String surfaceUnsupportedHintKey(UnsupportedReasonDto reason) {
  return switch (reason) {
    UnsupportedReasonDto.noProcessSpawn =>
      'surface.unsupported.hint.no_process_spawn',
    UnsupportedReasonDto.noLoaderRuntime =>
      'surface.unsupported.hint.no_loader_runtime',
    UnsupportedReasonDto.noWasmHost => 'surface.unsupported.hint.no_wasm_host',
    UnsupportedReasonDto.nativeOnlyTool =>
      'surface.unsupported.hint.native_only_tool',
  };
}

class SurfaceUnsupportedBody extends ConsumerWidget {
  const SurfaceUnsupportedBody({
    required this.reason,
    this.showAttachHint = false,
    super.key,
  });

  final UnsupportedReasonDto reason;

  /// When true, append the [surfaceUnsupportedAttachHintKey] line — a
  /// nudge that pairing a host in settings would make this tool runnable.
  /// Set by the board only for attach-solvable reasons with no host
  /// configured.
  final bool showAttachHint;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
    return Padding(
      key: surfaceUnsupportedBodyKey,
      padding: UpegSizing.pinBodyPadding,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.block_outlined, size: 12, color: tokens.fg4),
              const SizedBox(width: 4),
              // Flexible + ellipsis: locale variants of the badge copy
              // differ in width and must squeeze inside the pin body.
              Flexible(
                child: Text(
                  t(ref, surfaceUnsupportedLabelKey),
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    fontFamily: upegMonoFontFamily,
                    fontFamilyFallback: upegMonoFontFamilyFallback,
                    fontSize: 11,
                    color: tokens.fg3,
                    fontWeight: FontWeight.w500,
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 2),
          Text(
            t(ref, surfaceUnsupportedHintKey(reason)),
            style: TextStyle(
              fontFamily: upegMonoFontFamily,
              fontFamilyFallback: upegMonoFontFamilyFallback,
              fontSize: 9.5,
              color: tokens.fg4,
              height: 1.3,
            ),
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
          ),
          if (showAttachHint) ...[
            const SizedBox(height: 3),
            Text(
              t(ref, surfaceUnsupportedAttachHintKey),
              key: const Key('surface-unsupported-attach-hint'),
              style: TextStyle(
                fontFamily: upegMonoFontFamily,
                fontFamilyFallback: upegMonoFontFamilyFallback,
                fontSize: 9.5,
                color: tokens.accent,
                height: 1.3,
              ),
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
          ],
        ],
      ),
    );
  }
}
