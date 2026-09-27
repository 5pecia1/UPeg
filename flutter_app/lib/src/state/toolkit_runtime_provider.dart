/// Runtime state and dispatch bridge for downloadable browser toolkits.
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/features/host_attach/attach_canonical_result_decoder.dart';
import 'package:upeg/src/features/toolkits/toolkit_loader.dart';
import 'package:upeg/src/features/toolkits/toolkit_loader_contract.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/locale_provider.dart';
import 'package:upeg/src/state/tools_provider.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

const String kToolkitCatalogUrl = '/toolkits/catalog.json';
const String kToolkitAbiDigest = String.fromEnvironment(
  'UPEG_TOOLKIT_ABI_DIGEST',
);

sealed class ToolkitRuntimeState {
  const ToolkitRuntimeState();
}

final class ToolkitRuntimeIdle extends ToolkitRuntimeState {
  const ToolkitRuntimeIdle();
}

final class ToolkitRuntimeDownloading extends ToolkitRuntimeState {
  const ToolkitRuntimeDownloading(
    this.loaded,
    this.total, {
    this.retrying = false,
  });

  final int loaded;
  final int total;
  final bool retrying;
}

final class ToolkitRuntimeUnavailable extends ToolkitRuntimeState {
  const ToolkitRuntimeUnavailable(this.message, {required this.offline});

  final String message;
  final bool offline;
}

final class ToolkitRuntimeReady extends ToolkitRuntimeState {
  const ToolkitRuntimeReady();
}

final toolkitWorkerClientProvider = Provider<ToolkitWorkerClient>(
  (ref) => const BrowserToolkitWorkerClient(),
);

final toolkitRuntimeProvider =
    NotifierProvider<ToolkitRuntimeNotifier, Map<String, ToolkitRuntimeState>>(
      ToolkitRuntimeNotifier.new,
    );

class ToolkitRuntimeNotifier
    extends Notifier<Map<String, ToolkitRuntimeState>> {
  ToolkitCatalogDescriptor? _catalog;
  Future<ToolkitCatalogDescriptor>? _syncing;

  @override
  Map<String, ToolkitRuntimeState> build() =>
      const <String, ToolkitRuntimeState>{};

  Future<void> syncCatalog() async {
    if (!kIsWeb || _catalog != null) {
      return;
    }
    try {
      await _catalogOrSync();
    } on Object {
      // Catalog refresh is a warm-up only. An offline first launch still
      // renders embedded metadata and asks for a download only on use.
    }
  }

  Future<CanonicalToolResult> dispatch(
    ToolDto tool,
    ToolArgs args, {
    String? boardKey,
    String? pinId,
    required bool approve,
  }) async {
    if (!kIsWeb) {
      return dispatchToolAsync(
        toolId: tool.id,
        argsJson: args.encodeJson(),
        boardKey: boardKey,
        pinId: pinId,
        approve: approve,
      );
    }
    try {
      final catalog = await _catalogOrSync();
      final toolkitId = catalog.toolkitByToolId[tool.id];
      if (toolkitId == null) {
        return dispatchToolAsync(
          toolId: tool.id,
          argsJson: args.encodeJson(),
          boardKey: boardKey,
          pinId: pinId,
          approve: approve,
        );
      }
      final prepared = prepareWebToolkitDispatch(
        toolId: tool.id,
        argsJson: args.encodeJson(),
        boardKey: boardKey,
        pinId: pinId,
        approve: approve,
      );
      final preparationError = prepared.error;
      final effectiveArgsJson = prepared.effectiveArgsJson;
      if (preparationError != null || effectiveArgsJson == null) {
        return preparationError ??
            CanonicalToolResult(
              ok: false,
              outputs: <CanonicalOutputEntry>[],
              error: CanonicalToolError(
                code: 'toolkit_prepare_failed',
                message: _toolkitErrorMessage(false),
              ),
            );
      }
      final resultJson = await ref
          .read(toolkitWorkerClientProvider)
          .dispatch(
            catalog: catalog,
            toolkitId: toolkitId,
            toolId: tool.id,
            argsJson: effectiveArgsJson,
            onProgress: (update) => _recordProgress(toolkitId, update),
          );
      final decoded = jsonDecode(resultJson);
      if (decoded is! Map) {
        throw const FormatException('Invalid toolkit result.');
      }
      final result = decodeAttachCanonicalResult(
        Map<Object?, Object?>.from(decoded),
      );
      completeWebToolkitDispatch(
        toolId: tool.id,
        effectiveArgsJson: effectiveArgsJson,
        result: result,
      );
      state = <String, ToolkitRuntimeState>{
        ...state,
        toolkitId: const ToolkitRuntimeReady(),
      };
      return result;
    } on Object catch (error) {
      final text = '$error';
      final offline =
          text.contains('offline_download_required') ||
          text.contains('offline_catalog_required');
      state = <String, ToolkitRuntimeState>{
        ...state,
        tool.toolkit: ToolkitRuntimeUnavailable(
          _toolkitErrorMessage(offline),
          offline: offline,
        ),
      };
      return CanonicalToolResult(
        ok: false,
        outputs: const <CanonicalOutputEntry>[],
        error: CanonicalToolError(
          code: offline ? 'offline_download_required' : 'toolkit_load_failed',
          message: _toolkitErrorMessage(offline),
        ),
      );
    }
  }

  Future<ToolkitCatalogDescriptor> _catalogOrSync() {
    final loaded = _catalog;
    if (loaded != null) return Future<ToolkitCatalogDescriptor>.value(loaded);
    return _syncing ??= ref
        .read(toolkitWorkerClientProvider)
        .sync(catalogUrl: kToolkitCatalogUrl, abiDigest: kToolkitAbiDigest)
        .then((catalog) {
          _catalog = catalog;
          return catalog;
        })
        .whenComplete(() => _syncing = null);
  }

  void _recordProgress(String toolkitId, ToolkitLoadUpdate update) {
    state = <String, ToolkitRuntimeState>{
      ...state,
      toolkitId: ToolkitRuntimeDownloading(
        update.loaded,
        update.total,
        retrying: update.state == 'retrying',
      ),
    };
  }

  String _toolkitErrorMessage(bool offline) => ref.read(i18nTranslateOverride)(
    offline
        ? 'desktop.status.toolkit_offline'
        : 'desktop.status.toolkit_failed',
    ref.read(localeProvider),
  );
}

typedef ToolkitDispatch =
    Future<CanonicalToolResult> Function({
      required ToolId toolId,
      required ToolArgs args,
      String? boardKey,
      String? pinId,
      required bool approve,
    });

final toolkitDispatchProvider = Provider<ToolkitDispatch>((ref) {
  return ({
    required ToolId toolId,
    required ToolArgs args,
    String? boardKey,
    String? pinId,
    required bool approve,
  }) async {
    final tool = ref.read(toolByIdProvider(toolId));
    if (kIsWeb && tool != null) {
      return ref
          .read(toolkitRuntimeProvider.notifier)
          .dispatch(
            tool,
            args,
            boardKey: boardKey,
            pinId: pinId,
            approve: approve,
          );
    }
    return dispatchToolAsync(
      toolId: toolId.value,
      argsJson: args.encodeJson(),
      boardKey: boardKey,
      pinId: pinId,
      approve: approve,
    );
  };
});
