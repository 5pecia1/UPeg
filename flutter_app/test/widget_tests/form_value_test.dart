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
    test('FormValue_NumberValue는_5를_JSON_5로_인코딩한다', () {
      const value = NumberValue(5);
      final json = formValuesToJsonMap({'x': value});
      expect(json, equals({'x': 5}));
      expect(json['x'], isA<num>());
    });

    test('FormValue_BooleanValue는_true를_JSON_true로_인코딩한다', () {
      const value = BooleanValue(true);
      final json = formValuesToJsonMap({'flag': value});
      expect(json, equals({'flag': true}));
      expect(json['flag'], isA<bool>());
    });

    test('FormValue_OptionValue는_key를_JSON_string으로_인코딩한다', () {
      const value = OptionValue('alpha');
      final json = formValuesToJsonMap({'choice': value});
      expect(json, equals({'choice': 'alpha'}));
      expect(json['choice'], isA<String>());
    });

    test('FormValue_MultiOptionValue는_keys를_JSON_array로_인코딩한다', () {
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

    test('FormValue_TextValue는_문자열을_그대로_인코딩한다', () {
      const value = TextValue('hello');
      final json = formValuesToJsonMap({'msg': value});
      expect(json, equals({'msg': 'hello'}));
    });

    test('FormValue_DateTimeValue는_ISO8601_문자열로_인코딩한다', () {
      final dt = DateTime.utc(2026, 5, 25, 10, 30);
      final value = DateTimeValue(dt);
      final json = formValuesToJsonMap({'when': value});
      expect(json['when'], equals('2026-05-25T10:30:00.000Z'));
    });

    test('FormValue_map은_ToolArgs로_감싸서_JSON_문자열로_인코딩한다', () {
      final args = formValuesToToolArgs({
        'n': const NumberValue(7),
        'flag': const BooleanValue(true),
      });

      expect(args.encodeJson(), '{"n":7,"flag":true}');
    });
  });

  group('ToolArgs JSON boundary', () {
    test('ToolArgs는_빈_JSON_object를_비어있는_인자로_해석한다', () {
      final args = ToolArgs.tryDecodeObject('{}');

      expect(args, isNotNull);
      expect(args!.isEmpty, isTrue);
    });

    test('ToolArgs는_nullIfEmpty가_true이면_빈_object를_null로_해석한다', () {
      final args = ToolArgs.tryDecodeObject('{}', nullIfEmpty: true);

      expect(args, isNull);
    });

    test('ToolArgs는_JSON_object가_아니면_null을_반환한다', () {
      final args = ToolArgs.tryDecodeObject('[]');

      expect(args, isNull);
    });

    test('ToolArgs는_같은_nested_JSON이면_같은_값으로_비교된다', () {
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
    test('입력 스키마에 맞게 선택값과 날짜를 typed 값으로 복원한다', () {
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
    });

    test('입력 스키마에 없는 키와 허용되지 않은 값은 저장하지 않는다', () {
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

    test('각 field 종류는 선언된 JSON shape만 typed 값으로 변환한다', () {
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
    });

    test('Number seed는 JSON으로 표현할 수 없는 non-finite 값을 거부한다', () {
      const fieldType = InputFieldType_Number();

      expect(formValueFromJson(fieldType, double.nan), isNull);
      expect(formValueFromJson(fieldType, double.infinity), isNull);
      expect(formValueFromJson(fieldType, double.negativeInfinity), isNull);
    });

    test('잘못된 달력 날짜와 시간 없는 ISO 날짜는 DateTime seed로 허용하지 않는다', () {
      const fieldType = InputFieldType_DateTime();

      expect(formValueFromJson(fieldType, '2020-01-42T12:30:00Z'), isNull);
      expect(formValueFromJson(fieldType, '2023-02-29T12:30:00Z'), isNull);
      expect(formValueFromJson(fieldType, '2020-02-29T24:00:00Z'), isNull);
      expect(formValueFromJson(fieldType, '2020-01-31'), isNull);
    });

    test('offset이 있는 올바른 ISO 날짜와 시간은 UTC DateTimeValue로 복원한다', () {
      final value = formValueFromJson(
        const InputFieldType_DateTime(),
        '2026-07-15T12:30:00+09:00',
      );

      expect(value, DateTimeValue(DateTime.utc(2026, 7, 15, 3, 30)));
    });

    test('File seed는 path 문자열을 거부하고 structured object만 복원한다', () {
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
    });

    test('File seed는 잘못된 canonical base64를 저장하지 않는다', () {
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

    test('FileFormValue는 Rust file shape로 JSON을 인코딩한다', () {
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

    test('FileFormValue 동등성과 hash는 재귀 bytes 구조를 모두 비교한다', () {
      FileFormValue valueWithBytes(List<int> bytes) => FileFormValue(
        CanonicalFileValue(
          name: 'input.bin',
          isDir: false,
          content: CanonicalFileContent.bytes(bytes: Uint8List.fromList(bytes)),
        ),
      );
      final left = valueWithBytes(const <int>[1, 2]);
      final same = valueWithBytes(const <int>[1, 2]);
      final different = valueWithBytes(const <int>[1, 3]);

      expect(left, same);
      expect(left.hashCode, same.hashCode);
      expect(left, isNot(different));
    });
  });

  group('선언된 기본값 시딩', () {
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

    test('Integer_기본값은_JSON_정수로_시드된다', () {
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

    test('Number_기본값은_소수를_유지한다', () {
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

    test('String_기본값은_TextValue로_시드된다', () {
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

    test('placeholder만_선언된_필드는_시드되지_않는다', () {
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

    test('Boolean은_제약이_없어도_false로_시드된다', () {
      expect(
        defaultFormValueFor(field(const InputFieldType_Boolean())),
        const BooleanValue(false),
      );
    });

    test('제약이_없는_다른_종류는_시드되지_않는다', () {
      expect(defaultFormValueFor(field(const InputFieldType_Text())), isNull);
      expect(
        defaultFormValueFor(field(const InputFieldType_DateTime())),
        isNull,
      );
    });
  });

  group('Integer 필드 JSON 디코딩', () {
    test('Integer_필드는_소수_JSON을_거부한다', () {
      expect(formValueFromJson(const InputFieldType_Integer(), 1.5), isNull);
    });

    test('Integer_필드는_정수_JSON을_받아들인다', () {
      expect(
        formValueFromJson(const InputFieldType_Integer(), 20),
        const NumberValue(20),
      );
    });
  });
}
