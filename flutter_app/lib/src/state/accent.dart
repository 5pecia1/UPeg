/// Typed accent enum + FRB string boundary.
///
/// The Rust side ships accent as one of `Green`, `Amber`, `Cyan`,
/// `Pink` (see `upeg-frb/src/api/tweaks.rs::accent_to_str`). Consumer
/// widgets must NEVER pass that raw string around; [Accent.fromFrb] is
/// the single boundary that decodes it into the typed enum, and the
/// downstream theme/palette code switches on the variant.
///
/// Unknown variants fall back to [Accent.cyan] (the historical default
/// accent, same as `kDefaultAccent`) so a future FRB-side enum addition
/// degrades gracefully instead of crashing the UI.
library;

enum Accent {
  green('Green'),
  amber('Amber'),
  cyan('Cyan'),
  pink('Pink');

  const Accent(this.frbName);

  /// Canonical variant string used on the FRB wire — keep in lockstep
  /// with `accent_to_str` in `upeg-frb/src/api/tweaks.rs`.
  final String frbName;

  /// Decode the FRB-side variant string into the typed enum. Unknown
  /// inputs fall back to [Accent.cyan]; callers that need to reject
  /// invalid input should validate before calling.
  static Accent fromFrb(String s) {
    for (final a in values) {
      if (a.frbName == s) return a;
    }
    return Accent.cyan;
  }
}
