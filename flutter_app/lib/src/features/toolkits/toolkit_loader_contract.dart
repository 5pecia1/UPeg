library;

typedef ToolkitLoadProgress = void Function(ToolkitLoadUpdate update);

final class ToolkitLoadUpdate {
  const ToolkitLoadUpdate({
    required this.state,
    required this.loaded,
    required this.total,
  });

  final String state;
  final int loaded;
  final int total;
}

final class ToolkitCatalogDescriptor {
  const ToolkitCatalogDescriptor({
    required this.digest,
    required this.toolkitByToolId,
  });

  final String digest;
  final Map<String, String> toolkitByToolId;

  factory ToolkitCatalogDescriptor.fromJson(Map<String, Object?> json) {
    final digest = json['catalog_digest'];
    final rawToolkits = json['toolkits'];
    if (digest is! String || rawToolkits is! List) {
      throw const FormatException('Invalid toolkit catalog.');
    }
    final byToolId = <String, String>{};
    for (final rawToolkit in rawToolkits) {
      if (rawToolkit is! Map) continue;
      final toolkitId = rawToolkit['id'];
      final tools = rawToolkit['tools'];
      if (toolkitId is! String || tools is! List || rawToolkit['web'] == null) {
        continue;
      }
      for (final rawTool in tools) {
        if (rawTool is Map && rawTool['id'] is String) {
          byToolId[rawTool['id'] as String] = toolkitId;
        }
      }
    }
    return ToolkitCatalogDescriptor(
      digest: digest,
      toolkitByToolId: Map<String, String>.unmodifiable(byToolId),
    );
  }
}

abstract interface class ToolkitWorkerClient {
  Future<ToolkitCatalogDescriptor> sync({
    required String catalogUrl,
    required String abiDigest,
  });

  Future<String> dispatch({
    required ToolkitCatalogDescriptor catalog,
    required String toolkitId,
    required String toolId,
    required String argsJson,
    ToolkitLoadProgress? onProgress,
  });
}
