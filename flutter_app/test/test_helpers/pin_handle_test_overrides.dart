import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/tweaks_provider.dart';

/// Prevents pin handle widget tests from loading the native FRB library.
final pinHandleTestOverrides = [
  tweaksLoaderProvider.overrideWith(
    (ref) =>
        () => const TweaksDto(
          theme: 'Dark',
          accent: 'Green',
          showHoles: true,
          locale: 'En',
          localHttpHost: false,
        ),
  ),
  tweaksSaverProvider.overrideWith((ref) => (_) {}),
];
