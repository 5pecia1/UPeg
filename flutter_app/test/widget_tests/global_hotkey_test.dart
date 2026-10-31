/// Global summon hotkey — binding contract + install wiring.
///
/// The system-wide hotkey cannot fire in headless CI, so these tests
/// pin (1) the canonical binding per platform (magic-string guard: the
/// binding lives in exactly one place), (2) the key code the native plugin
/// receives — the root cause of the dead Linux Ctrl+Alt+Space — through the
/// plugin's public method channel, and (3) the install flow through
/// recording seams: hot-restart hygiene (`unregisterAll` before `register`),
/// idempotence, and a key-down handler whose toggle does not read focus.
library;

import 'dart:async' show FutureOr;
import 'dart:io' show Platform;

import 'package:flutter/foundation.dart'
    show TargetPlatform, debugDefaultTargetPlatformOverride;
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hotkey_manager/hotkey_manager.dart';

import 'package:upeg/src/platform/global_hotkey.dart';

const MethodChannel _hotkeyChannel = MethodChannel(
  'dev.leanflutter.plugins/hotkey_manager',
);
const MethodChannel _windowChannel = MethodChannel('window_manager');

const List<TargetPlatform> _desktopPlatforms = [
  TargetPlatform.linux,
  TargetPlatform.macOS,
  TargetPlatform.windows,
];

/// The `keyCode` the plugin sends to the native side when [hotKey] is
/// registered, recorded off its (mocked) method channel. The value comes
/// from the plugin's reverse lookup on the HOST platform, so callers skip
/// where the host is not the platform they pin.
Future<int?> _keyCodeSentFor(HotKey hotKey) async {
  final messenger =
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
  final calls = <MethodCall>[];
  messenger.setMockMethodCallHandler(_hotkeyChannel, (call) async {
    calls.add(call);
    return null;
  });
  addTearDown(() => messenger.setMockMethodCallHandler(_hotkeyChannel, null));

  await MethodChannelHotKeyManager().register(hotKey);

  final register = calls.singleWhere((call) => call.method == 'register');
  return (register.arguments as Map<Object?, Object?>)['keyCode'] as int?;
}

