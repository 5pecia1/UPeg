/// Unit tests for [`initialWindowMode`] and [`isWindowManagerSupported`].
///
/// The env var gate is the load-bearing contract:
/// `UPEG_DESKTOP_POPUP=1` flips the
/// initial mode to popup, anything else falls back to the default
/// (full) mode. We inject a Map explicitly so the test stays
/// platform-channel-free.
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/foundation.dart' show kIsWeb;

import 'package:upeg/src/platform/window.dart';
import 'package:upeg/src/state/window_mode_provider.dart';

void main() {
  group('initialWindowMode', () {
    test('initialWindowMode는_빈_env에서_default_mode를_반환한다', () {
      expect(
        initialWindowMode(env: const <String, String>{}),
        kDefaultWindowMode,
      );
    });

    test('initialWindowMode는_UPEG_DESKTOP_POPUP_set시_popup_mode를_반환한다', () {
      expect(
        initialWindowMode(env: const {'UPEG_DESKTOP_POPUP': '1'}),
        WindowMode.popup,
      );
    });

    test('initialWindowMode는_빈문자열_env_값을_무시한다', () {
      // Empty string is treated as "unset" — an empty assignment
      // counts as unset for our purposes, matching the retired Rust
      // surface where `std::env::var` returned `Err(NotPresent)` for
      // a missing var.
      expect(
        initialWindowMode(env: const {'UPEG_DESKTOP_POPUP': ''}),
        kDefaultWindowMode,
      );
    });
  });

  group('window_size_constants', () {
    test('full_gui_기본_크기는_1280x800이다', () {
      // PRD §5.9: full dashboard window must be 1280x800 — large enough
      // for six 168px pin columns (1008px) plus chrome padding.
      expect(fullSizeForTesting.width, equals(1280.0));
      expect(fullSizeForTesting.height, equals(800.0));
    });

    test('popup_기본_크기는_400x500으로_유지된다', () {
      // PRD §5.9: popup window stays at 400x500 — compact frameless panel.
      expect(popupSizeForTesting.width, equals(400.0));
      expect(popupSizeForTesting.height, equals(500.0));
    });
  });

  group('isWindowManagerSupported', () {
    test('isWindowManagerSupported는_target에_맞는_지원여부를_반환한다', () {
      // VM tests run on a desktop host; browser tests run on Web. The
      // getter must be true for desktop targets and false for Web so
      // platform channels are never touched in the browser build.
      expect(isWindowManagerSupported, kIsWeb ? isFalse : isTrue);
    });
  });
}
