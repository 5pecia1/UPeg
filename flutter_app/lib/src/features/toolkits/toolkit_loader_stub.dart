library;

import 'package:upeg/src/features/toolkits/toolkit_loader_contract.dart';

final class BrowserToolkitWorkerClient implements ToolkitWorkerClient {
  const BrowserToolkitWorkerClient();

  @override
  Future<ToolkitCatalogDescriptor> sync({
    required String catalogUrl,
    required String abiDigest,
  }) => Future<ToolkitCatalogDescriptor>.error(
    UnsupportedError('Toolkit downloads are only available in the browser.'),
  );

  @override
  Future<String> dispatch({
    required ToolkitCatalogDescriptor catalog,
    required String toolkitId,
    required String toolId,
    required String argsJson,
    ToolkitLoadProgress? onProgress,
  }) => Future<String>.error(
    UnsupportedError('Toolkit downloads are only available in the browser.'),
  );
}
