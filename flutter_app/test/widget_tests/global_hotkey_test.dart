/// Global summon hotkey — binding contract + install wiring.
///
/// The system-wide hotkey cannot fire in headless CI, so these tests
/// pin (1) the canonical binding constants (magic-string guard: the
/// binding lives in exactly one place) and (2) the install flow
/// through recording seams — hot-restart hygiene (`unregisterAll`
/// before `register`) and idempotence.
library;

import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hotkey_manager/hotkey_manager.dart';

import 'package:upeg/src/platform/global_hotkey.dart';

void main() {
  group('Summon hotkey binding', () {
    test('the_default_binding_is_ctrl_alt_space_at_system_scope', () {
      final hotKey = buildSummonHotKey();

      expect(hotKey.key, PhysicalKeyboardKey.space);
      expect(hotKey.modifiers, [HotKeyModifier.control, HotKeyModifier.alt]);
      expect(hotKey.scope, HotKeyScope.system);
    });

    test('the_identifier_is_a_fixed_constant_so_it_survives_hot_restart', () {
      // A constant identifier (not a random UUID) keeps re-registration and
      // unregistration deterministic.
      expect(buildSummonHotKey().identifier, kSummonHotkeyIdentifier);
      expect(buildSummonHotKey().identifier, buildSummonHotKey().identifier);
    });
  });

  group('UpegGlobalHotkey.install', () {
    test(
      'install_registers_the_summon_hotkey_after_unregisterall_and_reinvocation_is_a_no_op',
      () async {
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

        await container.read(installProvider);

        expect(calls, ['unregisterAll', 'register']);
        expect(registered?.identifier, kSummonHotkeyIdentifier);
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
      },
    );
  });
}
