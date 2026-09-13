library;

import 'package:flutter/widgets.dart';

typedef BootLog = void Function(String message);

void defaultBootLog(String message) => debugPrint(message);
