/// Design tokens for the Flutter GUI surface.
///
/// The initial values were ported from the legacy Dioxus CSS tokens, but
/// Flutter is now the active owner. Widgets read
/// `theme.extension<UpegTokens>()` instead of hard-coding
/// `Color(0xFFXXXXXX)` everywhere.
///
/// Pin-kind accent colours follow `upeg_pegboard_ui::tool_meta::pin_color`
/// — the FRB layer hands the variant string to Dart (e.g. `"Inline"`) and
/// [UpegTokens.pinKindColor] resolves it locally. Out-of-band variants
/// fall back to `accent` so adding a new variant on the Rust side never
/// crashes Flutter — it just shows the default accent until Dart catches
/// up.
library;

import 'package:flutter/material.dart';
import 'package:upeg/src/rust/api/tools.dart' as rust_tools;
import 'package:upeg/src/state/accent.dart';

const Color _darkContrastText = Colors.black;
const Color _lightContrastText = Colors.white;
const double _contrastLuminanceOffset = 0.05;
const double _lightContrastLuminance = 1;

/// Pick the stronger of the explicit black/white text pair for an accent.
/// Mid-tone accent ramps cannot use one fixed foreground while preserving
/// WCAG contrast across every user-selectable accent.
Color upegHighContrastForeground(Color background) {
  final luminance = background.computeLuminance();
  final darkContrast =
      (luminance + _contrastLuminanceOffset) / _contrastLuminanceOffset;
  final lightContrast =
      (_lightContrastLuminance + _contrastLuminanceOffset) /
      (luminance + _contrastLuminanceOffset);
  return darkContrast >= lightContrast ? _darkContrastText : _lightContrastText;
}

/// Mirror of `enum PinKind` in `upeg-core`. Variant string parsing lives
/// next to the colour table so the FRB → Dart hop never has to grow a
/// dedicated DTO for this one enum.
enum UpegPinKind {
  inline,
  launcher,
  live,
  action,
  embed,
  controlledEmbed,
  chain,
  llm;

  static UpegPinKind? tryParse(String? s) {
    if (s == null) return null;
    final lower = s.toLowerCase();
    for (final v in values) {
      if (v.name == lower) return v;
    }
    return null;
  }

  /// Convert the FRB-generated `PinKindDto` into the local widget-side
  /// enum. Living next to [tryParse] keeps the colour-table neighbours
  /// where Pin/BoardCanvas can find them.
  static UpegPinKind fromDto(rust_tools.PinKindDto kind) {
    switch (kind) {
      case rust_tools.PinKindDto.inline:
        return UpegPinKind.inline;
      case rust_tools.PinKindDto.launcher:
        return UpegPinKind.launcher;
      case rust_tools.PinKindDto.live:
        return UpegPinKind.live;
      case rust_tools.PinKindDto.action:
        return UpegPinKind.action;
      case rust_tools.PinKindDto.embed:
        return UpegPinKind.embed;
      case rust_tools.PinKindDto.controlledEmbed:
        return UpegPinKind.controlledEmbed;
      case rust_tools.PinKindDto.chain:
        return UpegPinKind.chain;
      case rust_tools.PinKindDto.llm:
        return UpegPinKind.llm;
    }
  }

  String get label {
    switch (this) {
      case UpegPinKind.controlledEmbed:
        return 'CONTROLLEDEMBED';
      default:
        return name.toUpperCase();
    }
  }
}

/// Short-form invoker label rendered in the Pin footer. Mirrors
/// `upeg_core::Invoker::label()` — kept in Dart so a new Rust variant
/// rolls in via codegen without forcing every widget to know how to
/// stringify it.
String invokerDtoLabel(rust_tools.InvokerDto invoker) {
  switch (invoker) {
    case rust_tools.InvokerDto.function:
      return 'function';
    case rust_tools.InvokerDto.external_:
      return 'external';
    case rust_tools.InvokerDto.http:
      return 'http';
    case rust_tools.InvokerDto.static_:
      return 'static';
    case rust_tools.InvokerDto.embed:
      return 'embed';
    case rust_tools.InvokerDto.chain:
      return 'chain';
    case rust_tools.InvokerDto.llm:
      return 'llm';
    case rust_tools.InvokerDto.wasm:
      return 'wasm';
  }
}

