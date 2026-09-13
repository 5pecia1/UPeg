/// Drag contract shared by the pin layer and the drop-target layer.
///
/// Kept apart from the widgets so the payload and the persistence
/// callback can be referenced without pulling in the whole canvas.
library;

import 'package:flutter/foundation.dart';
import 'package:upeg/src/identity.dart';

/// Fired by the empty-board card's call-to-action.
typedef OpenPaletteCallback = void Function();

/// Payload carried by `Draggable<PinDragPayload>`. Lives at module
/// scope so widget tests can hold a constant instance without
/// instantiating the surrounding `BoardCanvas`.
@immutable
class PinDragPayload {
  const PinDragPayload({required this.toolId, required this.boardKey});
  final ToolId toolId;
  final BoardKey boardKey;
}

/// Side-effect injected from outside so widget tests can intercept the
/// FRB persistence call without touching the Rust dylib.
typedef MovePinCallback =
    void Function({
      required BoardKey boardKey,
      required ToolId toolId,
      required int anchorX,
      required int anchorY,
    });
