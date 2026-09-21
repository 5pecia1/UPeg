/// Unit tests for [TweaksController].
///
/// The FRB `loadTweaks` / `saveTweaks` calls are swapped via
/// [tweaksLoaderProvider] / [tweaksSaverProvider] so no native dylib
/// is loaded. We drive the controller directly through a
/// `ProviderContainer` and observe state transitions.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/tweaks_provider.dart';

const TweaksDto _initial = TweaksDto(
  theme: 'Light',
  accent: 'Green',
  showHoles: true,
  locale: 'En',
  localHttpHost: false,
);

const TweaksDto _next = TweaksDto(
  theme: 'Dark',
  accent: 'Amber',
  showHoles: false,
  locale: 'Ko',
  localHttpHost: false,
);

ProviderContainer _container({
  TweaksDto initial = _initial,
  void Function(TweaksDto)? onSave,
}) {
  return ProviderContainer(
    overrides: [
      tweaksLoaderProvider.overrideWith(
        (ref) =>
            () => initial,
      ),
      tweaksSaverProvider.overrideWith((ref) => onSave ?? (TweaksDto _) {}),
    ],
  );
}

void main() {
  group('TweaksController', () {
    test('TweaksController_build_seeds_state_from_loader_result', () async {
      final container = _container();
      addTearDown(container.dispose);

      final value = await container.read(tweaksProvider.future);
      expect(value, _initial);
    });

    test('TweaksController_save_updates_provider_state', () async {
      TweaksDto? saved;
      final container = _container(onSave: (t) => saved = t);
      addTearDown(container.dispose);

      // Force the AsyncNotifier to build before driving save().
      await container.read(tweaksProvider.future);

      final controller = container.read(tweaksProvider.notifier);
      await controller.save(_next);

      expect(saved, _next);
      expect(container.read(tweaksProvider).value, _next);
    });

    test(
      'TweaksController_save_exposes_AsyncError_on_saver_exception',
      () async {
        final container = _container(
          onSave: (_) => throw Exception('disk full'),
        );
        addTearDown(container.dispose);

        await container.read(tweaksProvider.future);

        final controller = container.read(tweaksProvider.notifier);
        await controller.save(_next);

        final state = container.read(tweaksProvider);
        expect(state, isA<AsyncError<TweaksDto>>());
        expect(state.hasError, isTrue);
      },
    );
  });
}
