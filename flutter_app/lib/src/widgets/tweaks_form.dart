/// Settings panel body.
///
/// Layout contract:
///   * Section headers in uppercase fg-3,
///   * Radio rows render their options as segmented "gh" buttons —
///     the active option uses the `.gh.primary` accent fill,
///   * Toggle rows use a single ghost button that flips between
///     `on` / `off`,
///   * Backup row is a pair of ghost buttons.
///
/// Persistence still flows through `tweaksProvider`. Each radio /
/// toggle change writes immediately (live theme apply) so the user
/// doesn't have to press a Save button.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/keyboard/keyboard_command_resolver.dart';
import 'package:upeg/src/rust/api/keyboard.dart';
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/backup_section.dart';
import 'package:upeg/src/widgets/diagnostics_section.dart';
import 'package:upeg/src/widgets/host_attach_section.dart';
import 'package:upeg/src/widgets/project_context_section.dart';

/// Externally-stable handles for the "Local HTTP host" row. Tests grip
/// these instead of the localized copy, so a catalog edit never
/// cascades into the test suite.
const String kLocalHttpHostToggleKeyName = 'tweaks-local-http-host-toggle';
const Key kLocalHttpHostToggleKey = Key(kLocalHttpHostToggleKeyName);
const Key kLocalHttpHostHelpKey = Key('tweaks-local-http-host-help');

class TweaksForm extends ConsumerWidget {
  const TweaksForm({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AsyncValue<TweaksDto> persisted = ref.watch(tweaksProvider);
    return persisted.when(
      loading: () => const Padding(
        padding: EdgeInsets.all(24),
        child: Center(child: CircularProgressIndicator()),
      ),
      error: (err, _) => Padding(
        padding: const EdgeInsets.all(16),
        key: const Key('tweaks-form-error'),
        child: Text(
          t(ref, 'settings.load_failed', {'msg': '$err'}),
          style: TextStyle(color: context.upeg.warn),
        ),
      ),
      data: (loaded) => _TweaksFormBody(initial: loaded),
    );
  }
}

class _TweaksFormBody extends ConsumerStatefulWidget {
  const _TweaksFormBody({required this.initial});

  final TweaksDto initial;

  @override
  ConsumerState<_TweaksFormBody> createState() => _TweaksFormBodyState();
}

class _TweaksFormBodyState extends ConsumerState<_TweaksFormBody> {
  late TweaksDto _current = widget.initial;

  Future<void> _apply(TweaksDto next) async {
    setState(() => _current = next);
    await ref.read(tweaksProvider.notifier).save(next);
  }

  @override
  Widget build(BuildContext context) {
    final accents = ref.watch(supportedAccentsProvider)();
    final themes = ref.watch(supportedThemesProvider)();
    final locales = ref.watch(supportedLocalesProvider)();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _Section(
          label: t(ref, 'settings.section.project').toUpperCase(),
          child: const ProjectContextSection(),
        ),
        const SizedBox(height: 14),
        _Section(
          label: t(ref, 'settings.section.diagnostics').toUpperCase(),
          child: const DiagnosticsSection(),
        ),
        const SizedBox(height: 14),
        _Section(
          label: t(ref, 'settings.section.theme').toUpperCase(),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _RadioRow(
                label: t(ref, 'settings.radio.mode'),
                value: _current.theme,
                options: themes,
                keyName: 'tweaks-theme-radio',
                onChanged: (v) => _apply(_copyWith(_current, theme: v)),
              ),
              const SizedBox(height: 6),
              _RadioRow(
                label: t(ref, 'settings.radio.accent'),
                value: _current.accent,
                options: accents,
                keyName: 'tweaks-accent-radio',
                onChanged: (v) => _apply(_copyWith(_current, accent: v)),
              ),
            ],
          ),
        ),
        const SizedBox(height: 14),
        _Section(
          label: t(ref, 'settings.section.layout').toUpperCase(),
          child: _ToggleRow(
            label: t(ref, 'settings.toggle.peg_holes'),
            onLabel: t(ref, 'settings.toggle.on'),
            offLabel: t(ref, 'settings.toggle.off'),
            value: _current.showHoles,
            keyName: 'tweaks-show-holes-toggle',
            onChanged: (v) => _apply(_copyWith(_current, showHoles: v)),
          ),
        ),
        const SizedBox(height: 14),
        // The whole host control plane is one switch: ON makes this app
        // the local HTTP host at next boot, OFF leaves it a pure client
        // that attaches to whatever host is already running. Nothing to
        // poll — the preference is the state.
        _Section(
          label: t(ref, 'settings.section.host').toUpperCase(),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _ToggleRow(
                label: t(ref, 'settings.toggle.local_http_host'),
                onLabel: t(ref, 'settings.toggle.on'),
                offLabel: t(ref, 'settings.toggle.off'),
                value: _current.localHttpHost,
                keyName: kLocalHttpHostToggleKeyName,
                onChanged: (v) => _apply(_copyWith(_current, localHttpHost: v)),
              ),
              const SizedBox(height: 6),
              Text(
                key: kLocalHttpHostHelpKey,
                t(ref, 'settings.toggle.local_http_host_help'),
                style: TextStyle(fontSize: 10.5, color: context.upeg.fg3),
              ),
            ],
          ),
        ),
        const SizedBox(height: 14),
        _Section(
          label: t(ref, 'settings.section.language').toUpperCase(),
          child: _RadioRow(
            label: t(ref, 'settings.radio.locale'),
            value: _current.locale,
            options: locales,
            keyName: 'tweaks-locale-radio',
            onChanged: (v) => _apply(_copyWith(_current, locale: v)),
          ),
        ),
        const SizedBox(height: 14),
        _Section(
          label: t(ref, 'settings.section.backup').toUpperCase(),
          child: const BackupSection(),
        ),
        const SizedBox(height: 14),
        _Section(
          label: t(ref, 'settings.section.host_attach').toUpperCase(),
          child: const HostAttachSection(),
        ),
      ],
    );
  }
}

