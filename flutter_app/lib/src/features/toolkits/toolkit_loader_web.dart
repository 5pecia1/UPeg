library;

import 'dart:js_interop';

import 'package:upeg/src/features/toolkits/toolkit_loader_contract.dart';

final class BrowserToolkitWorkerClient implements ToolkitWorkerClient {
  const BrowserToolkitWorkerClient();

  @override
  Future<ToolkitCatalogDescriptor> sync({
    required String catalogUrl,
    required String abiDigest,
  }) async {
    final value = await _sync(
      <String, Object?>{
            'catalogUrl': catalogUrl,
            'abiDigest': abiDigest,
          }.jsify()!
          as JSObject,
    ).toDart;
    final response = _objectFromJs(value);
    final catalog = response['catalog'];
    if (catalog is! Map) {
      throw const FormatException('Invalid toolkit catalog.');
    }
    return ToolkitCatalogDescriptor.fromJson(
      Map<String, Object?>.from(catalog),
    );
  }

  @override
  Future<String> dispatch({
    required ToolkitCatalogDescriptor catalog,
    required String toolkitId,
    required String toolId,
    required String argsJson,
    ToolkitLoadProgress? onProgress,
  }) async {
    final value = await _dispatch(
      <String, Object?>{
            'catalogDigest': catalog.digest,
            'toolkitId': toolkitId,
            'toolId': toolId,
            'argsJson': argsJson,
          }.jsify()!
          as JSObject,
      onProgress == null
          ? null
          : ((JSAny? event) {
              final decoded = _objectFromJs(event);
              onProgress(
                ToolkitLoadUpdate(
                  state: decoded['state'] as String? ?? 'downloading',
                  loaded: decoded['loaded'] as int? ?? 0,
                  total: decoded['total'] as int? ?? 0,
                ),
              );
            }).toJS,
    ).toDart;
    final response = _objectFromJs(value);
    final result = response['resultJson'];
    if (result is! String) {
      throw const FormatException('Invalid toolkit result.');
    }
    return result;
  }

  Map<String, Object?> _objectFromJs(JSAny? value) {
    final decoded = value.dartify();
    if (decoded is! Map) {
      throw const FormatException('Invalid toolkit worker response.');
    }
    return Map<String, Object?>.from(decoded);
  }
}

@JS('UpegToolkitLoader.sync')
external JSPromise<JSAny?> _sync(JSObject options);

@JS('UpegToolkitLoader.dispatch')
external JSPromise<JSAny?> _dispatch(JSObject options, JSFunction? onProgress);