/// Short-form trigger-source label. Mirrors `upeg_core::Source` so the
/// modal footer reports the tool's real source (`timer`, `manual`,
/// `shortcut`, …) instead of a hardcoded claim. Kept in Dart so a new
/// Rust variant rolls in via codegen without editing every widget.
String sourceDtoLabel(rust_tools.SourceDto source) {
  return switch (source) {
    rust_tools.SourceDto_UserInput() => 'user input',
    rust_tools.SourceDto_Timer(:final intervalMs) => 'timer ${intervalMs}ms',
    rust_tools.SourceDto_Shortcut(:final keys) => 'shortcut $keys',
    rust_tools.SourceDto_Manual() => 'manual',
    rust_tools.SourceDto_Static() => 'static',
  };
}

/// Token bag for the upeg design system. The values started as a port
/// of the legacy CSS `:root` selector and now live here as Flutter's
/// active design-token source.
@immutable
class UpegTokens extends ThemeExtension<UpegTokens> {
  const UpegTokens({
    required this.bg,
    required this.bg2,
    required this.surface,
    required this.surface2,
    required this.line,
    required this.lineSoft,
    required this.fg,
    required this.fg2,
    required this.fg3,
    required this.fg4,
    required this.accent,
    required this.accent2,
    required this.onAccent,
    required this.warn,
    required this.onWarn,
    required this.pin,
    required this.focusRing,
    required this.hole,
    required this.holeDeep,
    required this.pinInline,
    required this.pinLauncher,
    required this.pinLive,
    required this.pinAction,
    required this.pinEmbed,
    required this.pinControlledEmbed,
    required this.pinChain,
    required this.pinLlm,
  });

  // Surface palette
  final Color bg;
  final Color bg2;
  final Color surface;
  final Color surface2;
  final Color line;
  final Color lineSoft;

  // Foreground (text) ramp
  final Color fg;
  final Color fg2;
  final Color fg3;
  final Color fg4;

  // Brand accents
  final Color accent;
  final Color accent2;

  /// Foreground ink painted on accent-filled surfaces (Run button,
  /// selected chips, `ok` badges). Recomputed per accent variant in
  /// [UpegTheme.forAccent] via [onFill] so every accent × brightness
  /// pair clears WCAG AA (>= [minAaContrast]).
  final Color onAccent;
  final Color warn;

  /// Foreground ink painted on warn-filled surfaces (`error` badges,
  /// warning banners). See [onAccent] for the contrast contract.
  final Color onWarn;
  final Color pin;

  /// Keyboard-focus ring ink. Deliberately NOT derived from `accent`
  /// or any per-pin colour override: the ring is painted with an
  /// offset gap against [bg]/[surface], and this token is pinned to
  /// the top of the foreground ramp so the pair always clears the
  /// WCAG 2.x non-text minimum ([minNonTextContrast]) in both themes
  /// regardless of the user's accent choice or `pinColorOverride`.
  final Color focusRing;

  // Pegboard hole decoration
  final Color hole;
  final Color holeDeep;

  // PinKind-specific colours (mirrors `tool_meta::pin_color`).
  final Color pinInline;
  final Color pinLauncher;
  final Color pinLive;
  final Color pinAction;
  final Color pinEmbed;
  final Color pinControlledEmbed;
  final Color pinChain;
  final Color pinLlm;

  Color pinKindColor(UpegPinKind? kind) {
    switch (kind) {
      case UpegPinKind.inline:
        return pinInline;
      case UpegPinKind.launcher:
        return pinLauncher;
      case UpegPinKind.live:
        return pinLive;
      case UpegPinKind.action:
        return pinAction;
      case UpegPinKind.embed:
        return pinEmbed;
      case UpegPinKind.controlledEmbed:
        return pinControlledEmbed;
      case UpegPinKind.chain:
        return pinChain;
      case UpegPinKind.llm:
        return pinLlm;
      case null:
        return accent;
    }
  }

  /// WCAG 2.x AA minimum contrast ratio for normal-size text.
  static const double minAaContrast = 4.5;

