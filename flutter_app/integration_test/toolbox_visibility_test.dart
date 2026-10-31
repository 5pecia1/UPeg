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
import 'package:upeg/src/rust/api/tools/input_field.dart';
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
    expect(ids, contains('media.pdf_compress'));
    expect(ids, contains('id.uuid_v7'));
  });

  test('pdf_compress_exposes_its_pwa_input_and_output_contract', () {
    final tool = listTools().singleWhere((t) => t.id == 'media.pdf_compress');
    final fields = {for (final field in tool.inputFields) field.key: field};

    final input = fields['input']!;
    expect(input.required_, isTrue);
    expect(input.fieldType, isA<InputFieldType_File>());
    final policy = (input.fieldType as InputFieldType_File).policy;
    expect(policy.extensions, ['pdf']);
    expect(policy.maxCount, 1);
    expect(policy.maxFileBytes, BigInt.from(50 * 1024 * 1024));

    final target = fields['target_bytes']!;
    expect(target.required_, isTrue);
    expect(target.fieldType, isA<InputFieldType_Integer>());
    expect(target.constraints?.number?.min, 1);
    expect(target.constraints?.number?.max, 67108864);

    final mode = fields['mode']!;
    expect(mode.required_, isFalse);
    expect(mode.fieldType, isA<InputFieldType_Select>());
    final options = (mode.fieldType as InputFieldType_Select).options;
    expect(options.map((option) => option.value), ['balanced', 'high_quality']);

    expect(
      tool.outputFields.map(
        (field) => (field.key, field.fieldType.runtimeType),
      ),
      [('result', OutputFieldType_File), ('report', OutputFieldType_Json)],
    );
  });

  test('toolbox_filters_by_toolkit', () async {
    final convert = listTools(toolkit: 'convert');
    expect(convert.every((t) => t.toolkit == 'convert'), isTrue);
    expect(convert, isNotEmpty);
  });
}
