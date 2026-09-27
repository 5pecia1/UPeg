/// Streaming dispatch seam for the desktop surface.
///
/// One subscription per run: [DispatchStreamEventDto_Chunk] events carry
/// the tool's output while it is still running, and a single
/// [DispatchStreamEventDto_Done] carries the canonical result last. The
/// Rust side (`upeg-frb/src/api/dispatch_stream.rs`) installs the
/// runtime's progress sink + cancellation token around the same dispatch
/// body the one-shot entry points use.
///
/// SoC: this file owns only the seam (which function, which board
/// context, which run id). Folding chunks into renderable lines is
/// `widgets/expanded_modal/live_output_tail.dart`; deciding whether a
/// run may start at all is `widgets/approval_confirm_dialog.dart`.
library;

import 'dart:math';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/dispatch_stream.dart';
import 'package:upeg/src/state/app_state.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

/// Identity of one streamed dispatch.
///
/// Minted on the Dart side because the dispatch entry point returns a
/// stream, not a value: there is no return slot to hand an id back
/// through, and an id invented in Rust could only reach Dart as a stream
/// event — i.e. after the moment a user may already want to cancel.
@immutable
final class DispatchRunId {
  const DispatchRunId._(this.value);

  final String value;

  @override
  String toString() => value;

  @override
  bool operator ==(Object other) =>
      identical(this, other) || other is DispatchRunId && other.value == value;

  @override
  int get hashCode => value.hashCode;
}

/// Prefix that marks a run id as minted by this surface. Named so the
/// wire format has no inline literal; the Rust registry treats the id as
/// opaque.
const String dispatchRunIdPrefix = 'desktop-run-';

int _dispatchRunSequence = 0;
final _dispatchSessionId =
    '${DateTime.now().microsecondsSinceEpoch}-'
    '${Random.secure().nextInt(1 << 32).toRadixString(16)}';

/// Session identity plus a monotonic sequence keeps persisted reports distinct
/// across app restarts while preserving an ordered local run number.
DispatchRunId nextDispatchRunId() => DispatchRunId._(
  '$dispatchRunIdPrefix$_dispatchSessionId-${++_dispatchRunSequence}',
);

/// Start a streamed dispatch.
///
/// `approve` is the typed approval flag: Rust is the only writer of the
/// reserved `approve` call argument, so this bool is the only way a
/// desktop run lifts a Chain's approval barrier.
typedef DispatchStreamFn =
    Stream<DispatchStreamEventDto> Function({
      PinKey? pinKey,
      required ToolId toolId,
      required ToolArgs args,
      required bool approve,
      required DispatchRunId runId,
    });

/// Ask a live run to stop. Returns whether one was found — `false` is
/// the honest answer for a Cancel pressed a moment too late.
typedef CancelDispatchFn = bool Function({required DispatchRunId runId});

/// Streaming dispatch bound to the currently-selected board, mirroring
/// the one-shot bridges: a run entered from a board carries that board's
/// execution context (pin args-preset merge + `_upeg.board`).
final dispatchStreamFnProvider = Provider<DispatchStreamFn>(
  (ref) =>
      ({
        PinKey? pinKey,
        required ToolId toolId,
        required ToolArgs args,
        required bool approve,
        required DispatchRunId runId,
      }) => dispatchToolStreamed(
        toolId: toolId.value,
        argsJson: args.encodeJson(),
        boardKey: pinKey?.$1.value ?? ref.read(currentBoardKeyProvider)?.value,
        pinId: pinKey?.$2.value,
        approve: approve,
        runId: runId.value,
      ),
);

/// Cancel bridge. Separate provider from [dispatchStreamFnProvider] so a
/// widget test can stub one without inventing the other.
final cancelDispatchFnProvider = Provider<CancelDispatchFn>(
  (ref) =>
      ({required DispatchRunId runId}) => cancelDispatch(runId: runId.value),
);