  /// WCAG 2.x minimum contrast for non-text UI (1.4.11) and large
  /// text (1.4.3). Always-visible status information (status bar
  /// copy, pin footer state) must clear this against its backdrop —
  /// `fg4` does not, so those call sites read `fg3` instead.
  static const double minNonTextContrast = 3.0;

  /// The near-black ink historically painted on accent fills. Kept as
  /// the preferred [onFill] candidate so the bright dark-theme accents
  /// keep their established look — the golden baselines under
  /// `test/goldens/` pin these exact pixels.
  static const Color _darkInk = Color(0xFF07120A);

  /// WCAG 2.x contrast ratio between two opaque colours (1..21).
  static double contrastRatio(Color a, Color b) {
    final la = a.computeLuminance();
    final lb = b.computeLuminance();
    final lighter = la >= lb ? la : lb;
    final darker = la >= lb ? lb : la;
    return (lighter + 0.05) / (darker + 0.05);
  }

  /// Foreground ink for an arbitrary opaque fill colour.
  ///
  /// Prefers the brand [_darkInk] when it clears WCAG AA against
  /// [fill]; otherwise falls back to plain black or white — whichever
  /// has more contrast headroom. The fallback is always AA-safe: the
  /// better of black/white never drops below ~4.58:1 for any opaque
  /// sRGB fill.
  static Color onFill(Color fill) {
    if (contrastRatio(fill, _darkInk) >= minAaContrast) return _darkInk;
    return contrastRatio(fill, Colors.black) >=
            contrastRatio(fill, Colors.white)
        ? Colors.black
        : Colors.white;
  }

  /// Dark theme token set.
  static const UpegTokens dark = UpegTokens(
    bg: Color(0xFF0A0B0D),
    bg2: Color(0xFF0F1114),
    surface: Color(0xFF14171C),
    surface2: Color(0xFF1A1E24),
    line: Color(0xFF23282F),
    lineSoft: Color(0xFF1C2026),
    fg: Color(0xFFE8EBEF),
    fg2: Color(0xFFB6BCC4),
    fg3: Color(0xFF7A818B),
    fg4: Color(0xFF4A5159),
    // oklch(0.78 0.16 145) ≈ #5BD16C
    accent: Color(0xFF5BD16C),
    // oklch(0.78 0.16 75) ≈ #D9B14C
    accent2: Color(0xFFD9B14C),
    // 9.8:1 against #5BD16C — see onFill
    onAccent: _darkInk,
    // oklch(0.72 0.18 35) ≈ #E08260
    warn: Color(0xFFE08260),
    // 6.8:1 against #E08260
    onWarn: _darkInk,
    // oklch(0.78 0.16 35) ≈ #E89478
    pin: Color(0xFFE89478),
    // Top of the dark fg ramp — ≥15:1 against bg/surface.
    focusRing: Color(0xFFE8EBEF),
    hole: Color(0x0DFFFFFF), // rgba(255,255,255,0.05)
    holeDeep: Color(0x73000000), // rgba(0,0,0,0.45)
    // PinKind palette — see tool_meta::pin_color
    pinInline: Color(0xFF5BD16C), // var(--accent)
    pinLauncher: Color(0xFFD58CE0), // oklch(0.78 0.13 320)
    pinLive: Color(0xFFD9B14C), // var(--accent-2)
    pinAction: Color(0xFFE89478), // var(--pin)
    pinEmbed: Color(0xFF7DAEE3), // oklch(0.78 0.13 250)
    pinControlledEmbed: Color(
      0xFF44C5B8,
    ), // oklch(0.74 0.15 200) — distinct teal
    pinChain: Color(0xFF5BCFC4), // oklch(0.78 0.12 180)
    pinLlm: Color(0xFFEFA67D), // oklch(0.80 0.14 35)
  );

