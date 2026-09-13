library;

String normalizeFileExtension(String extension) {
  final normalized = extension.trim().toLowerCase();
  if (normalized.startsWith('*.')) return normalized.substring(2);
  if (normalized.startsWith('.')) return normalized.substring(1);
  return normalized;
}

Set<String> normalizeFileExtensions(Iterable<String> extensions) =>
    extensions.map(normalizeFileExtension).toSet();

bool hasAllowedFileExtension(String name, Set<String> extensions) {
  final normalizedName = name.toLowerCase();
  return extensions.any((extension) => normalizedName.endsWith('.$extension'));
}

/// Best-effort MIME lookup from a file extension, mirroring the CLI's
/// `@path` MIME guess. Returns `null` for unknown extensions.
String? mimeForFileExtension(String? extension) {
  if (extension == null) return null;
  return switch (normalizeFileExtension(extension)) {
    'pptx' =>
      'application/vnd.openxmlformats-officedocument.presentationml.presentation',
    'docx' =>
      'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
    'xlsx' =>
      'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
    'pdf' => 'application/pdf',
    'zip' => 'application/zip',
    'png' => 'image/png',
    'jpg' || 'jpeg' => 'image/jpeg',
    'gif' => 'image/gif',
    'webp' => 'image/webp',
    'svg' => 'image/svg+xml',
    'txt' => 'text/plain',
    'json' => 'application/json',
    _ => null,
  };
}
