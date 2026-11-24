import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/platform/desktop_update.dart';
import 'package:upeg/src/state/desktop_update_provider.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUp(() => debugDefaultTargetPlatformOverride = TargetPlatform.linux);
  tearDown(() => debugDefaultTargetPlatformOverride = null);

  test(
    'should persist dismissal by release and retain manual access',
    () async {
      String? saved;
      var checks = 0;
      final container = ProviderContainer(
        overrides: [
          desktopUpdateServiceProvider.overrideWithValue(
            DesktopUpdateService(
              resolveLatest: () async {
                checks++;
                return Uri.parse(
                  'https://github.com/5pecia1/UPeg/releases/tag/v2.0.0',
                );
              },
            ),
          ),
          updateDismissalReaderProvider.overrideWithValue(() async => saved),
          updateDismissalWriterProvider.overrideWithValue((version) async {
            saved = version;
          }),
        ],
      );
      addTearDown(container.dispose);
      final notifier = container.read(desktopUpdateProvider.notifier);
      await notifier.check('1.0.0');
      expect(container.read(desktopUpdateProvider).unread, isTrue);
      await notifier.dismiss();
      expect(saved, '2.0.0');
      expect(container.read(desktopUpdateProvider).unread, isFalse);
      expect(container.read(desktopUpdateProvider).version, '2.0.0');
      await notifier.check('1.0.0');
      expect(checks, 1);
    },
  );

  test(
    'should allow retry after a failed operation and prevent duplicate work',
    () async {
      final gate = Completer<String?>();
      var calls = 0;
      final container = ProviderContainer(
        overrides: [
          desktopUpdateServiceProvider.overrideWithValue(
            DesktopUpdateService(
              platform: TargetPlatform.linux,
              resolveLatest: () async => Uri.parse(
                'https://github.com/5pecia1/UPeg/releases/tag/v2.0.0',
              ),
              nativeUpdate: (_) {
                calls++;
                return gate.future;
              },
            ),
          ),
          updateDismissalReaderProvider.overrideWithValue(() async => null),
        ],
      );
      addTearDown(container.dispose);
      final notifier = container.read(desktopUpdateProvider.notifier);
      await notifier.check('1.0.0');
      final first = notifier.perform();
      expect(container.read(desktopUpdateProvider).busy, isTrue);
      expect(await notifier.perform(), isNull);
      expect(calls, 1);
      gate.completeError(const DesktopUpdateException('failure'));
      expect(await first, isNull);
      final state = container.read(desktopUpdateProvider);
      expect(state.busy, isFalse);
      expect(state.failed, isTrue);
      expect(state.error, 'failure');
      expect(state.unread, isTrue);
    },
  );
}