  /// Light theme token set.
  static const UpegTokens light = UpegTokens(
    bg: Color(0xFFF4F3EE),
    bg2: Color(0xFFEFEEE8),
    surface: Color(0xFFFFFFFF),
    surface2: Color(0xFFFAF9F5),
    line: Color(0xFFD9D6CE),
    lineSoft: Color(0xFFE6E3DA),
    fg: Color(0xFF1A1A18),
    fg2: Color(0xFF3D3D39),
    fg3: Color(0xFF6D6D66),
    fg4: Color(0xFF9A9A92),
    // oklch(0.55 0.16 145) ≈ #2E8C44
    accent: Color(0xFF2E8C44),
    // oklch(0.62 0.16 60) ≈ #B07832
    accent2: Color(0xFFB07832),
    // 4.5:1 against #2E8C44 — see onFill
    onAccent: _darkInk,
    warn: Color(0xFFBF5230),
    // 4.7:1 against #BF5230 (the dark ink only reaches 4.06:1 there)
    onWarn: Colors.white,
    // oklch(0.58 0.18 35) ≈ #BA5832
    pin: Color(0xFFBA5832),
    // Top of the light fg ramp — ≥14:1 against bg/surface.
    focusRing: Color(0xFF1A1A18),
    hole: Color(0x12000000), // rgba(0,0,0,0.07)
    holeDeep: Color(0x99FFFFFF), // rgba(255,255,255,0.6)
    pinInline: Color(0xFF2E8C44),
    pinLauncher: Color(0xFF9A4FB0),
    pinLive: Color(0xFFB07832),
    pinAction: Color(0xFFBA5832),
    pinEmbed: Color(0xFF3F77B5),
    pinControlledEmbed: Color(0xFF1E8C82),
    pinChain: Color(0xFF2E9E91),
    pinLlm: Color(0xFFC9854E),
  );

  @override
  UpegTokens copyWith({
    Color? bg,
    Color? bg2,
    Color? surface,
    Color? surface2,
    Color? line,
    Color? lineSoft,
    Color? fg,
    Color? fg2,
    Color? fg3,
    Color? fg4,
    Color? accent,
    Color? accent2,
    Color? onAccent,
    Color? warn,
    Color? onWarn,
    Color? pin,
    Color? focusRing,
    Color? hole,
    Color? holeDeep,
    Color? pinInline,
    Color? pinLauncher,
    Color? pinLive,
    Color? pinAction,
    Color? pinEmbed,
    Color? pinControlledEmbed,
    Color? pinChain,
    Color? pinLlm,
  }) {
    return UpegTokens(
      bg: bg ?? this.bg,
      bg2: bg2 ?? this.bg2,
      surface: surface ?? this.surface,
      surface2: surface2 ?? this.surface2,
      line: line ?? this.line,
      lineSoft: lineSoft ?? this.lineSoft,
      fg: fg ?? this.fg,
      fg2: fg2 ?? this.fg2,
      fg3: fg3 ?? this.fg3,
      fg4: fg4 ?? this.fg4,
      accent: accent ?? this.accent,
      accent2: accent2 ?? this.accent2,
      onAccent: onAccent ?? this.onAccent,
      warn: warn ?? this.warn,
      onWarn: onWarn ?? this.onWarn,
      pin: pin ?? this.pin,
      focusRing: focusRing ?? this.focusRing,
      hole: hole ?? this.hole,
      holeDeep: holeDeep ?? this.holeDeep,
      pinInline: pinInline ?? this.pinInline,
      pinLauncher: pinLauncher ?? this.pinLauncher,
      pinLive: pinLive ?? this.pinLive,
      pinAction: pinAction ?? this.pinAction,
      pinEmbed: pinEmbed ?? this.pinEmbed,
      pinControlledEmbed: pinControlledEmbed ?? this.pinControlledEmbed,
      pinChain: pinChain ?? this.pinChain,
      pinLlm: pinLlm ?? this.pinLlm,
    );
  }

  @override
  UpegTokens lerp(ThemeExtension<UpegTokens>? other, double t) {
    if (other is! UpegTokens) return this;
    Color l(Color a, Color b) => Color.lerp(a, b, t) ?? a;
    return UpegTokens(
      bg: l(bg, other.bg),
      bg2: l(bg2, other.bg2),
      surface: l(surface, other.surface),
      surface2: l(surface2, other.surface2),
      line: l(line, other.line),
      lineSoft: l(lineSoft, other.lineSoft),
      fg: l(fg, other.fg),
      fg2: l(fg2, other.fg2),
      fg3: l(fg3, other.fg3),
      fg4: l(fg4, other.fg4),
      accent: l(accent, other.accent),
      accent2: l(accent2, other.accent2),
      onAccent: l(onAccent, other.onAccent),
      warn: l(warn, other.warn),
      onWarn: l(onWarn, other.onWarn),
      pin: l(pin, other.pin),
      focusRing: l(focusRing, other.focusRing),
      hole: l(hole, other.hole),
      holeDeep: l(holeDeep, other.holeDeep),
      pinInline: l(pinInline, other.pinInline),
      pinLauncher: l(pinLauncher, other.pinLauncher),
      pinLive: l(pinLive, other.pinLive),
      pinAction: l(pinAction, other.pinAction),
      pinEmbed: l(pinEmbed, other.pinEmbed),
      pinControlledEmbed: l(pinControlledEmbed, other.pinControlledEmbed),
      pinChain: l(pinChain, other.pinChain),
      pinLlm: l(pinLlm, other.pinLlm),
    );
  }
}

