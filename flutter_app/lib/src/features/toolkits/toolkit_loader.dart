/// Browser toolkit-worker boundary.
///
/// The worker fetches and verifies executable pack artifacts.  The Dart UI
/// only sees a catalog identity and the canonical result JSON returned by a
/// guest; it never accepts executable bytes.
library;

export 'toolkit_loader_stub.dart'
    if (dart.library.js_interop) 'toolkit_loader_web.dart';
