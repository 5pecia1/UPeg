/// Shared fixtures for the `popup_page_*_test.dart` family: the popup
/// provider scope, the recording window-mode / hider doubles, and the
/// palette-hit fixtures the popup grid is rendered from.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart' show Override;
import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/pages/popup_page.dart';
import 'package:upeg/src/rust/api/palette.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart' show PinKindDto, ToolDto;
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/rust/api/pause.dart' show PausedStateDto;
import 'package:upeg/src/rust/api/status.dart'
    show McpImportPhaseDto, NetworkReachabilityDto, NetworkStatusDto;
import 'package:upeg/src/rust/api/pin_activation.dart';
import 'package:upeg/src/state/pin_activation_provider.dart';
import 'package:upeg/src/state/status_provider.dart';
import 'package:upeg/src/widgets/popup_auto_hide_observer.dart';

import 'fake_keyboard_resolver.dart';
import 'pegboard_selection_overrides.dart';

// Both page families assert on the same window-mode toggle; the recorder
// they share lives in one file (see window_mode_recorder.dart).
export 'window_mode_recorder.dart';

import 'i18n_test_catalog.dart';

class FixedStatusNotifier extends StatusNotifier {
  FixedStatusNotifier(this._snapshot);

  final StatusSnapshotDto _snapshot;

  @override
  StatusSnapshotDto build() => _snapshot;
}

class RecordingWindowHider extends WindowHider {
  int hideCount = 0;

  @override
  Future<void> hide() async {
    hideCount += 1;
  }
}

StatusSnapshotDto statusWithVersion(String version) => StatusSnapshotDto(
  network: const NetworkStatusDto(
    reachability: NetworkReachabilityDto.loopbackOnly,
    label: 'loopback-only',
  ),
  paused: PausedStateDto.running,
  mcpImportCount: 0,
  mcpImportPhase: McpImportPhaseDto.notStarted,
  buildVersion: version,
);

PaletteHit paletteHit(String id, String label, {String description = ''}) =>
    PaletteHit(
      id: id,
      label: label,
      description: description,
      score: 0,
      pinKind: PinKindDto.inline,
    );

List<PaletteHit> paletteHits(int count, {String prefix = 'test.tool_'}) => [
  for (var i = 0; i < count; i++)
    paletteHit('$prefix$i', 'Tool $i', description: 'test tool $i'),
];

Override paletteBrowseOverride(List<PaletteHit> hits) =>
    paletteSearcherProvider.overrideWith(
      (ref) =>
          (String query) => query.trim().isEmpty ? hits : const <PaletteHit>[],
    );

/// Base overrides every popup test installs so the FRB dylib never
/// gets loaded. Overriding [statusSnapshotProvider] directly (rather
/// than just the reader seam) keeps the 2-second `Timer.periodic`
/// from leaking past `tester.pumpWidget` and tripping the post-test
/// pending-timer assertion.
///
/// Tests that care about the empty-query catalogue should override
/// [paletteSearcherProvider] with browse hits; typed-query tests reuse the
/// same seam.
List<Override> basePopupOverrides({String boardKey = 'dev'}) => [
  ...pegboardSelectionOverrides(boardKey: boardKey),
  statusSnapshotProvider.overrideWith(
    () => FixedStatusNotifier(statusWithVersion('0.0.0')),
  ),
  fakeKeyboardResolverOverride,
  // Activation now consults `pinActivationFor` inside the popup;
  // default every hit to the form-needed (OpenModal) verdict so the
  // legacy full-transition expectations keep exercising that path
  // without loading the dylib. Inline-run tests override this with a
  // DispatchImmediate verdict.
  pinActivationProvider.overrideWith(
    (ref) =>
        ({required toolId, required argsJson}) =>
            PinActivationDto.openModal(toolId: toolId.value),
  ),
];

Widget popupPageHarness({
  List<PaletteHit> hits = const <PaletteHit>[],
  List<BoardDto> boards = const <BoardDto>[],
  List<Override>? extraOverrides,
}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      paletteSearcherProvider.overrideWith(
        (ref) =>
            (String _) => hits,
      ),
      // PopupPage.initState restores the board selection (R8), which
      // reads `boardsProvider`. Seed the loader so the restore stays
      // dylib-free even when a test doesn't care about boards.
      boardsLoaderProvider.overrideWith(
        (ref) =>
            () => boards,
      ),
      ...basePopupOverrides(),
      // Keep tool lookup empty by default. Popup browse/search hits come from
      // the palette search seam, not the unfiltered tool list.
      toolsLoaderProvider.overrideWith(
        (ref) =>
            () => const <ToolDto>[],
      ),
      ...?extraOverrides,
    ],
    child: const MaterialApp(home: PopupPage()),
  );
}

Finder popupRootFocusFinder() {
  return find.byWidgetPredicate(
    (widget) => widget is Focus && widget.focusNode?.debugLabel == 'PopupPage',
  );
}
