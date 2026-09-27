/// Inline editable body for a memo notepad pin (`memo.scratch`).
///
/// Renders the active memo as an in-place multiline text field. Editing
/// persists straight through [memosProvider] — there is no activation
/// step (a `Live` pin is always live). Because the field is a real
/// `EditableText`, the board's full key-yield guard
/// (`primaryFocusIsEditableText`) already routes typing here instead of
/// into board shortcuts.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/features/memos/memos_provider.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

/// Widget key for the notepad's text field — lets tests target the memo
/// editor without depending on the tool id.
const Key memoPinFieldKey = Key('memo-pin-field');

const String memoPinHintText = 'scratch memo…';

class MemoPinBody extends ConsumerStatefulWidget {
  const MemoPinBody({required this.pinKey, super.key});

  final PinKey pinKey;

  @override
  ConsumerState<MemoPinBody> createState() => _MemoPinBodyState();
}

class _MemoPinBodyState extends ConsumerState<MemoPinBody> {
  final TextEditingController _controller = TextEditingController();
  String? _boundKey;

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  /// Sync the controller with the active memo when the active key or its
  /// persisted body changes underneath us (e.g. `memo.create` switched
  /// the active memo). Guarded so we do not stomp the caret while the
  /// user is mid-edit of the same entry.
  void _syncController(String activeKey, String body) {
    if (_boundKey == activeKey && _controller.text == body) return;
    if (_boundKey != activeKey) {
      _boundKey = activeKey;
      _controller.text = body;
      _controller.selection = TextSelection.collapsed(
        offset: _controller.text.length,
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final tokens = context.upeg;
    final activeKey = ref.watch(activeMemoKeyProvider(widget.pinKey));
    final body = ref.watch(
      memosProvider.select((entries) {
        for (final entry in entries) {
          if (entry.key == activeKey) return entry.body;
        }
        return '';
      }),
    );
    _syncController(activeKey, body);

    return Padding(
      padding: UpegSizing.pinBodyPadding,
      child: TextField(
        key: memoPinFieldKey,
        controller: _controller,
        maxLines: null,
        expands: true,
        textAlignVertical: TextAlignVertical.top,
        onChanged: (value) =>
            ref.read(memosProvider.notifier).updateBody(activeKey, value),
        style: TextStyle(
          fontFamily: upegMonoFontFamily,
          fontFamilyFallback: upegMonoFontFamilyFallback,
          fontSize: 11,
          color: tokens.fg,
          height: 1.35,
        ),
        decoration: InputDecoration(
          isDense: true,
          border: InputBorder.none,
          hintText: memoPinHintText,
          hintStyle: TextStyle(
            fontFamily: upegMonoFontFamily,
            fontFamilyFallback: upegMonoFontFamilyFallback,
            fontSize: 11,
            color: tokens.fg4,
          ),
        ),
      ),
    );
  }
}
