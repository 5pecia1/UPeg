import 'package:fake_async/fake_async.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/widgets/iframe_load_observer.dart';

void main() {
  test('an_onload_arriving_before_the_timeout_transitions_to_loaded', () {
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

  test('no_onload_by_the_timeout_transitions_to_timeout', () {
    fakeAsync((async) {
      final observer = IframeLoadObserver(timeout: const Duration(seconds: 5));
      final states = <IframeLoadState>[];
      observer.changes.listen(states.add);

      async.elapse(const Duration(seconds: 6));
      expect(states, [isA<IframeTimeout>()]);
      observer.dispose();
    });
  });

  test('an_already_settled_observer_ignores_further_notifyloaded_calls', () {
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

  test('a_late_notifyloaded_after_dispose_is_a_no_op', () {
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
