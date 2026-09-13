/// FRB seam for `pin_activation_for`.
///
/// Production reads the dylib; widget tests override [pinActivationProvider]
/// with a closure that returns a synthetic [PinActivationDto]. Keeping the
/// shim behind a typedef stays consistent with the other FRB seams
/// (`dispatchStreamFnProvider`, `keyboardCommandResolverProvider`).
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/pin_activation.dart';

typedef PinActivationFn =
    PinActivationDto Function({
      required ToolId toolId,
      required String argsJson,
    });

final pinActivationProvider = Provider<PinActivationFn>(
  (ref) =>
      ({required toolId, required argsJson}) =>
          pinActivationFor(toolId: toolId.value, argsJson: argsJson),
);
