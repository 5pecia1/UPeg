/// FRB dispatch-capability seam — Riverpod-overridable so widget tests can
/// swap in a fake verdict without linking the native dylib or running wasm.
///
/// `dispatchCapabilityFor` is a sync FRB call safe to invoke from `build`
/// for pin rendering. The honest "미지원" state only ever occurs on the
/// wasm/PWA runtime, so consumers gate their consultation on
/// [isWasmRuntimeProvider]; tests override it to `true` to exercise the
/// path on the native test host.
library;

import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/rust/api/capability.dart';

/// `dispatchCapabilityFor(toolId) -> DispatchCapabilityDto` function shape.
typedef ToolCapabilityFn = DispatchCapabilityDto Function(String toolId);

DispatchCapabilityDto _capabilityBridge(String toolId) =>
    dispatchCapabilityFor(toolId: toolId);

/// FRB capability adapter — swap in tests via [Provider.overrideWith].
final toolCapabilityFnProvider = Provider<ToolCapabilityFn>(
  (ref) => _capabilityBridge,
);

/// Whether the in-process runtime is the wasm32 (Flutter web / PWA) build.
///
/// Native surfaces link the full runtime and support every declared tool,
/// so the honest-unsupported UI is only consulted when this is `true`.
/// Defaults to `kIsWeb`; tests override it to force the wasm branch.
final isWasmRuntimeProvider = Provider<bool>((ref) => kIsWeb);