/// Convenience getter: pulls [UpegTokens] off the ambient theme.
///
/// Falls back to [UpegTokens.dark] when the extension is missing so
/// widget tests that don't install the theme still render meaningful
/// colours. Production callers always go through [UpegTheme.themeData].
extension UpegTokensX on BuildContext {
  UpegTokens get upeg =>
      Theme.of(this).extension<UpegTokens>() ?? UpegTokens.dark;
}

/// Spacing scale + radii + font sizes for the Flutter GUI.
/// Kept as compile-time constants rather than a theme extension because
/// they don't vary by theme mode.
class UpegSizing {
  const UpegSizing._();

  /// Pegboard column width. Paired with the fixed `BOARD_COLS` column
  /// count (`upeg_core::BOARD_COLS`) to size the grid; this Dart constant
  /// is the source of truth for the pixel value, there is no Rust-side
  /// mirror.
  static const double pinCellWidth = 168;

  /// Pegboard row height for a 1-unit pin.
  static const double pinCellHeight = 118;

  /// Spacing between pin cells.
  static const double pinGap = 8;

  /// Tab bar height.
  static const double tabBarHeight = 38;

  /// Tag chip strip height.
  static const double tagRowHeight = 34;

  /// Status bar height (fixed bottom).
  static const double statusBarHeight = 26;

  /// Pegboard outer padding.
  static const double pegboardPadding = 24;

  /// Border radii — mirror `--radius-1/2/3` from input.css.
  static const double radius1 = 4;
  static const double radius2 = 6;
  static const double radius3 = 10;

  /// Pin internal padding.
  static const EdgeInsets pinHeaderPadding = EdgeInsets.symmetric(
    horizontal: 8,
    vertical: 6,
  );
  static const EdgeInsets pinBodyPadding = EdgeInsets.all(10);
  static const EdgeInsets pinFooterPadding = EdgeInsets.symmetric(
    horizontal: 8,
    vertical: 4,
  );
}

/// Monospace family preference — JetBrains Mono first, then the bundled
/// D2Coding for Hangul glyphs (JetBrains Mono has none), then OS
/// fallbacks. Both families ship as assets (see pubspec.yaml `fonts:`).
const List<String> upegMonoFontFamilyFallback = [
  'D2Coding',
  'SF Mono',
  'Menlo',
  'Consolas',
  'monospace',
];
const String upegMonoFontFamily = 'JetBrainsMono';

/// Builds the [ThemeData] the app installs at root, wiring [UpegTokens]
/// into both Material primitives (so `Theme.of` colour properties stay
/// usable) and the [ThemeExtension] graph.
///
/// The [forAccent] factory is the single source of truth: it takes a
/// typed [Accent] + [Brightness] and produces a complete [ThemeData]
/// with the accent-coloured palette applied to `UpegTokens.accent`.
/// `MaterialApp` callers must NEVER hardcode an accent — they should
/// always go through `forAccent` (typically driven by `accentProvider`).
class UpegTheme {
  const UpegTheme._();

  /// Build the [ThemeData] for the given accent + brightness. The
  /// accent variant resolves to a per-brightness sRGB colour via
  /// [accentPaletteColor]; the rest of the token bag inherits from
  /// [UpegTokens.dark] / [UpegTokens.light] respectively.
  static ThemeData forAccent(Accent accent, {required Brightness brightness}) {
    final base = brightness == Brightness.dark
        ? UpegTokens.dark
        : UpegTokens.light;
    final accentColor = accentPaletteColor(accent, brightness);
    final tokens = base.copyWith(
      accent: accentColor,
      onAccent: UpegTokens.onFill(accentColor),
    );
    return _build(tokens, brightness);
  }

