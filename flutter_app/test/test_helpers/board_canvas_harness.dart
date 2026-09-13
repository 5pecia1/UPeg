/// Shared fixtures for the `board_canvas_*_test.dart` family: the
/// provider scopes that let `debugBoardCanvasGrid` render without the
/// Rust dylib, plus the tool/result fixtures those boards are built from.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/rust/api/pegboard.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/capability_provider.dart';
import 'package:upeg/src/state/live_outcome_provider.dart' show LiveDispatchFn;
import 'package:upeg/src/state/push_preview_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/board_canvas.dart';
import 'package:upeg/src/widgets/controlled_embed/runner.dart';
import 'package:upeg/src/widgets/embed_iframe_factory.dart';
import 'package:upeg/src/widgets/pin.dart';

import 'i18n_test_catalog.dart';
import 'tool_fixture.dart';
import 'dispatch_stream_fixture.dart';

/// Wraps the pure-grid renderer with a `ProviderScope` that injects an
/// empty tools list — so the `toolByIdProvider` lookup resolves to `null`
/// without needing the Rust dylib loaded.
Widget boardCanvasHarness({
  required LayoutSnapshotDto snapshot,
  PinTapCallback? onPinTap,
  MovePinCallback? onMovePin,
}) {
  return ProviderScope(
    overrides: [
      ...i18nTestOverrides,
      toolsLoaderProvider.overrideWith(
        (ref) =>
            () => const <ToolDto>[],
      ),
      pushPreviewLoaderProvider.overrideWithValue(
        (boardKey, toolId, anchorX, anchorY) => const <PlacementDto>[],
      ),
    ],
    child: MaterialApp(
      home: Scaffold(
        body: debugBoardCanvasGrid(
          snapshot: snapshot,
          onPinTap: onPinTap ?? (_) {},
          onMovePin: onMovePin,
        ),
      ),
    ),
  );
}

/// Test helper that provides a no-op iframe factory for testing.
class NoopIframeFactory extends IframeFactory {
  const NoopIframeFactory();
  @override
  String register(String url, {void Function()? onLoad}) => 'noop-$url';
}

CanonicalToolResult controlledEmbedSuccessResult(Map<String, String> outputs) {
  final entries = [
    for (final entry in outputs.entries)
      CanonicalOutputEntry(
        id: entry.key,
        kind: kControlledEmbedStringOutputKind,
        value: CanonicalOutputValue.string(value: entry.value),
      ),
  ];
  return CanonicalToolResult(
    ok: true,
    primaryOutputId: entries.isEmpty ? null : entries.first.id,
    outputs: entries,
  );
}

ToolDto genericInputTool({
  required String id,
  PinKindDto pinKind = PinKindDto.inline,
  PegboardUnitsDto pegboardUnits = PegboardUnitsDto.u1,
  SourceDto source = const SourceDto.manual(),
}) => fixtureToolDto(
  id: id,
  label: id,
  pinKind: pinKind,
  pegboardUnits: pegboardUnits,
  source: source,
  inputFields: const <InputFieldDto>[
    InputFieldDto(
      key: 'value',
      label: 'Value',
      fieldType: InputFieldType.text(),
      required_: true,
    ),
  ],
);

Widget genericInlineHarness({
  required List<ToolDto> tools,
  required LayoutSnapshotDto snapshot,
  required LiveDispatchFn dispatch,
  PinTapCallback? onOpenModal,
}) {
  return ProviderScope(
    overrides: [
      // The merged Pin renders its accessibility label/value through `t()`,
      // so every board harness must swap the FRB i18n seam for the test
      // catalog or Pin.build throws "flutter_rust_bridge has not been
      // initialized".
      ...i18nTestOverrides,
      toolsLoaderProvider.overrideWith(
        (ref) =>
            () => tools,
      ),
      ...dispatchOverrides(dispatch),
      // This fixture verifies the native inline contract even when the test
      // runner itself is Chrome. Wasm capability cases override both seams
      // explicitly in their dedicated fixtures below.
      isWasmRuntimeProvider.overrideWithValue(false),
    ],
    child: MaterialApp(
      theme: UpegTheme.darkTheme(),
      home: Scaffold(
        body: SizedBox(
          width: 1200,
          height: 600,
          child: debugBoardCanvasGrid(
            snapshot: snapshot,
            onPinTap: (_) {},
            onOpenModal: onOpenModal,
          ),
        ),
      ),
    ),
  );
}
