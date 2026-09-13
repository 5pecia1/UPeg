import 'dart:async';
import 'dart:js_interop';
import 'dart:typed_data';

import 'package:web/web.dart' as web;

const Duration _downloadUrlLifetime = Duration(seconds: 30);

/// Trigger a browser-managed download without requiring a filesystem path.
void downloadBytes(Uint8List bytes, String name, String mime) {
  final blob = web.Blob([bytes.toJS].toJS, web.BlobPropertyBag(type: mime));
  final url = web.URL.createObjectURL(blob);
  final anchor = web.HTMLAnchorElement()
    ..href = url
    ..download = name;
  try {
    web.document.body?.append(anchor);
    anchor.click();
  } finally {
    anchor.remove();
    Timer(_downloadUrlLifetime, () => web.URL.revokeObjectURL(url));
  }
}
