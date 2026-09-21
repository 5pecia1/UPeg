/// Unit tests for [`isTraySupported`].
///
/// The tray menu-ID mapping lives in `_TrayKeys` private constants
/// which are exercised in production by the menu builder; here we
/// cover the public surface: the `isTraySupported` getter that gates
/// the actual platform call.
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/foundation.dart' show kIsWeb;

import 'package:upeg/src/platform/tray.dart';

void main() {
  group('isTraySupported', () {
    test('isTraySupported_returns_the_support_flag_matching_the_target', () {
      // VM tests run on a desktop host; browser tests run on Web. The
      // getter must be true for desktop targets and false for Web so
      // platform channels are never touched in the browser build.
      expect(isTraySupported, kIsWeb ? isFalse : isTrue);
    });
  });
}
