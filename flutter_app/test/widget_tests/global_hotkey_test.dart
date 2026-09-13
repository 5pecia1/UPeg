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
    test('기본_binding은_Ctrl_Alt_Space_system_scope다', () {
      final hotKey = buildSummonHotKey();

      expect(hotKey.key, PhysicalKeyboardKey.space);
      expect(hotKey.modifiers, [HotKeyModifier.control, HotKeyModifier.alt]);
      expect(hotKey.scope, HotKeyScope.system);
    });

    test('identifier는_고정_상수라_hot_restart에도_안정적이다', () {
      // 무작위 UUID가 아니라 상수 identifier여야 재등록/해제가 결정적이다.
      expect(buildSummonHotKey().identifier, kSummonHotkeyIdentifier);
      expect(buildSummonHotKey().identifier, buildSummonHotKey().identifier);
    });
  });

  group('UpegGlobalHotkey.install', () {
    test('unregisterAll_후_summon_hotkey를_등록하고_재호출은_no_op이다', () async {
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

      // 두 번째 install은 등록을 중복시키지 않는다 (hot reload 보호).
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
    });
  });
}
