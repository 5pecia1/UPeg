/// `?` keyboard-cheatsheet overlay.
///
/// Renders the shared binding catalog
/// (`keyboardBindingCatalogProvider` → `upeg_core::binding_catalog`)
/// instead of a hand-written shortcut table, so the overlay can never
/// drift from the resolver contract. Layout follows the palette
/// overlay idiom: a top-anchored card with an `esc` kbd chip, body
/// scrolls when the catalog outgrows the viewport.
///
/// Sections are the keyboard scopes; the section for [CheatsheetOverlay.currentScope]
/// is hoisted to the top so the keys that apply *right now* are the
/// first thing on screen. Focus-gated rows carry a dim "focused pin"
/// badge.
///
/// Key handling reuses the shared confirm scope (the same
/// `KeyboardScopeDto.confirmDelete` policy `QuitConfirmDialog` uses):
/// Esc/`n`/`q` cancel and Enter/`y`/F1 confirm both just close the
/// overlay — a cheatsheet has nothing to commit.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/keyboard/binding_catalog.dart';
import 'package:upeg/src/keyboard/keyboard_command_resolver.dart';
import 'package:upeg/src/platform/keyboard_label.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/no_transition_dialog.dart';

class CheatsheetOverlay extends ConsumerWidget {
  const CheatsheetOverlay({
    this.currentScope = KeyboardScopeDto.board,
    super.key,
  });

  /// Stable widget-test contract keys.
  static const Key overlayKey = Key('cheatsheet-overlay');

  static Key scopeSectionKey(KeyboardScopeDto scope) =>
      Key('cheatsheet-scope-${scope.name}');

  /// The scope the user was in when the overlay opened — its section is
  /// rendered first.
  final KeyboardScopeDto currentScope;

  KeyEventResult _onKeyEvent(
    BuildContext context,
    WidgetRef ref,
    KeyEvent event,
  ) {
    final cmd = resolveKeyboardCommand(
      ref,
      event,
      scope: KeyboardScopeDto.confirmDelete,
    );
    return switch (cmd) {
      KeyboardCommandDto_Confirm() ||
      KeyboardCommandDto_Cancel() => _close(context),
      _ => KeyEventResult.ignored,
    };
  }

  KeyEventResult _close(BuildContext context) {
    Navigator.of(context).pop();
    return KeyEventResult.handled;
  }

  /// Catalog sections with [currentScope] hoisted to the front,
  /// otherwise in canonical catalog order.
  List<ScopeBindingsDto> _orderedSections(List<ScopeBindingsDto> catalog) {
    return [
      for (final section in catalog)
        if (section.scope == currentScope) section,
      for (final section in catalog)
        if (section.scope != currentScope) section,
    ];
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final tokens = context.upeg;
    final sections = _orderedSections(
      ref.watch(keyboardBindingCatalogProvider),
    );
    return Focus(
      autofocus: true,
      onKeyEvent: (_, event) => _onKeyEvent(context, ref, event),
      child: Dialog(
        backgroundColor: Colors.transparent,
        alignment: Alignment.topCenter,
        insetPadding: const EdgeInsets.only(top: 64, left: 16, right: 16),
        elevation: 0,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 640, maxHeight: 560),
          child: Container(
            key: overlayKey,
            decoration: BoxDecoration(
              color: tokens.surface,
              border: Border.all(color: tokens.line),
              borderRadius: BorderRadius.circular(8),
              boxShadow: const [
                BoxShadow(
                  color: Color(0x66000000),
                  blurRadius: 60,
                  offset: Offset(0, 20),
                ),
              ],
            ),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                _CheatsheetHeader(tokens: tokens),
                Flexible(
                  child: ListView(
                    padding: const EdgeInsets.fromLTRB(14, 8, 14, 14),
                    children: [
                      for (final section in sections)
                        _ScopeSection(
                          key: scopeSectionKey(section.scope),
                          tokens: tokens,
                          section: section,
                        ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _CheatsheetHeader extends ConsumerWidget {
  const _CheatsheetHeader({required this.tokens});

  final UpegTokens tokens;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Container(
      padding: const EdgeInsets.fromLTRB(14, 12, 14, 12),
      decoration: BoxDecoration(
        border: Border(bottom: BorderSide(color: tokens.lineSoft)),
      ),
      child: Row(
        children: [
          Expanded(
            child: Text(
              t(ref, 'keys.title'),
              style: TextStyle(
                color: tokens.fg,
                fontSize: 13,
                fontWeight: FontWeight.w600,
                fontFamily: upegMonoFontFamily,
                fontFamilyFallback: upegMonoFontFamilyFallback,
              ),
            ),
          ),
          _Kbd(tokens: tokens, text: 'esc'),
          const SizedBox(width: 4),
          Text(
            t(ref, 'keys.footer.close'),
            style: TextStyle(color: tokens.fg4, fontSize: 10),
          ),
        ],
      ),
    );
  }
}

class _ScopeSection extends ConsumerWidget {
  const _ScopeSection({required this.tokens, required this.section, super.key});

  final UpegTokens tokens;
  final ScopeBindingsDto section;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.only(top: 12, bottom: 6),
          child: Text(
            t(ref, section.labelKey),
            style: TextStyle(
              color: tokens.fg3,
              fontSize: 11,
              fontWeight: FontWeight.w600,
              letterSpacing: 0.4,
            ),
          ),
        ),
        for (final entry in section.entries)
          _BindingRow(tokens: tokens, entry: entry),
      ],
    );
  }
}

class _BindingRow extends ConsumerWidget {
  const _BindingRow({required this.tokens, required this.entry});

  final UpegTokens tokens;
  final BindingEntryDto entry;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final platform = ref.watch(keyboardPlatformProvider);
    final caps = catalogEntryKeyCaps(platform, entry.bindings);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 220,
            child: Wrap(
              spacing: 4,
              runSpacing: 4,
              children: [
                for (final cap in caps) _Kbd(tokens: tokens, text: cap),
              ],
            ),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Padding(
              padding: const EdgeInsets.only(top: 2),
              child: Text.rich(
                TextSpan(
                  text: t(ref, entry.labelKey),
                  children: [
                    if (entry.requiresToolFocus)
                      TextSpan(
                        text: '  · ${t(ref, 'keys.requires_focus')}',
                        style: TextStyle(color: tokens.fg4, fontSize: 10),
                      ),
                  ],
                ),
                style: TextStyle(color: tokens.fg2, fontSize: 12),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _Kbd extends StatelessWidget {
  const _Kbd({required this.tokens, required this.text});

  final UpegTokens tokens;
  final String text;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 2),
      decoration: BoxDecoration(
        color: tokens.surface2,
        border: Border.all(color: tokens.line),
        borderRadius: BorderRadius.circular(3),
      ),
      child: Text(
        text,
        style: TextStyle(
          fontFamily: upegMonoFontFamily,
          fontFamilyFallback: upegMonoFontFamilyFallback,
          fontSize: 11,
          color: tokens.fg2,
          height: 1.0,
        ),
      ),
    );
  }
}

/// Helper for [BoardPage] / the `?` shortcut.
Future<void> showCheatsheetOverlay(
  BuildContext context, {
  KeyboardScopeDto currentScope = KeyboardScopeDto.board,
}) {
  return showNoTransitionDialog<void>(
    context: context,
    barrierColor: Colors.black54,
    builder: (_) => CheatsheetOverlay(currentScope: currentScope),
  );
}