class _Section extends StatelessWidget {
  const _Section({required this.label, required this.child});
  final String label;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text(
          label,
          style: TextStyle(
            fontFamily: upegMonoFontFamily,
            fontFamilyFallback: upegMonoFontFamilyFallback,
            fontSize: 10,
            letterSpacing: 0.6,
            color: tokens.fg3,
          ),
        ),
        const SizedBox(height: 6),
        child,
      ],
    );
  }
}

class _RadioRow extends ConsumerStatefulWidget {
  const _RadioRow({
    required this.label,
    required this.value,
    required this.options,
    required this.keyName,
    required this.onChanged,
  });

  final String label;
  final String value;
  final List<String> options;
  final String keyName;
  final ValueChanged<String> onChanged;

  @override
  ConsumerState<_RadioRow> createState() => _RadioRowState();
}

class _RadioRowState extends ConsumerState<_RadioRow> {
  late final FocusNode _focusNode;

  @override
  void initState() {
    super.initState();
    _focusNode = FocusNode(debugLabel: widget.keyName);
  }

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
  }

  KeyEventResult _onKeyEvent(FocusNode node, KeyEvent event) {
    final cmd = resolveKeyboardCommand(
      ref,
      event,
      scope: KeyboardScopeDto.settings,
    );
    if (cmd is! KeyboardCommandDto_Move) return KeyEventResult.ignored;
    return switch (cmd.direction) {
      DirectionDto.right => _moveSelection(forward: true),
      DirectionDto.left => _moveSelection(forward: false),
      _ => KeyEventResult.ignored,
    };
  }

  KeyEventResult _moveSelection({required bool forward}) {
    final options = widget.options;
    if (options.isEmpty) {
      return KeyEventResult.ignored;
    }
    final current = options.indexOf(widget.value);
    final base = current == -1 ? 0 : current;
    final delta = forward ? 1 : -1;
    final next = options[(base + delta + options.length) % options.length];
    if (next != widget.value) {
      widget.onChanged(next);
    }
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    return Focus(
      focusNode: _focusNode,
      descendantsAreFocusable: false,
      descendantsAreTraversable: false,
      onFocusChange: (_) => setState(() {}),
      onKeyEvent: _onKeyEvent,
      child: DecoratedBox(
        key: Key('${widget.keyName}-focus'),
        decoration: BoxDecoration(
          border: Border.all(
            color: _focusNode.hasPrimaryFocus
                ? tokens.accent
                : Colors.transparent,
          ),
          borderRadius: BorderRadius.circular(UpegSizing.radius1),
        ),
        child: Row(
          children: [
            SizedBox(
              width: 56,
              child: Text(
                widget.label,
                style: TextStyle(
                  fontFamily: upegMonoFontFamily,
                  fontFamilyFallback: upegMonoFontFamilyFallback,
                  fontSize: 11,
                  color: tokens.fg3,
                ),
              ),
            ),
            const SizedBox(width: 6),
            Expanded(
              child: Wrap(
                spacing: 4,
                runSpacing: 4,
                children: [
                  for (final option in widget.options)
                    _OptionChip(
                      key: Key('${widget.keyName}-$option'),
                      label: option,
                      selected: option == widget.value,
                      onTap: () => widget.onChanged(option),
                    ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _ToggleRow extends ConsumerStatefulWidget {
  const _ToggleRow({
    required this.label,
    required this.onLabel,
    required this.offLabel,
    required this.value,
    required this.keyName,
    required this.onChanged,
  });

  final String label;
  final String onLabel;
  final String offLabel;
  final bool value;
  final String keyName;
  final ValueChanged<bool> onChanged;

  @override
  ConsumerState<_ToggleRow> createState() => _ToggleRowState();
}

class _ToggleRowState extends ConsumerState<_ToggleRow> {
  late final FocusNode _focusNode;

  @override
  void initState() {
    super.initState();
    _focusNode = FocusNode(debugLabel: widget.keyName);
  }

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
  }

  KeyEventResult _onKeyEvent(FocusNode node, KeyEvent event) {
    final cmd = resolveKeyboardCommand(
      ref,
      event,
      scope: KeyboardScopeDto.settings,
    );
    if (cmd is! KeyboardCommandDto_Move) return KeyEventResult.ignored;
    return switch (cmd.direction) {
      DirectionDto.left => _toggle(),
      DirectionDto.right => _toggle(),
      _ => KeyEventResult.ignored,
    };
  }

  KeyEventResult _toggle() {
    widget.onChanged(!widget.value);
    return KeyEventResult.handled;
  }

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    return Focus(
      focusNode: _focusNode,
      descendantsAreFocusable: false,
      descendantsAreTraversable: false,
      onFocusChange: (_) => setState(() {}),
      onKeyEvent: _onKeyEvent,
      child: DecoratedBox(
        key: Key('${widget.keyName}-focus'),
        decoration: BoxDecoration(
          border: Border.all(
            color: _focusNode.hasPrimaryFocus
                ? tokens.accent
                : Colors.transparent,
          ),
          borderRadius: BorderRadius.circular(UpegSizing.radius1),
        ),
        child: Row(
          children: [
            Expanded(
              child: Text(
                widget.label,
                style: TextStyle(
                  fontFamily: upegMonoFontFamily,
                  fontFamilyFallback: upegMonoFontFamilyFallback,
                  fontSize: 11,
                  color: tokens.fg2,
                ),
              ),
            ),
            _OptionChip(
              key: Key(widget.keyName),
              label: widget.value ? widget.onLabel : widget.offLabel,
              selected: widget.value,
              onTap: () => widget.onChanged(!widget.value),
            ),
          ],
        ),
      ),
    );
  }
}

class _OptionChip extends StatelessWidget {
  const _OptionChip({
    required this.label,
    required this.selected,
    required this.onTap,
    super.key,
  });

  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    final borderColor = selected ? tokens.accent : tokens.line;
    final bgColor = selected ? tokens.accent : Colors.transparent;
    final fgColor = selected ? tokens.onAccent : tokens.fg2;
    return InkWell(
      onTap: onTap,
      canRequestFocus: false,
      borderRadius: BorderRadius.circular(UpegSizing.radius1),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
        decoration: BoxDecoration(
          color: bgColor,
          border: Border.all(color: borderColor),
          borderRadius: BorderRadius.circular(UpegSizing.radius1),
        ),
        child: Text(
          label,
          style: TextStyle(
            fontFamily: upegMonoFontFamily,
            fontFamilyFallback: upegMonoFontFamilyFallback,
            fontSize: 11,
            color: fgColor,
            fontWeight: FontWeight.w500,
            height: 1.0,
          ),
        ),
      ),
    );
  }
}

TweaksDto _copyWith(
  TweaksDto src, {
  String? theme,
  String? accent,
  String? locale,
  bool? showHoles,
  bool? localHttpHost,
}) {
  return TweaksDto(
    theme: theme ?? src.theme,
    accent: accent ?? src.accent,
    showHoles: showHoles ?? src.showHoles,
    locale: locale ?? src.locale,
    localHttpHost: localHttpHost ?? src.localHttpHost,
  );
}
