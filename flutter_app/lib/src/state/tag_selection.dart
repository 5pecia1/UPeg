/// Sealed `TagSelection` — typed wrapper around the tag filter.
///
/// The Rust side (`upeg_pegboard_ui::features::layouts`) uses the
/// magic string `"all"` to mean "no filter applied". That literal
/// collides if a tool ever ships a tag literally named `all`, and
/// `String == "all"` checks scatter through the UI.
///
/// This sealed class isolates the magic string to the FRB boundary
/// (via the `frbValue` getter) and the persistence boundary (via
/// `persistValue`). The rest of the Dart code switches on the variant
/// and the compiler enforces exhaustiveness.
library;

/// Sealed root. Two variants — [`TagAll`] (every tool) and
/// [`TagSpecific`] (one tag string). Use [`TagSelection.fromPersisted`]
/// to hydrate from `UiPrefsStore` and `pattern == const TagAll()`
/// (or `is TagAll`) to discriminate.
sealed class TagSelection {
  const TagSelection();

  /// The string the Rust FRB API expects for this selection. Equals
  /// `"all"` for [`TagAll`], the wrapped tag for [`TagSpecific`].
  /// Only the FRB-boundary helpers in `tag_provider.dart` need this;
  /// UI code should `switch` on the variant instead.
  String get frbValue;

  /// The value to persist via [`UiPrefsStore.writeSelectedTag`]. A
  /// `null` write distinguishes "user actively cleared the filter"
  /// from "no preference recorded yet" — both restore as [`TagAll`].
  String? get persistValue;

  /// Hydrate from a persisted value. `null` ⇒ [`TagAll`].
  factory TagSelection.fromPersisted(String? value) =>
      value == null ? const TagAll() : TagSpecific(value);
}

/// No filter applied. Singleton-ish via the `const` constructor —
/// `const TagAll() == const TagAll()` is `true`.
final class TagAll extends TagSelection {
  const TagAll();

  @override
  String get frbValue => _allLiteral;

  @override
  String? get persistValue => null;

  @override
  bool operator ==(Object other) => other is TagAll;

  @override
  int get hashCode => _allLiteral.hashCode;

  @override
  String toString() => 'TagAll()';
}

/// Filter by a single tag string. `tag` is whatever Rust returned
/// from `tag_options()` minus the leading `"all"` sentinel.
final class TagSpecific extends TagSelection {
  const TagSpecific(this.tag);

  final String tag;

  @override
  String get frbValue => tag;

  @override
  String? get persistValue => tag;

  @override
  bool operator ==(Object other) => other is TagSpecific && other.tag == tag;

  @override
  int get hashCode => tag.hashCode;

  @override
  String toString() => 'TagSpecific($tag)';
}

/// The FRB-side sentinel. Kept here as a private constant so this is
/// the **only** place in the Dart codebase that mentions the literal
/// `"all"`. If Rust ever swaps the sentinel, this is the single line
/// that needs updating.
const String _allLiteral = 'all';
