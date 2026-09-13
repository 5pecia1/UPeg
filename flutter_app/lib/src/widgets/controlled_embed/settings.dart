/// Resolver: convert manifest `ControlledEmbedSettingsDto` into explicit
/// browser settings (user-agent + viewport) that `WebViewPanel` consumes.
///
/// The resolver distinguishes "omitted" from "explicit default": if the
/// manifest has no settings, the UA is left at the browser default; if
/// the user picks "default", that is a deliberate opt-in to the host UA
/// and is signalled via `hasUserAgentOverride == true`.
library;

import 'package:flutter/material.dart';
import 'package:upeg/src/rust/api/embed.dart' as frb;
import 'package:upeg/src/widgets/expanded_modal/webview_panel.dart'
    show kMobileUserAgent;

/// Preset viewport dimensions for Controlled Embed sessions.
///
/// Use `ViewPortPresetSizes.mobile` etc. when configuring a viewport
/// override for a pin.
abstract final class ViewPortPresetSizes {
  /// iPhone-style narrow viewport.
  static const Size mobile = Size(390, 844);

  /// iPad-style medium viewport.
  static const Size tablet = Size(768, 1024);

  /// Desktop-style wide viewport.
  static const Size desktop = Size(1366, 768);
}

/// Resolved browser settings that `WebViewPanel` applies.
///
/// `hasUserAgentOverride` is the gate: when `false` the UA stays at the
/// browser default; when `true` the UA is explicitly set (even to `null`
/// for the "default" choice, which clears any prior override).
@immutable
final class ResolvedBrowserSettings {
  const ResolvedBrowserSettings({
    required this.hasUserAgentOverride,
    this.userAgent,
    this.viewportSize,
  });

  /// When `false`, no UA change was requested in the manifest so the
  /// browser default applies. When `true`, the manifest had an explicit
  /// UA choice (even if that choice resolves to `null`).
  final bool hasUserAgentOverride;

  /// The UA string to pass to `WebViewController.setUserAgent`.
  /// `null` means the browser default.
  final String? userAgent;

  /// If non-null, the webview renderer is wrapped in a `SizedBox` of
  /// this exact size.
  final Size? viewportSize;
}

/// Convert a `ControlledEmbedSettingsDto` (usually loaded from the
/// manifest sidecar) into a `ResolvedBrowserSettings`.
///
/// Returns `null` when no settings exist at all — the caller should
/// treat this identically to `hasUserAgentOverride: false`.
ResolvedBrowserSettings? resolveBrowserSettings(
  frb.ControlledEmbedSettingsDto? settings,
) {
  if (settings == null) return null;

  String? resolvedUserAgent;
  bool hasUserAgentOverride = false;

  final ua = settings.userAgent;
  if (ua != null) {
    hasUserAgentOverride = true;
    resolvedUserAgent = switch (ua) {
      frb.ControlledEmbedUserAgentDto_MobileSafari() => kMobileUserAgent,
      frb.ControlledEmbedUserAgentDto_Custom(:final value) => value,
      frb.ControlledEmbedUserAgentDto_Default() => null,
    };
  }

  Size? resolvedViewport;
  final vp = settings.viewport;
  if (vp != null) {
    resolvedViewport = switch (vp) {
      frb.ControlledEmbedViewportDto_Preset(:final preset) => switch (preset) {
        frb.ControlledEmbedViewportPresetDto.mobile =>
          ViewPortPresetSizes.mobile,
        frb.ControlledEmbedViewportPresetDto.tablet =>
          ViewPortPresetSizes.tablet,
        frb.ControlledEmbedViewportPresetDto.desktop =>
          ViewPortPresetSizes.desktop,
      },
      frb.ControlledEmbedViewportDto_Custom(:final width, :final height) =>
        Size(width.toDouble(), height.toDouble()),
    };
  }

  return ResolvedBrowserSettings(
    hasUserAgentOverride: hasUserAgentOverride,
    userAgent: resolvedUserAgent,
    viewportSize: resolvedViewport,
  );
}
