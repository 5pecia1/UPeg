/// Pre-boot splash rendered while `initApp()` is in-flight.
///
/// User-facing surface: the upeg wordmark plus a minimal loading
/// indicator — no developer jargon. Intentionally NOT localized via
/// `t()`: this page paints before the FRB catalog (and thus the Rust
/// dylib) is guaranteed to be up, and "upeg" is a brand wordmark, not
/// translatable copy. On success the BoardPage simply paints over.
library;

import 'package:flutter/material.dart';

import 'package:upeg/src/theme/upeg_theme.dart';

/// Brand wordmark shown on the splash. A product name, not a catalog
/// string — it must read identically in every locale.
const String splashWordmark = 'upeg';

/// Widget key so tests (and the migration roadmap) can locate the
/// splash without depending on copy.
const Key splashWordmarkKey = Key('splash-wordmark');

class SplashPage extends StatelessWidget {
  const SplashPage({super.key});

  @override
  Widget build(BuildContext context) {
    return const Scaffold(
      body: Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              splashWordmark,
              key: splashWordmarkKey,
              style: TextStyle(
                fontFamily: upegMonoFontFamily,
                fontFamilyFallback: upegMonoFontFamilyFallback,
                fontSize: 28,
                fontWeight: FontWeight.w600,
                letterSpacing: 4,
              ),
            ),
            SizedBox(height: 24),
            SizedBox(
              width: 20,
              height: 20,
              child: CircularProgressIndicator(strokeWidth: 2),
            ),
          ],
        ),
      ),
    );
  }
}
