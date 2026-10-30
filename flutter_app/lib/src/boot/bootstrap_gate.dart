/// Flutter-only first frame while desktop plugins and the Rust bridge start.
library;

import 'package:flutter/material.dart';
import 'package:upeg/src/brand/startup_logo.dart';
import 'package:upeg/src/pages/splash_page.dart';
import 'package:upeg/src/state/accent.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

sealed class BootstrapOutcome {
  const BootstrapOutcome();
}

final class BootstrapReady extends BootstrapOutcome {
  const BootstrapReady();
}

final class BootstrapFailed extends BootstrapOutcome {
  const BootstrapFailed(this.error);

  final Object error;
}

class BootstrapGate extends StatelessWidget {
  const BootstrapGate({
    required this.outcome,
    required this.readyChild,
    super.key,
  });

  final Future<BootstrapOutcome> outcome;
  final Widget readyChild;

  @override
  Widget build(BuildContext context) {
    return StartupLogoSession(
      child: FutureBuilder<BootstrapOutcome>(
        future: outcome,
        builder: (context, snapshot) {
          if (snapshot.connectionState != ConnectionState.done) {
            return _shell(const SplashPage());
          }
          return switch (snapshot.data) {
            BootstrapReady() => readyChild,
            BootstrapFailed(:final error) => _shell(
              Scaffold(body: Center(child: Text('boot failed: $error'))),
            ),
            null => _shell(
              const Scaffold(body: Center(child: Text('boot failed'))),
            ),
          };
        },
      ),
    );
  }

  Widget _shell(Widget page) => MaterialApp(
    title: 'upeg',
    theme: UpegTheme.forAccent(kDefaultAccent, brightness: Brightness.light),
    debugShowCheckedModeBanner: false,
    home: page,
  );
}
