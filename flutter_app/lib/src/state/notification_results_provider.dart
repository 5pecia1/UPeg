/// Unread HTTP tool outcomes, polled from the shared result store.
library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:upeg/src/rust/api/notification_results.dart';

final unreadNotificationResultsProvider = StreamProvider<bool>((ref) {
  Timer? timer;
  var stopped = false;
  var running = false;
  void stop() {
    stopped = true;
    timer?.cancel();
  }

  ref.onDispose(stop);
  late final StreamController<bool> controller;
  Future<void> poll() async {
    if (running || stopped) return;
    running = true;
    try {
      final unread = await hasUnreadNotificationResults();
      if (!stopped) controller.add(unread);
    } on Object catch (error) {
      debugPrint('upeg: cannot read notification results: $error');
    } finally {
      running = false;
    }
  }

  controller = StreamController<bool>(
    onListen: () {
      unawaited(poll());
      timer = Timer.periodic(
        const Duration(seconds: 2),
        (_) => unawaited(poll()),
      );
    },
    onCancel: stop,
  );
  return controller.stream;
});
