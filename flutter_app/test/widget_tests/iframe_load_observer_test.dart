import 'package:fake_async/fake_async.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/widgets/iframe_load_observer.dart';

void main() {
  test('onload이_타임아웃_전에_도착하면_loaded로_전이한다', () {
    fakeAsync((async) {
      final observer = IframeLoadObserver(timeout: const Duration(seconds: 5));
      final states = <IframeLoadState>[];
      observer.changes.listen(states.add);

      async.elapse(const Duration(seconds: 2));
      observer.notifyLoaded();
      async.elapse(const Duration(seconds: 5));

      expect(states, [isA<IframeLoaded>()]);
      observer.dispose();
    });
  });

  test('타임아웃까지_onload이_안오면_timeout으로_전이한다', () {
    fakeAsync((async) {
      final observer = IframeLoadObserver(timeout: const Duration(seconds: 5));
      final states = <IframeLoadState>[];
      observer.changes.listen(states.add);

      async.elapse(const Duration(seconds: 6));
      expect(states, [isA<IframeTimeout>()]);
      observer.dispose();
    });
  });

  test('이미_settle된_observer는_추가_notifyLoaded를_무시한다', () {
    fakeAsync((async) {
      final observer = IframeLoadObserver(timeout: const Duration(seconds: 5));
      final states = <IframeLoadState>[];
      observer.changes.listen(states.add);

      observer.notifyLoaded();
      observer.notifyLoaded();
      observer.notifyLoaded();
      async.elapse(const Duration(seconds: 10));

      expect(states, [isA<IframeLoaded>()]);
      observer.dispose();
    });
  });

  test('dispose_후_late_notifyLoaded는_no_op이다', () {
    fakeAsync((async) {
      final observer = IframeLoadObserver(timeout: const Duration(seconds: 5));
      final states = <IframeLoadState>[];
      observer.changes.listen(states.add);

      // Dispose terminal: timer is cancelled, stream is closed, _closed = true
      observer.dispose();

      // Late callback must not crash or emit further events
      observer.notifyLoaded();

      // No state transition after close — still loading (the initial state)
      expect(states, isEmpty);
      expect(observer.state, isA<IframeLoading>());
    });
  });
}
