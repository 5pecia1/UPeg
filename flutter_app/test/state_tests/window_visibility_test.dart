/// Window visibility state: native window events -> [windowShownProvider].
///
/// The events come from a recording double, so no platform channel (and no
/// Flutter binding) is involved.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/platform/window.dart' show isWindowManagerSupported;
import 'package:upeg/src/platform/window_visibility.dart';

import '../test_helpers/fake_window_visibility_source.dart';

ProviderContainer _containerWith(WindowVisibilitySource? source) {
  final container = ProviderContainer(
    overrides: [windowVisibilitySourceProvider.overrideWithValue(source)],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  group('windowShownProvider', () {
    test('starts_shown_and_registers_exactly_one_listener', () {
      final source = FakeWindowVisibilitySource();
      final container = _containerWith(source);

      expect(container.read(windowShownProvider), isTrue);
      expect(source.listeners, hasLength(1));
    });

    test('hide_and_show_events_toggle_the_state', () {
      final source = FakeWindowVisibilitySource();
      final container = _containerWith(source);
      container.read(windowShownProvider);

      source.hide();
      expect(container.read(windowShownProvider), isFalse);

      source.show();
      expect(container.read(windowShownProvider), isTrue);
    });

    test('minimize_and_restore_events_toggle_the_state', () {
      final source = FakeWindowVisibilitySource();
      final container = _containerWith(source);
      container.read(windowShownProvider);

      source.minimize();
      expect(container.read(windowShownProvider), isFalse);

      source.restore();
      expect(container.read(windowShownProvider), isTrue);
    });

    test('maximize_after_minimize_counts_as_shown', () {
      // Windows brings a minimized, maximized window back as `maximize`, not
      // `restore`.
      final source = FakeWindowVisibilitySource();
      final container = _containerWith(source);
      container.read(windowShownProvider);

      source.minimize();
      expect(container.read(windowShownProvider), isFalse);

      source.maximize();
      expect(container.read(windowShownProvider), isTrue);
    });

    test('blur_and_focus_do_not_count_as_hiding', () {
      final source = FakeWindowVisibilitySource();
      final container = _containerWith(source);
      container.read(windowShownProvider);

      source.emit('blur');
      expect(container.read(windowShownProvider), isTrue);

      source.hide();
      source.emit('focus');
      expect(
        container.read(windowShownProvider),
        isFalse,
        reason: 'focus alone must not undo a hide',
      );
    });

    test('repeated_hide_events_notify_listeners_once', () {
      final source = FakeWindowVisibilitySource();
      final container = _containerWith(source);
      final seen = <bool>[];
      container.listen<bool>(windowShownProvider, (_, next) => seen.add(next));

      source
        ..hide()
        ..hide()
        ..show()
        ..show();

      expect(seen, [false, true]);
    });

    test('disposing_the_container_removes_the_listener', () {
      final source = FakeWindowVisibilitySource();
      final container = ProviderContainer(
        overrides: [windowVisibilitySourceProvider.overrideWithValue(source)],
      );
      container.read(windowShownProvider);
      expect(source.listeners, hasLength(1));

      container.dispose();

      expect(source.listeners, isEmpty);
    });

    test('without_a_source_the_window_counts_as_shown', () {
      final container = _containerWith(null);

      expect(container.read(windowShownProvider), isTrue);
    });
  });

  group('windowVisibilitySourceProvider', () {
    test('default_source_follows_window_manager_support', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      expect(
        container.read(windowVisibilitySourceProvider),
        isWindowManagerSupported
            ? isA<WindowManagerVisibilitySource>()
            : isNull,
      );
    });
  });
}
