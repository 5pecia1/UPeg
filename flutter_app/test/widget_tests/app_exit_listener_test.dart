/// Platform app-exit requests save pending drafts before native termination.
library;

import 'dart:ui' show AppExitResponse;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/platform/app_exit_listener.dart';
import 'package:upeg/src/state/inline_draft_provider.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

class _CancelAppExitObserver with WidgetsBindingObserver {
  @override
  Future<AppExitResponse> didRequestAppExit() async => AppExitResponse.cancel;
}

void main() {
  testWidgets('should save pending draft when native app exit is requested', (
    tester,
  ) async {
    final saves = <ToolArgs>[];
    final store = InlineDraftStore(
      save: (_, _, args) => saves.add(args),
      saveWindow: const Duration(minutes: 1),
    );
    addTearDown(store.dispose);
    await tester.pumpWidget(
      ProviderScope(
        overrides: [inlineDraftProvider.overrideWithValue(store)],
        child: const AppExitListener(child: SizedBox.shrink()),
      ),
    );

    final args = ToolArgs.fromJsonObject(const {'left': 'last edit'});
    final pin = (BoardKey.parse('dev'), PinId.parse('pin-a'));
    store.set(pin, args, toolId: ToolId.parse('text.pair'));
    expect(saves, isEmpty);

    expect(
      await WidgetsBinding.instance.handleRequestAppExit(),
      AppExitResponse.exit,
    );
    expect(saves, [args]);

    // A repeated native request must not write the same draft twice.
    await WidgetsBinding.instance.handleRequestAppExit();
    expect(saves, [args]);
  });

  testWidgets(
    'should keep handling drafts when another observer cancels exit',
    (tester) async {
      final saves = <ToolArgs>[];
      final store = InlineDraftStore(
        save: (_, _, args) => saves.add(args),
        saveWindow: const Duration(minutes: 1),
      );
      addTearDown(store.dispose);
      await tester.pumpWidget(
        ProviderScope(
          overrides: [inlineDraftProvider.overrideWithValue(store)],
          child: const AppExitListener(child: SizedBox.shrink()),
        ),
      );
      final cancelObserver = _CancelAppExitObserver();
      WidgetsBinding.instance.addObserver(cancelObserver);
      addTearDown(() => WidgetsBinding.instance.removeObserver(cancelObserver));

      final pin = (BoardKey.parse('dev'), PinId.parse('pin-a'));
      final first = ToolArgs.fromJsonObject(const {'left': 'first edit'});
      store.set(pin, first, toolId: ToolId.parse('text.pair'));
      expect(
        await WidgetsBinding.instance.handleRequestAppExit(),
        AppExitResponse.cancel,
      );
      expect(saves, [first]);

      // The app stays alive after cancellation and must save a later edit too.
      final second = ToolArgs.fromJsonObject(const {'left': 'later edit'});
      store.set(pin, second, toolId: ToolId.parse('text.pair'));
      expect(
        await WidgetsBinding.instance.handleRequestAppExit(),
        AppExitResponse.cancel,
      );
      expect(saves, [first, second]);
    },
  );

  testWidgets('should unregister exit handler when widget is disposed', (
    tester,
  ) async {
    final saves = <ToolArgs>[];
    final store = InlineDraftStore(
      save: (_, _, args) => saves.add(args),
      saveWindow: const Duration(minutes: 1),
    );
    addTearDown(store.dispose);
    await tester.pumpWidget(
      ProviderScope(
        overrides: [inlineDraftProvider.overrideWithValue(store)],
        child: const AppExitListener(child: SizedBox.shrink()),
      ),
    );
    await tester.pumpWidget(const SizedBox.shrink());

    final pin = (BoardKey.parse('dev'), PinId.parse('pin-a'));
    store.set(
      pin,
      ToolArgs.fromJsonObject(const {'left': 'still pending'}),
      toolId: ToolId.parse('text.pair'),
    );
    expect(
      await WidgetsBinding.instance.handleRequestAppExit(),
      AppExitResponse.exit,
    );
    expect(saves, isEmpty);
    store.clear(pin);
  });
}
