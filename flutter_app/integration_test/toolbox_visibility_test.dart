/// Integration test — toolbox visibility.
///
/// Asserts that built-in tools surface in the runtime toolbox via
/// `listTools()` on the FRB bridge. If a `#[upeg::tool]` is ever
/// silently dropped from the inventory crate's link path, this test
/// fires.
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/frb_generated.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    await RustLib.init();
  });

  test('toolbox_exposes_core_built_in_tools', () async {
    final tools = listTools();
    final ids = tools.map((t) => t.id).toSet();

    expect(ids, contains('num.hex_to_decimal'));
    expect(ids, contains('media.image_to_pdf'));
    expect(ids, contains('media.pdf_to_images'));
    expect(ids, contains('id.uuid_v7'));
  });

  test('toolbox_filters_by_toolkit', () async {
    final convert = listTools(toolkit: 'convert');
    expect(convert.every((t) => t.toolkit == 'convert'), isTrue);
    expect(convert, isNotEmpty);
  });
}
