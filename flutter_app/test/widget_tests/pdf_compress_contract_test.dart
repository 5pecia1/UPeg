/// UI contract coverage for the generic PDF compression tool form and result.
/// These use fixture metadata and canonical outputs; they do not call the
/// Rust compressor or establish backend behavior.
library;

import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/identity.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/file_input_policy.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/widgets/dispatch_result_dialog.dart';
import 'package:upeg/src/widgets/expanded_modal/generic_form.dart';

import '../test_helpers/i18n_test_catalog.dart';
import '../test_helpers/tool_fixture.dart';

final _pdfInput = InputFieldDto(
  key: 'input',
  label: 'PDF file',
  description: 'PDF file (up to 50 MiB)',
  fieldType: InputFieldType.file(
    policy: FileInputPolicyDto(
      extensions: ['pdf'],
      maxCount: 1,
      maxFileBytes: BigInt.from(52428800),
      maxTotalBytes: BigInt.from(52428800),
    ),
  ),
  required_: true,
);

const _target = InputFieldDto(
  key: 'target_bytes',
  label: 'target_bytes',
  description: 'Desired PDF size in bytes',
  fieldType: InputFieldType.integer(),
  required_: true,
  constraints: FieldConstraintsDto(
    number: NumberConstraintsDto(min: 1, max: 67108864),
  ),
);

const _mode = InputFieldDto(
  key: 'mode',
  label: 'mode',
  fieldType: InputFieldType.select(
    options: [
      ChoiceOptionDto(value: 'balanced', label: 'Balanced'),
      ChoiceOptionDto(value: 'high_quality', label: 'High quality'),
    ],
  ),
  required_: false,
);

ToolDto _compressionTool() => fixtureToolDto(
  id: 'media.pdf_compress',
  toolkit: 'media',
  label: 'Compress PDF',
  inputFields: [_pdfInput, _target, _mode],
  outputFields: const [
    OutputFieldDto(
      key: 'result',
      label: 'PDF artifact',
      fieldType: OutputFieldType.file(),
    ),
    OutputFieldDto(
      key: 'report',
      label: 'Compression report',
      fieldType: OutputFieldType.json(),
    ),
  ],
);

Widget _formHarness(GenericFormController controller) => ProviderScope(
  overrides: i18nTestOverrides,
  child: MaterialApp(
    home: Scaffold(
      body: SingleChildScrollView(
        child: GenericFormWidget(
          tool: _compressionTool(),
          controller: controller,
        ),
      ),
    ),
  ),
);

final class _PdfCompressionRobot {
  _PdfCompressionRobot(this.tester);

  final WidgetTester tester;

  Future<void> pumpForm(GenericFormController controller) async {
    await tester.pumpWidget(_formHarness(controller));
  }

  Future<void> setTargetAndMode() async {
    await tester.enterText(
      find.byKey(const Key('field-target_bytes')),
      '1048576',
    );
    await tester.tap(find.byKey(const Key('field-mode')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('High quality').last);
    await tester.pumpAndSettle();
  }

  void expectPdfInputsPresent() {
    expect(find.byKey(const Key('field-input')), findsOneWidget);
    expect(find.byKey(const Key('field-target_bytes')), findsOneWidget);
    expect(find.byKey(const Key('field-mode')), findsOneWidget);
  }

  void expectModeAndTargetSerialized(GenericFormController controller) {
    expect(controller.snapshot()['target_bytes'], 1048576);
    expect(controller.snapshot()['mode'], 'high_quality');
  }

  Future<void> enterTarget(String value) async {
    await tester.enterText(find.byKey(const Key('field-target_bytes')), value);
    await tester.pump();
  }

  void expectTargetRangeError() {
    final decorator = tester.widget<InputDecorator>(
      find.descendant(
        of: find.byKey(const Key('field-target_bytes')),
        matching: find.byType(InputDecorator),
      ),
    );
    expect(decorator.decoration.errorText, 'min 1');
  }

  Future<void> pumpResult(CanonicalToolResult outcome) async {
    await tester.pumpWidget(
      ProviderScope(
        overrides: i18nTestOverrides,
        child: MaterialApp(
          home: Scaffold(
            body: DispatchResultDialog(
              toolId: ToolId.parse('media.pdf_compress'),
              outcome: outcome,
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
  }

  void expectArtifactAndReportVisible() {
    expect(
      find.text('scan-compressed.pdf · application/pdf · 4 bytes'),
      findsOneWidget,
    );
    expect(
      find.text(
        '{"status":"target_unmet","input_bytes":2000000,"output_bytes":1200000}',
      ),
      findsOneWidget,
    );
  }
}

void main() {
  testWidgets('the_pdf_compression_form_serializes_target_and_quality_mode', (
    tester,
  ) async {
    final controller = GenericFormController();
    final robot = _PdfCompressionRobot(tester);

    await robot.pumpForm(controller);
    robot.expectPdfInputsPresent();
    await robot.setTargetAndMode();

    robot.expectModeAndTargetSerialized(controller);
  });

  testWidgets('the_pdf_target_rejects_zero_and_negative_values', (
    tester,
  ) async {
    final robot = _PdfCompressionRobot(tester);
    await robot.pumpForm(GenericFormController());

    await robot.enterTarget('0');
    robot.expectTargetRangeError();
    await robot.enterTarget('-1');
    robot.expectTargetRangeError();
  });

  testWidgets('the_pdf_compression_result_displays_artifact_and_target_report', (
    tester,
  ) async {
    final outcome = CanonicalToolResult(
      ok: true,
      primaryOutputId: 'result',
      outputs: [
        CanonicalOutputEntry(
          id: 'result',
          label: 'PDF artifact',
          kind: 'file',
          value: CanonicalOutputValue.file(
            value: CanonicalFileValue(
              name: 'scan-compressed.pdf',
              isDir: false,
              mime: 'application/pdf',
              content: CanonicalFileContent.bytes(
                bytes: Uint8List.fromList([37, 80, 68, 70]),
              ),
            ),
          ),
        ),
        CanonicalOutputEntry(
          id: 'report',
          label: 'Compression report',
          kind: 'json',
          value: CanonicalOutputValue.json(
            value:
                '{"status":"target_unmet","input_bytes":2000000,"output_bytes":1200000}',
          ),
        ),
      ],
    );

    final robot = _PdfCompressionRobot(tester);
    await robot.pumpResult(outcome);
    robot.expectArtifactAndReportVisible();
  });
}
