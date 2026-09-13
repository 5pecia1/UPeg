/// Selection cursor state for the popup grid.
///
/// A single `final class` so callers can pattern-match on it and
/// extending the state (e.g. adding a `lastJump` field for animations)
/// doesn't break the existing surface. The bare-`int` alternative would
/// drift the meaning of the index across call sites.
library;

final class PopupSelection {
  /// Zero-based index into the effective hit list (search results or
  /// the default 6 board pins). Always non-negative; the notifier
  /// guarantees this by wrapping at the list bounds.
  final int index;

  const PopupSelection({this.index = 0});

  @override
  int get hashCode => index.hashCode;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is PopupSelection &&
          runtimeType == other.runtimeType &&
          index == other.index;
}
