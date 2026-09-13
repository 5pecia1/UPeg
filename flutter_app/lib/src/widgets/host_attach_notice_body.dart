/// Inline pin-body notice for a failed host-attach dispatch (Task B3).
///
/// When the PWA routes an in-process-unsupported tool to the paired daemon
/// and the attempt fails, the pin shows an honest, remediation-oriented
/// notice instead of a stale/empty body. Mirrors [SurfaceUnsupportedBody]'s
/// badge + hint layout so the two honest states read identically.
///
/// A successful ([AttachDispatchOk]) dispatch never reaches here — its
/// result flows through `lastOutcomeProvider` and renders as a normal pin
/// output. [hostAttachNoticeFor] returns `null` for ok / not-yet-dispatched.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/features/host_attach/attach_client.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

/// Widget key so tests locate the notice without depending on copy.
const Key hostAttachNoticeBodyKey = Key('host-attach-notice-body');

/// Catalog keys — copy lives in the Rust En/Ko catalog
/// (upeg-pegboard-ui/src/i18n.rs) and renders through `t()`.
const String kHostAttachUnauthorizedLabelKey =
    'host_attach.notice.unauthorized_label';
const String kHostAttachUnauthorizedHintKey =
    'host_attach.notice.unauthorized_hint';
const String kHostAttachUnreachableLabelKey =
    'host_attach.notice.unreachable_label';
const String kHostAttachUnreachableHintKey =
    'host_attach.notice.unreachable_hint';
const String kHostAttachToolErrorLabelKey =
    'host_attach.notice.tool_error_label';

/// The user-facing badge + hint for a failed attach dispatch, or `null`
/// when there is nothing to show (ok, or not dispatched yet, or a
/// `unavailable`/`unauthorized`/`unreachable`/`toolError` variant).
HostAttachNotice? hostAttachNoticeFor(AttachDispatchResult? result) {
  return switch (result) {
    null || AttachDispatchOk() => null,
    AttachDispatchUnauthorized() => const HostAttachNotice(
      labelKey: kHostAttachUnauthorizedLabelKey,
      hintKey: kHostAttachUnauthorizedHintKey,
    ),
    AttachDispatchUnreachable() => const HostAttachNotice(
      labelKey: kHostAttachUnreachableLabelKey,
      hintKey: kHostAttachUnreachableHintKey,
    ),
    AttachDispatchUnavailable(:final hint) => HostAttachNotice(
      labelKey: kHostAttachUnreachableLabelKey,
      hintText: hint,
    ),
    AttachDispatchToolError(:final error) => HostAttachNotice(
      labelKey: kHostAttachToolErrorLabelKey,
      hintText: error.message.isEmpty ? error.code : error.message,
    ),
  };
}

/// Resolved badge/hint pair for a failed attach dispatch.
///
/// The label is always a catalog key. The hint is either a catalog key
/// ([hintKey]) or dynamic pass-through text from the daemon ([hintText])
/// — exactly one of the two is set, so the widget knows whether to
/// translate.
@immutable
class HostAttachNotice {
  const HostAttachNotice({required this.labelKey, this.hintKey, this.hintText})
    : assert(
        (hintKey == null) != (hintText == null),
        'exactly one of hintKey / hintText must be set',
      );

  final String labelKey;
  final String? hintKey;
  final String? hintText;

  @override
  int get hashCode => Object.hash(labelKey, hintKey, hintText);

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is HostAttachNotice &&
          runtimeType == other.runtimeType &&
          labelKey == other.labelKey &&
          hintKey == other.hintKey &&
          hintText == other.hintText;
}

class HostAttachNoticeBody extends ConsumerWidget {
  const HostAttachNoticeBody({required this.notice, super.key});

  final HostAttachNotice notice;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
    final hintKey = notice.hintKey;
    final hint = hintKey != null ? t(ref, hintKey) : notice.hintText!;
    return Padding(
      key: hostAttachNoticeBodyKey,
      padding: UpegSizing.pinBodyPadding,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.cloud_off_outlined, size: 12, color: tokens.warn),
              const SizedBox(width: 4),
              // Flexible + ellipsis: locale variants of the badge copy
              // differ in width and must squeeze inside the pin body.
              Flexible(
                child: Text(
                  t(ref, notice.labelKey),
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
            hint,
            style: TextStyle(
              fontFamily: upegMonoFontFamily,
              fontFamilyFallback: upegMonoFontFamilyFallback,
              fontSize: 9.5,
              color: tokens.fg4,
              height: 1.3,
            ),
            maxLines: 3,
            overflow: TextOverflow.ellipsis,
          ),
        ],
      ),
    );
  }
}
