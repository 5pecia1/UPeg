/// Iframe load-state observer.
///
/// Wraps the "did the iframe call onload within N seconds?" decision in
/// a single typed unit so widget code only ever reads a sealed state.
/// SoC: the observer owns nothing UI; the rendering widget listens to
/// `changes` and rebuilds.
library;

import 'dart:async';

/// Sealed iframe lifecycle state used by `_IframeWithTimeout`.
sealed class IframeLoadState {
  const IframeLoadState();
  const factory IframeLoadState.loading() = IframeLoading;
  const factory IframeLoadState.loaded() = IframeLoaded;
  const factory IframeLoadState.timeout() = IframeTimeout;
}

final class IframeLoading extends IframeLoadState {
  const IframeLoading();
}

final class IframeLoaded extends IframeLoadState {
  const IframeLoaded();
}

final class IframeTimeout extends IframeLoadState {
  const IframeTimeout();
}

/// Single iframe's load lifecycle. Construct one per `<iframe>` mount;
/// call [notifyLoaded] from the `onload` handler. The first transition
/// (loaded or timeout) is broadcast on [changes]; subsequent calls are
/// ignored.
class IframeLoadObserver {
  IframeLoadObserver({required Duration timeout}) {
    _timer = Timer(timeout, _onTimeout);
  }

  Timer? _timer;
  final StreamController<IframeLoadState> _controller =
      StreamController<IframeLoadState>.broadcast();
  IframeLoadState _state = const IframeLoadState.loading();
  bool _settled = false;
  bool _closed = false;

  Stream<IframeLoadState> get changes => _controller.stream;
  IframeLoadState get state => _state;

  void notifyLoaded() {
    if (_closed || _settled) return;
    _settled = true;
    _timer?.cancel();
    _timer = null;
    _state = const IframeLoadState.loaded();
    _controller.add(_state);
  }

  void _onTimeout() {
    if (_settled) return;
    _settled = true;
    _state = const IframeLoadState.timeout();
    _controller.add(_state);
  }

  void dispose() {
    _closed = true;
    _timer?.cancel();
    _controller.close();
  }
}
