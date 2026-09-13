/// Phase 9 integration test — FRB dispatch round-trip.
///
/// Pure FRB test (no widget tree): asserts that `dispatchTool` over the
/// bridge runs the num.hex_to_decimal built-in and surfaces its
/// stdout. This is the load-bearing assertion for "Flutter UI can
/// actually invoke a tool via the new Rust API" — if this passes,
/// every other dispatch in the app uses the same code path.
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:upeg/src/rust/canonical_tool_result_view.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/frb_generated.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    await RustLib.init();
  });

  test('FRB_dispatch_tool은_convert_hex_to_dec를_실행한다', () async {
    final outcome = dispatchTool(
      toolId: 'num.hex_to_decimal',
      argsJson: '{"input":"0xff"}',
      approve: false,
    );
    expect(outcome.ok, isTrue);
    expect(outcome.primaryOutputText.trim(), '255');
  });
}