/// Runs [body] with `defaultTargetPlatform` pinned to [platform]; `flutter
/// test` otherwise reports android whatever the host is. The override is
/// cleared before returning, as `testWidgets` requires.
Future<T> _withPlatform<T>(
  TargetPlatform platform,
  FutureOr<T> Function() body,
) async {
  debugDefaultTargetPlatformOverride = platform;
  try {
    return await body();
  } finally {
    debugDefaultTargetPlatformOverride = null;
  }
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('Summon hotkey binding', () {
    test('the_default_key_is_U_on_linux_and_Space_on_macos_and_windows', () {
      expect(
        summonHotkeyKeyFor(TargetPlatform.linux),
        PhysicalKeyboardKey.keyU,
      );
      expect(
        summonHotkeyKeyFor(TargetPlatform.macOS),
        PhysicalKeyboardKey.space,
      );
      expect(
        summonHotkeyKeyFor(TargetPlatform.windows),
        PhysicalKeyboardKey.space,
      );
    });

    test('the_binding_is_ctrl_alt_plus_the_platform_key_at_system_scope', () {
      for (final platform in _desktopPlatforms) {
        final hotKey = buildSummonHotKey(platform: platform);

        expect(hotKey.key, summonHotkeyKeyFor(platform), reason: '$platform');
        expect(hotKey.modifiers, [
          HotKeyModifier.control,
          HotKeyModifier.alt,
        ], reason: '$platform');
        expect(hotKey.scope, HotKeyScope.system, reason: '$platform');
      }
    });

    test('the_binding_defaults_to_the_running_platform', () async {
      // Without a pinned platform `flutter test` reports android, so
      // comparing against the platform default alone could not tell the
      // Linux override from a plain Space.
      final keys = {
        for (final platform in _desktopPlatforms)
          platform: await _withPlatform(
            platform,
            () => buildSummonHotKey().key,
          ),
      };

      expect(keys, {
        TargetPlatform.linux: PhysicalKeyboardKey.keyU,
        TargetPlatform.macOS: PhysicalKeyboardKey.space,
        TargetPlatform.windows: PhysicalKeyboardKey.space,
      });
    });

    test('the_identifier_is_a_fixed_constant_so_it_survives_hot_restart', () {
      // A constant identifier (not a random UUID) keeps re-registration and
      // unregistration deterministic.
      expect(buildSummonHotKey().identifier, kSummonHotkeyIdentifier);
      expect(buildSummonHotKey().identifier, buildSummonHotKey().identifier);
    });
  });

  group('Key code the native plugin receives', () {
    const hostNote = 'the plugin maps keys with the HOST platform tables';

    test(
      'U_reaches_the_linux_plugin_as_the_canonical_keysym_0x75',
      () async {
        final keyCode = await _keyCodeSentFor(
          buildSummonHotKey(platform: TargetPlatform.linux),
        );

        expect(keyCode, 0x75);
      },
      skip: Platform.isLinux ? false : hostNote,
    );

    test(
      'Space_reaches_the_linux_plugin_as_KP_Space_which_X11_cannot_grab',
      () async {
        // The root cause of the dead Linux Ctrl+Alt+Space: the plugin's
        // reverse lookup finds only KP_Space (0xFF80 = 65408) for `space`,
        // and no regular keyboard produces that key. If a plugin update
        // changes this, the Linux override to U can be reconsidered.
        final keyCode = await _keyCodeSentFor(
          buildSummonHotKey(platform: TargetPlatform.macOS),
        );

        expect(keyCode, 65408);
      },
      skip: Platform.isLinux ? false : hostNote,
    );
  });

  group('UpegGlobalHotkey.install', () {
    testWidgets(
      'install_registers_the_summon_hotkey_once_and_its_handler_hides_a_shown_window_without_reading_focus',
      (tester) async {
        final messenger =
            TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
        final calls = <String>[];
        HotKey? registered;
        HotKeyHandler? capturedHandler;

        final container = ProviderContainer();
        addTearDown(container.dispose);
        final installProvider = Provider<Future<void>>(
          (ref) => UpegGlobalHotkey.install(
            ref,
            register: (hotKey, {keyDownHandler}) async {
              calls.add('register');
              registered = hotKey;
              capturedHandler = keyDownHandler;
            },
            unregisterAll: () async {
              calls.add('unregisterAll');
            },
          ),
        );

        await _withPlatform(
          TargetPlatform.linux,
          () => container.read(installProvider),
        );

        expect(calls, ['unregisterAll', 'register']);
        expect(registered?.identifier, kSummonHotkeyIdentifier);
        expect(
          registered?.key,
          PhysicalKeyboardKey.keyU,
          reason: 'install registers the running platform default',
        );
        expect(capturedHandler, isNotNull);

        // A second install must not double-register (hot reload guard).
        final container2 = ProviderContainer();
        addTearDown(container2.dispose);
        final reinstallProvider = Provider<Future<void>>(
          (ref) => UpegGlobalHotkey.install(
            ref,
            register: (hotKey, {keyDownHandler}) async {
              calls.add('register');
            },
            unregisterAll: () async {
              calls.add('unregisterAll');
            },
          ),
        );
        await container2.read(reinstallProvider);

        expect(calls, ['unregisterAll', 'register']);

        // The key-down handler runs the shared toggle, which does not read
        // focus: on X11 the hotkey's own key grab makes an active window
        // read as unfocused while the key is held, so a shown window hides
        // whatever focus would say. (Install is once per isolate, so this
        // check shares the test.)
        final windowCalls = <String>[];
        messenger.setMockMethodCallHandler(_windowChannel, (call) async {
          windowCalls.add(call.method);
          return switch (call.method) {
            'isVisible' => true,
            'isMinimized' => false,
            _ => null,
          };
        });
        addTearDown(
          () => messenger.setMockMethodCallHandler(_windowChannel, null),
        );

        capturedHandler!(registered!);
        await tester.pump();

        expect(windowCalls, contains('hide'));
        expect(windowCalls, isNot(contains('show')));
        expect(windowCalls, isNot(contains('isFocused')));
      },
    );
  });
}