  /// Test-only convenience: dark theme with the Green accent. Mirrors
  /// the pre-batch hardcoded palette so widget tests that pinned the
  /// historical token bag keep painting the same colours. Production
  /// must always go through [forAccent] driven by `accentProvider`.
  @visibleForTesting
  static ThemeData darkTheme() =>
      forAccent(Accent.green, brightness: Brightness.dark);

  /// Test-only convenience: light theme with the Green accent. See
  /// [darkTheme] for the rationale.
  @visibleForTesting
  static ThemeData lightTheme() =>
      forAccent(Accent.green, brightness: Brightness.light);

  /// Resolve the accent variant to its canonical sRGB colour for the
  /// given brightness. Values mirror
  /// `upeg_pegboard_ui::features::tweaks::accent_color_for` (which
  /// returns CSS `oklch(...)` literals) — see `pegboard-ui` for the
  /// canonical OKLCH source.
  static Color accentPaletteColor(Accent accent, Brightness brightness) {
    final dark = brightness == Brightness.dark;
    return switch (accent) {
      // oklch(0.78 0.16 145) / oklch(0.55 0.16 145)
      Accent.green => dark ? const Color(0xFF5BD16C) : const Color(0xFF2E8C44),
      // oklch(0.82 0.16 75) / oklch(0.58 0.16 60)
      Accent.amber => dark ? const Color(0xFFE9B541) : const Color(0xFFA46A2A),
      // oklch(0.80 0.13 200) / oklch(0.58 0.13 210)
      Accent.cyan => dark ? const Color(0xFF5BC5D1) : const Color(0xFF2C7C90),
      // oklch(0.78 0.16 0) / oklch(0.58 0.16 0)
      Accent.pink => dark ? const Color(0xFFE58FA1) : const Color(0xFFB2415A),
    };
  }

  static ThemeData _build(UpegTokens t, Brightness brightness) {
    final base = ThemeData(
      brightness: brightness,
      useMaterial3: true,
      // Material colour scheme — kept so widgets that read `colorScheme`
      // (e.g. SnackBar, ProgressIndicator) get on-brand defaults.
      colorScheme: ColorScheme(
        brightness: brightness,
        primary: t.accent,
        onPrimary: t.onAccent,
        secondary: t.accent2,
        onSecondary: upegHighContrastForeground(t.accent2),
        surface: t.surface,
        onSurface: t.fg,
        surfaceContainerHighest: t.surface2,
        error: t.warn,
        onError: t.onWarn,
      ),
      scaffoldBackgroundColor: t.bg,
      canvasColor: t.bg,
      dividerColor: t.line,
      iconTheme: IconThemeData(color: t.fg2, size: 16),
      fontFamily: upegMonoFontFamily,
      fontFamilyFallback: upegMonoFontFamilyFallback,
      textTheme: _buildTextTheme(t),
    );
    return base.copyWith(extensions: <ThemeExtension<dynamic>>[t]);
  }

  static TextTheme _buildTextTheme(UpegTokens t) {
    return TextTheme(
      // Generic body
      bodyLarge: TextStyle(color: t.fg, fontSize: 13),
      bodyMedium: TextStyle(color: t.fg2, fontSize: 12),
      bodySmall: TextStyle(color: t.fg3, fontSize: 10.5),
      // Labels (used by pin chrome)
      labelLarge: TextStyle(
        color: t.fg2,
        fontSize: 12,
        fontWeight: FontWeight.w500,
      ),
      labelMedium: TextStyle(color: t.fg3, fontSize: 11),
      labelSmall: TextStyle(color: t.fg3, fontSize: 10, letterSpacing: 0.4),
      // Titles (modal headers etc.)
      titleLarge: TextStyle(
        color: t.fg,
        fontSize: 14,
        fontWeight: FontWeight.w600,
      ),
      titleMedium: TextStyle(
        color: t.fg,
        fontSize: 13,
        fontWeight: FontWeight.w500,
      ),
      titleSmall: TextStyle(color: t.fg2, fontSize: 12),
    );
  }
}
