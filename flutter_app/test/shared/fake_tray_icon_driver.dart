import 'package:upeg/src/platform/tray_icon.dart';

class FakeTrayIconDriver {
  final calls = <TrayIconSpec>[];
  Future<void> Function(TrayIconSpec)? beforeWrite;
  int inFlight = 0;
  int maxInFlight = 0;

  Future<void> setIcon(TrayIconSpec spec) async {
    inFlight++;
    if (inFlight > maxInFlight) maxInFlight = inFlight;
    calls.add(spec);
    try {
      await beforeWrite?.call(spec);
    } finally {
      inFlight--;
    }
  }
}
