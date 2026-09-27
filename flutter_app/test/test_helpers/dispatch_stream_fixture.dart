/// Test doubles for the streaming dispatch seam
/// (`lib/src/state/dispatch_stream_provider.dart`).
///
/// Most widget tests only care about the final result, not the live
/// output in front of it, so [stubDispatchStream] lets them keep writing
/// a one-shot answer while the widget under test consumes a stream.
/// Tests that *are* about the tail build their own stream and feed
/// [DispatchStreamEventDto.chunk] events directly.
library;

import 'package:flutter_riverpod/misc.dart' show Override;

import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/dispatch_stream.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/dispatch_stream_provider.dart';
import 'package:upeg/src/state/live_outcome_provider.dart'
    show LiveDispatchFn, liveDispatchToolFnProvider;
import 'package:upeg/src/state/pin_provider.dart' show PinKey;
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

/// One dispatch, answered without any progress: no chunks, one `Done`.
///
/// Takes a thunk rather than a future so the call happens *inside* the
/// generator, after a listener has attached. Building the future eagerly
/// would leave a failing stub's error unhandled for a microtask, which
/// the test zone reports as an escaped exception instead of letting the
/// widget's own error handling see it.
Stream<DispatchStreamEventDto> dispatchStreamOfResult(
  Future<CanonicalToolResult> Function() dispatch,
) async* {
  yield DispatchStreamEventDto.done(result: await dispatch());
}

/// The dispatch a stub answers, with the arguments the widget passed.
typedef StubDispatch =
    Future<CanonicalToolResult> Function({
      required ToolId toolId,
      required ToolArgs args,
      required bool approve,
    });

/// Adapt a one-shot [StubDispatch] into a [DispatchStreamFn].
DispatchStreamFn stubDispatchStream(StubDispatch onDispatch) =>
    ({
      PinKey? pinKey,
      required ToolId toolId,
      required ToolArgs args,
      required bool approve,
      required DispatchRunId runId,
    }) => dispatchStreamOfResult(
      () => onDispatch(toolId: toolId, args: args, approve: approve),
    );

/// Adapt a one-shot [LiveDispatchFn] into the streaming seam.
///
/// Board/inline harnesses were written when the inline pin dispatched
/// through `liveDispatchToolFnProvider`; this keeps their `dispatch`
/// argument meaning the same thing now that the pin streams.
DispatchStreamFn dispatchStreamFromLive(LiveDispatchFn dispatch) =>
    ({
      PinKey? pinKey,
      required ToolId toolId,
      required ToolArgs args,
      required bool approve,
      required DispatchRunId runId,
    }) => dispatchStreamOfResult(
      () => dispatch(pinKey: pinKey, toolId: toolId, args: args),
    );

/// The cancel every stubbed run answers with when a test does not care.
///
/// `false` is the truthful answer for this fixture: [dispatchStreamOfResult]
/// has no live run to stop — it yields one `Done` and finishes — so nothing
/// was found to cancel. A test that is *about* cancellation overrides
/// [cancelDispatchFnProvider] itself with a recorder.
bool _noLiveRunToCancel({required DispatchRunId runId}) => false;

/// All three dispatch seams answered by one fake.
///
/// A board harness drives two consumers: the inline pin body (streaming)
/// and live polling / pin activation (one-shot). Spreading this keeps a
/// test's "what does dispatch answer" in exactly one place.
///
/// [cancelDispatchFnProvider] is in the list even though this fixture never
/// produces a cancellable run, because leaving it real is not neutral: the
/// widgets that stream (`generic_inline_pin_body.dart`,
/// `expanded_modal_page.dart`) read it the moment a user presses Cancel, and
/// the real provider calls straight into the FRB bridge — which in a widget
/// test means reaching for a Rust library that was never loaded. Pass
/// [cancel] to observe the call instead.
List<Override> dispatchOverrides(
  LiveDispatchFn dispatch, {
  CancelDispatchFn cancel = _noLiveRunToCancel,
}) => <Override>[
  liveDispatchToolFnProvider.overrideWithValue(dispatch),
  dispatchStreamFnProvider.overrideWithValue(dispatchStreamFromLive(dispatch)),
  cancelDispatchFnProvider.overrideWithValue(cancel),
];
