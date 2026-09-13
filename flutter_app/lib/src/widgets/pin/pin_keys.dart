/// Widget keys the pin surface publishes as its test contract, plus the
/// move-handle tooltip copy.
///
/// Kept in one file so "what can a test reach inside a pin" is a single
/// list rather than a scatter across the pin's part files, and so the
/// board canvas can inject its own handle under the same key without
/// importing the whole pin widget tree.
library;

import 'package:flutter/foundation.dart';

const String pinMoveTooltip = 'move pin';
const Key pinMoveHandleKey = Key('pin-move-handle');

/// Test hook: the SE-corner resize drag handle injected by the board
/// canvas (mirrors [pinMoveHandleKey]).
const Key pinResizeHandleKey = Key('pin-resize-handle');
const Key pinChromeContainerKey = Key('pin-chrome-container');

/// Test hook: the focus ring painted around a keyboard-focused pin.
const Key pinFocusRingKey = Key('pin-focus-ring');

/// Test hook: the header's kind icon (only rendered when `pinKind` is
/// known — falls back to the plain dot otherwise).
const Key pinKindIconKey = Key('pin-kind-icon');

/// Test hook: the `live`-variant refresh glyph in the header.
const Key pinLiveIndicatorKey = Key('pin-live-indicator');

/// Test hook: the `frame`-variant body wrapper (Embed/ControlledEmbed).
const Key pinEmbedFrameKey = Key('pin-embed-frame');

/// Test hook: the `output`-variant body panel (Inline).
const Key pinOutputPanelKey = Key('pin-output-panel');

/// Test hook: the pin header container (asserts the `button`-variant
/// background tint).
const Key pinHeaderKey = Key('pin-header');

/// Test hook: the "last run · N ago" footer badge on a restored result.
const Key pinRestoredBadgeKey = Key('pin-restored-badge');
