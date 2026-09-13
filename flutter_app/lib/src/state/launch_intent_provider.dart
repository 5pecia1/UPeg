/// Pending [LaunchIntent] Riverpod provider.
///
/// Single observer pattern: cold-boot argv parsing
/// (`AppInitReport.launch`), the `app_links` second-instance event
/// stream, and the in-process popup encode all converge here. The
/// [`LaunchIntentApplier`] root widget reacts to non-null state by
/// dispatching the tool activation, then calling [clear] so a stale
/// intent doesn't re-fire on the next rebuild.
///
/// `LaunchIntent` is the Dart-side typed mirror of the FRB
/// `LaunchIntentDto`. We keep them as separate types so the UI layer
/// never has to import the auto-generated FRB file directly — and so
/// tests can construct an intent with a `const` constructor without
/// going through the dylib.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/deep_link.dart' show LaunchIntentDto;

/// Typed launch intent shared by the three intent sources.
///
/// `board` / `tool` / `inputJson` are intentionally nullable so the
/// observer can decide which sub-action to dispatch per call:
/// a board-only intent switches the selected board, a tool-only intent
/// fires `pinActivationFor`, a full intent does both.
class LaunchIntent {
  /// Board key to focus (e.g. `'dev'`). `null` ⇒ leave the current
  /// board selection alone.
  final String? board;

  /// Tool id to activate (e.g. `'num.hex_to_decimal'`). `null` ⇒ no
  /// tool activation.
  final ToolId? tool;

  /// Pre-fill args for the tool, encoded as a JSON object string.
  /// `null` ⇒ activate with `'{}'`.
  final String? inputJson;

  LaunchIntent({this.board, String? tool, this.inputJson})
    : tool = tool == null ? null : ToolId.parse(tool);

  const LaunchIntent.typed({this.board, this.tool, this.inputJson});

  /// Promote an FRB DTO into the Dart-side intent. Centralised here so
  /// callers don't sprinkle the field-by-field copy across listeners.
  factory LaunchIntent.fromDto(LaunchIntentDto dto) => LaunchIntent.typed(
    board: dto.board,
    tool: ToolId.tryParse(dto.tool),
    inputJson: dto.inputJson,
  );

  /// `true` when every field is empty — the observer treats this as a
  /// no-op so callers don't have to special-case "intent with nothing
  /// to do".
  bool get isEmpty => board == null && tool == null && inputJson == null;
}

/// Riverpod notifier holding the pending intent. `null` ⇒ idle; a
/// non-null value is consumed by [`LaunchIntentApplier`] on the next
/// frame and immediately [clear]ed.
class LaunchIntentNotifier extends Notifier<LaunchIntent?> {
  @override
  LaunchIntent? build() => null;

  /// Queue an intent for the observer. Overwrites any prior pending
  /// intent so the most recent source wins.
  void set(LaunchIntent intent) {
    state = intent;
  }

  /// Drop the pending intent once the observer has dispatched it.
  void clear() {
    state = null;
  }
}

final launchIntentProvider =
    NotifierProvider<LaunchIntentNotifier, LaunchIntent?>(
      LaunchIntentNotifier.new,
    );
