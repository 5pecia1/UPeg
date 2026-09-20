/// RED test for the typed `FormValue` sealed class powering the
/// generic form's controller. The form must encode each kind through a
/// concrete shape so dispatch sends typed JSON (number→num, boolean→bool,
/// multi-option→array) instead of stringly-typed input.
library;

import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tools/input_field.dart';
import 'package:upeg/src/rust/api/tools/file_input_policy.dart';
import 'package:upeg/src/widgets/expanded_modal/form_value.dart';
import 'package:upeg/src/widgets/expanded_modal/generic_form.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

void main() {
  group('FormValue typed JSON encoding', () {
    test('FormValue_NumberValue_encodes_5_as_JSON_5', () {
      const value = NumberValue(5);
      final json = formValuesToJsonMap({'x': value});
      expect(json, equals({'x': 5}));
      expect(json['x'], isA<num>());
    });

    test('FormValue_BooleanValue_encodes_true_as_JSON_true', () {
      const value = BooleanValue(true);
      final json = formValuesToJsonMap({'flag': value});
      expect(json, equals({'flag': true}));
      expect(json['flag'], isA<bool>());
    });

    test('FormValue_OptionValue_encodes_the_key_as_a_JSON_string', () {
      const value = OptionValue('alpha');
      final json = formValuesToJsonMap({'choice': value});
      expect(json, equals({'choice': 'alpha'}));
      expect(json['choice'], isA<String>());
    });

    test('FormValue_MultiOptionValue_encodes_keys_as_a_JSON_array', () {
      const value = MultiOptionValue(['a', 'b', 'c']);
      final json = formValuesToJsonMap({'tags': value});
      expect(
        json,
        equals({
          'tags': ['a', 'b', 'c'],
        }),
      );
      expect(json['tags'], isA<List<String>>());
    });

    test('FormValue_TextValue_encodes_strings_unchanged', () {
      const value = TextValue('hello');
      final json = formValuesToJsonMap({'msg': value});
      expect(json, equals({'msg': 'hello'}));
    });

    test('FormValue_DateTimeValue_encodes_as_an_ISO8601_string', () {
      final dt = DateTime.utc(2026, 5, 25, 10, 30);
      final value = DateTimeValue(dt);
      final json = formValuesToJsonMap({'when': value});
      expect(json['when'], equals('2026-05-25T10:30:00.000Z'));
    });

    test('a_FormValue_map_wrapped_in_ToolArgs_encodes_as_a_JSON_string', () {
      final args = formValuesToToolArgs({
        'n': const NumberValue(7),
        'flag': const BooleanValue(true),
      });

      expect(args.encodeJson(), '{"n":7,"flag":true}');
    });
  });

  group('ToolArgs JSON boundary', () {
    test('ToolArgs_interprets_an_empty_JSON_object_as_empty_arguments', () {
      final args = ToolArgs.tryDecodeObject('{}');

      expect(args, isNotNull);
      expect(args!.isEmpty, isTrue);
    });

    test(
      'ToolArgs_interprets_an_empty_object_as_null_when_nullIfEmpty_is_true',
      () {
        final args = ToolArgs.tryDecodeObject('{}', nullIfEmpty: true);

        expect(args, isNull);
      },
    );

    test('ToolArgs_returns_null_for_non_object_JSON', () {
      final args = ToolArgs.tryDecodeObject('[]');

      expect(args, isNull);
    });

    test('ToolArgs_with_the_same_nested_JSON_compare_equal', () {
      final left = ToolArgs.fromJsonObject(const <String, Object?>{
        'tags': ['a', 'b'],
        'nested': {'flag': true},
      });
      final right = ToolArgs.fromJsonObject(const <String, Object?>{
        'nested': {'flag': true},
        'tags': ['a', 'b'],
      });

      expect(left, equals(right));
      expect(left.hashCode, equals(right.hashCode));
    });
  });

  group('GenericFormController schema-aware seed', () {
    test(
      'restores selections and dates as typed values according to the input schema',
      () {
        final controller = GenericFormController();
        const fields = [
          InputFieldDto(
            key: 'choice',
            label: 'Choice',
            fieldType: InputFieldType_Select(
              options: <ChoiceOptionDto>[
                ChoiceOptionDto(value: 'alpha', label: 'alpha'),
                ChoiceOptionDto(value: 'beta', label: 'beta'),
              ],
            ),
            required_: false,
          ),
          InputFieldDto(
            key: 'when',
            label: 'When',
            fieldType: InputFieldType_DateTime(),
            required_: false,
          ),
        ];
        controller.seed(
          ToolArgs.fromJsonObject(const <String, Object?>{
            'choice': 'alpha',
            'when': '2026-07-15T12:30:00Z',
          }),
          fields: fields,
        );

        expect(controller.value('choice'), const OptionValue('alpha'));
        expect(
          controller.value('when'),
          DateTimeValue(DateTime.utc(2026, 7, 15, 12, 30)),
        );
      },
    );

    test('does not store unknown input schema keys or disallowed values', () {
      final controller = GenericFormController();
      const fields = [
        InputFieldDto(
          key: 'choice',
          label: 'Choice',
          fieldType: InputFieldType_Select(
            options: <ChoiceOptionDto>[
              ChoiceOptionDto(value: 'alpha', label: 'alpha'),
            ],
          ),
          required_: false,
        ),
        InputFieldDto(
          key: 'number',
          label: 'Number',
          fieldType: InputFieldType_Number(),
          required_: false,
        ),
      ];
      controller.seed(
        ToolArgs.fromJsonObject(const <String, Object?>{
          'choice': 'unknown',
          'number': '42',
          'extra': 'discard me',
        }),
        fields: fields,
      );

      expect(controller.value('choice'), isNull);
      expect(controller.value('number'), isNull);
      expect(controller.value('extra'), isNull);
    });

    test(
      'each field type converts only its declared JSON shape to a typed value',
      () {
        expect(
          formValueFromJson(const InputFieldType_Text(), 'text'),
          const TextValue('text'),
        );
        expect(
          formValueFromJson(const InputFieldType_Number(), 42),
          const NumberValue(42),
        );
        expect(
          formValueFromJson(const InputFieldType_Boolean(), false),
          const BooleanValue(false),
        );
        expect(
          formValueFromJson(
            const InputFieldType_Select(
              options: <ChoiceOptionDto>[
                ChoiceOptionDto(value: 'alpha', label: 'alpha'),
              ],
            ),
            'alpha',
          ),
          const OptionValue('alpha'),
        );
        expect(
          formValueFromJson(
            const InputFieldType_MultiOptions(
              options: <ChoiceOptionDto>[
                ChoiceOptionDto(value: 'alpha', label: 'alpha'),
                ChoiceOptionDto(value: 'beta', label: 'beta'),
              ],
            ),
            const <Object?>['alpha', 'beta'],
          ),
          const MultiOptionValue(['alpha', 'beta']),
        );
        expect(
          formValueFromJson(
            const InputFieldType_DateTime(),
            '2026-07-15T12:30:00Z',
          ),
          DateTimeValue(DateTime.utc(2026, 7, 15, 12, 30)),
        );
      },
    );

    test(
      'Number seeds reject non-finite values that JSON cannot represent',
      () {
        const fieldType = InputFieldType_Number();

        expect(formValueFromJson(fieldType, double.nan), isNull);
        expect(formValueFromJson(fieldType, double.infinity), isNull);
        expect(formValueFromJson(fieldType, double.negativeInfinity), isNull);
      },
    );

    test(
      'DateTime seeds reject invalid calendar dates and ISO dates without a time',
      () {
        const fieldType = InputFieldType_DateTime();

        expect(formValueFromJson(fieldType, '2020-01-42T12:30:00Z'), isNull);
        expect(formValueFromJson(fieldType, '2023-02-29T12:30:00Z'), isNull);
        expect(formValueFromJson(fieldType, '2020-02-29T24:00:00Z'), isNull);
        expect(formValueFromJson(fieldType, '2020-01-31'), isNull);
      },
    );

    test(
      'a valid ISO date and time with an offset restores as a UTC DateTimeValue',
      () {
        final value = formValueFromJson(
          const InputFieldType_DateTime(),
          '2026-07-15T12:30:00+09:00',
        );

        expect(value, DateTimeValue(DateTime.utc(2026, 7, 15, 3, 30)));
      },
    );

    test(
      'File seeds reject path strings and restore only structured objects',
      () {
        const fieldType = InputFieldType_File(
          policy: FileInputPolicyDto(
            extensions: <String>[],
            maxCount: 1,
            maxFileBytes: null,
            maxTotalBytes: null,
          ),
        );
        const structured = <String, Object?>{
          'name': 'input.bin',
          'is_dir': false,
          'content': <String, Object?>{'kind': 'bytes', 'bytes': 'AQL/'},
        };

        expect(formValueFromJson(fieldType, '/tmp/input.bin'), isNull);
        expect(
          formValueFromJson(fieldType, structured),
          FileFormValue(
            CanonicalFileValue(
              name: 'input.bin',
              isDir: false,
              content: CanonicalFileContent.bytes(
                bytes: Uint8List.fromList(const <int>[1, 2, 255]),
              ),
            ),
          ),
        );
      },
    );

    test('File seeds do not store invalid canonical base64', () {
      const fieldType = InputFieldType_File(
        policy: FileInputPolicyDto(
          extensions: <String>[],
          maxCount: 1,
          maxFileBytes: null,
          maxTotalBytes: null,
        ),
      );
      const structured = <String, Object?>{
        'name': 'input.bin',
        'is_dir': false,
        'content': <String, Object?>{'kind': 'bytes', 'bytes': 'AQI'},
      };

      expect(formValueFromJson(fieldType, structured), isNull);
    });

    test('FileFormValue encodes JSON using the Rust file shape', () {
      final value = FileFormValue(
        CanonicalFileValue(
          name: 'input.bin',
          isDir: false,
          content: CanonicalFileContent.bytes(
            bytes: Uint8List.fromList(const <int>[0, 255]),
          ),
        ),
      );

      expect(value.toJson(), <String, Object?>{
        'name': 'input.bin',
        'is_dir': false,
        'content': <String, Object?>{'kind': 'bytes', 'bytes': 'AP8='},
      });
    });

    test(
      'FileFormValue equality and hash include the full nested bytes structure',
      () {
        FileFormValue valueWithBytes(List<int> bytes) => FileFormValue(
          CanonicalFileValue(
            name: 'input.bin',
            isDir: false,
            content: CanonicalFileContent.bytes(
              bytes: Uint8List.fromList(bytes),
            ),
          ),
        );
        final left = valueWithBytes(const <int>[1, 2]);
        final same = valueWithBytes(const <int>[1, 2]);
        final different = valueWithBytes(const <int>[1, 3]);

        expect(left, same);
        expect(left.hashCode, same.hashCode);
        expect(left, isNot(different));
      },
    );
  });

  group('Seeding declared defaults', () {
    InputFieldDto field(
      InputFieldType type, {
      FieldConstraintsDto? constraints,
    }) => InputFieldDto(
      key: 'value',
      label: 'Value',
      fieldType: type,
      required_: false,
      constraints: constraints,
    );

    test('Integer_defaults_are_seeded_as_JSON_integers', () {
      final seeded = defaultFormValueFor(
        field(
          const InputFieldType_Integer(),
          constraints: const FieldConstraintsDto(
            number: NumberConstraintsDto(default_: 20),
          ),
        ),
      );

      expect(seeded, const NumberValue(20));
      // `20.0` would fail Rust's integer validation, so the seed must
      // encode as a JSON integer.
      expect(seeded!.toJson(), isA<int>());
    });

    test('Number_defaults_preserve_fractional_values', () {
      expect(
        defaultFormValueFor(
          field(
            const InputFieldType_Number(),
            constraints: const FieldConstraintsDto(
              number: NumberConstraintsDto(default_: 1.5),
            ),
          ),
        ),
        const NumberValue(1.5),
      );
    });

    test('String_defaults_are_seeded_as_TextValue', () {
      expect(
        defaultFormValueFor(
          field(
            const InputFieldType_Text(),
            constraints: const FieldConstraintsDto(
              string: StringConstraintsDto(default_: 'hello-world'),
            ),
          ),
        ),
        const TextValue('hello-world'),
      );
    });

    test('a_field_with_only_a_placeholder_is_not_seeded', () {
      expect(
        defaultFormValueFor(
          field(
            const InputFieldType_Text(),
            constraints: const FieldConstraintsDto(
              string: StringConstraintsDto(placeholder: 'my-post-title'),
            ),
          ),
        ),
        isNull,
      );
    });

    test('Boolean_is_seeded_as_false_even_without_constraints', () {
      expect(
        defaultFormValueFor(field(const InputFieldType_Boolean())),
        const BooleanValue(false),
      );
    });

    test('other_types_without_constraints_are_not_seeded', () {
      expect(defaultFormValueFor(field(const InputFieldType_Text())), isNull);
      expect(
        defaultFormValueFor(field(const InputFieldType_DateTime())),
        isNull,
      );
    });
  });

  group('Integer field JSON decoding', () {
    test('Integer_fields_reject_fractional_JSON_numbers', () {
      expect(formValueFromJson(const InputFieldType_Integer(), 1.5), isNull);
    });

    test('Integer_fields_accept_JSON_integers', () {
      expect(
        formValueFromJson(const InputFieldType_Integer(), 20),
        const NumberValue(20),
      );
    });
  });
}
