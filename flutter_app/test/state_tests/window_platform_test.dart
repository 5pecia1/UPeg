import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/platform/window.dart';
import 'package:upeg/src/state/window_mode_provider.dart';
import 'package:window_manager/window_manager.dart';

class _RecordingWindowModeDriver implements WindowModeDriver {
  _RecordingWindowModeDriver({this.throwOnShadow = false});

  final bool throwOnShadow;
  final calls = <String>[];

  @override
  Future<void> focus() async {
    calls.add('focus');
  }

  @override
  Future<void> setHasShadow(bool hasShadow) async {
    calls.add('setHasShadow:$hasShadow');
    if (throwOnShadow) {
      throw MissingPluginException('setHasShadow');
    }
  }

  @override
  Future<void> setSize(Size size) async {
    calls.add('setSize:${size.width}x${size.height}');
  }

  @override
  Future<void> setTitleBarStyle(TitleBarStyle titleBarStyle) async {
    calls.add('setTitleBarStyle:$titleBarStyle');
  }

  @override
  Future<void> show() async {
    calls.add('show');
  }
}

void main() {
  group('applyWindowMode', () {
    test(
      'keeps_calling_show_and_focus_even_when_setHasShadow_is_unsupported',
      () async {
        final driver = _RecordingWindowModeDriver(throwOnShadow: true);

        await applyWindowModeWith(WindowMode.full, driver);

        expect(driver.calls, [
          'setSize:1280.0x800.0',
          'setHasShadow:true',
          'setTitleBarStyle:TitleBarStyle.normal',
          'show',
          'focus',
        ]);
      },
    );
  });
}
