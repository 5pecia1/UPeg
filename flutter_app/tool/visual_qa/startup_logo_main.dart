/// Starts with the production splash as the first-ever Flutter root.
/// This deliberately uses no test binding or Rust initialization.
library;

import 'package:flutter/material.dart';
import 'package:upeg/src/brand/startup_logo.dart';
import 'package:upeg/src/pages/splash_page.dart';
import 'package:upeg/src/state/accent.dart';
import 'package:upeg/src/theme/upeg_theme.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(
    StartupLogoSession(
      child: MaterialApp(
        debugShowCheckedModeBanner: false,
        theme: UpegTheme.forAccent(
          kDefaultAccent,
          brightness: Brightness.light,
        ),
        home: const SplashPage(),
      ),
    ),
  );
}
