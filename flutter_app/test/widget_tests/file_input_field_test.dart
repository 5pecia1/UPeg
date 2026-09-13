library;

import 'dart:async';
import 'dart:ui' show Tristate;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/platform/file_picker_bridge.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/rust/api/tweaks.dart';
import 'package:upeg/src/state/tweaks_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/file_drop_adapter.dart';
import 'package:upeg/src/widgets/expanded_modal/file_input_field.dart';
import 'package:upeg/src/widgets/expanded_modal/file_selection_assembler.dart';
import 'package:upeg/src/widgets/expanded_modal/form_value.dart';

import '../test_helpers/i18n_test_catalog.dart';

/// 설명(description) 유무를 검증하는 케이스들이 공유하는 안내 문구.
/// 표시되는 경우와 표시되지 않는 경우가 반드시 같은 문자열을 보도록
/// 한 곳에 둔다.
const _sampleDescription = '10MB 이하의 이미지 파일만 첨부할 수 있습니다.';

/// 설명이 렌더링될 때 필드가 추가로 그리는 `Text` 줄 수. 설명이 없는
/// 트리와의 차이가 정확히 이만큼이어야 "표시하지 않는다"가 성립한다.
const _descriptionLineTextCount = 1;

/// 필드 안에 그려진 `Text` 위젯 개수. 설명 줄이 빈 문자열이나 자리표시자로
/// 남지 않고 아예 사라졌는지 세기 위한 것이다.
int _fieldTextCount(WidgetTester tester) => tester
    .widgetList<Text>(
      find.descendant(
        of: find.byType(FileInputField),
        matching: find.byType(Text),
      ),
    )
    .length;

final class _PickerBridge extends FilePickerBridge {
  _PickerBridge({this.files, this.pendingFiles, this.errorToThrow});

  final List<PickedFileData>? files;
  final Future<List<PickedFileData>?>? pendingFiles;
  final Object? errorToThrow;
  int calls = 0;
  bool? allowMultiple;
  List<String>? allowedExtensions;
  int? maxCount;
  int? maxFileBytes;
  int? maxTotalBytes;

  @override
  Future<List<PickedFileData>?> pickOpenFiles({
    String dialogTitle = 'Pick files',
    List<String>? allowedExtensions,
    bool allowMultiple = false,
    int? maxCount,
    int? maxFileBytes,
    int? maxTotalBytes,
  }) async {
    calls += 1;
    this.allowMultiple = allowMultiple;
    this.allowedExtensions = allowedExtensions;
    this.maxCount = maxCount;
    this.maxFileBytes = maxFileBytes;
    this.maxTotalBytes = maxTotalBytes;
    final error = errorToThrow;
    if (error != null) throw error;
    final pending = pendingFiles;
    if (pending != null) return pending;
    return files;
  }

  @override
  Future<PickedFileData?> pickOpenFile({
    String dialogTitle = 'Pick a file',
    List<String>? allowedExtensions,
  }) async => files?.firstOrNull;

  @override
  Future<String?> pickOpenPath() async => null;

  @override
  Future<String?> pickSavePath({
    required String defaultName,
    String dialogTitle = 'Save file',
    List<String>? allowedExtensions,
  }) async => null;

  @override
  Future<void> writeBytesTo(String path, Uint8List bytes) async {}
}

final class _DropAdapter implements FileDropAdapter {
  FileDropCallbacks? callbacks;

  @override
  Widget wrap({required Widget child, required FileDropCallbacks callbacks}) {
    this.callbacks = callbacks;
    return child;
  }
}

final class _FileFieldHarness extends StatefulWidget {
  const _FileFieldHarness({
    required this.controller,
    required this.picker,
    required this.dropAdapter,
    required this.policy,
    this.compact = false,
    this.width,
    this.description,
  });

  final GenericFileValueController controller;
  final FilePickerBridge picker;
  final FileDropAdapter dropAdapter;
  final FileSelectionPolicy policy;
  final bool compact;
  final double? width;
  final String? description;

  @override
  State<_FileFieldHarness> createState() => _FileFieldHarnessState();
}

final class _FileFieldHarnessState extends State<_FileFieldHarness> {
  @override
  Widget build(BuildContext context) {
    return ProviderScope(
      overrides: [...i18nTestOverrides],
      child: MaterialApp(
        theme: UpegTheme.lightTheme(),
        home: Scaffold(
          body: Align(
            alignment: Alignment.topCenter,
            child: SizedBox(
              width: widget.width,
              child: FileInputField(
                label: '첨부 파일',
                description: widget.description,
                value: widget.controller.value,
                policy: widget.policy,
                pickerBridge: widget.picker,
                dropAdapter: widget.dropAdapter,
                compact: widget.compact,
                onChanged: (value) {
                  setState(() => widget.controller.value = value);
                },
                onCleared: () {
                  setState(() => widget.controller.value = null);
                },
              ),
            ),
          ),
        ),
      ),
    );
  }
}

final class GenericFileValueController {
  GenericFileValueController([this.value]);

  FileFormValue? value;
}

final class _FileInputFieldRobot {
  _FileInputFieldRobot(this.tester, this.dropAdapter);

  final WidgetTester tester;
  final _DropAdapter dropAdapter;

  Future<void> tapPickButton() async {
    await tester.tap(find.byKey(FileInputKeys.pickButton));
    await tester.pumpAndSettle();
  }

  Future<void> activatePickButtonWithSpace() async {
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.space);
    await tester.pumpAndSettle();
  }

  Future<void> enterDrag() async {
    dropAdapter.callbacks!.onEntered();
    await tester.pump();
  }

  Future<void> exitDrag() async {
    dropAdapter.callbacks!.onExited();
    await tester.pump();
  }

  Future<void> dropFiles(List<FileSelectionCandidate> files) async {
    await dropAdapter.callbacks!.onDropped(files);
    await tester.pumpAndSettle();
  }

  Future<void> clearSelection() async {
    await tester.tap(find.byKey(FileInputKeys.clearButton));
    await tester.pump();
  }

  Future<Object?> tapPickButtonAndTakeException() async {
    await tester.tap(find.byKey(FileInputKeys.pickButton));
    await tester.pump();
    return tester.takeException();
  }

  Future<void> expectDropThrows(
    List<FileSelectionCandidate> files,
    Matcher matcher,
  ) async {
    await expectLater(
      dropAdapter.callbacks!.onDropped(files),
      throwsA(matcher),
    );
    await tester.pump();
  }

  Future<void> focusNextAction() async {
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
  }

  void expectSelectionSummary(int count, Iterable<String> names) {
    expect(find.text('$count개 파일 선택됨'), findsOneWidget);
    for (final name in names) {
      expect(find.text(name), findsOneWidget);
    }
  }

  void expectSingleSelectionSummary(String name) {
    expect(find.text('1개 파일 선택됨'), findsOneWidget);
    expect(find.text(name), findsOneWidget);
  }

  void expectError(String message) {
    expect(find.text(message), findsOneWidget);
  }

  void expectPickButtonAccessible() {
    final semantics = tester.getSemantics(find.byKey(FileInputKeys.pickButton));
    expect(semantics.label, contains(i18nEn('modal.generic.file_pick')));
    expect(semantics.flagsCollection.isButton, isTrue);
    expect(semantics.flagsCollection.isEnabled, Tristate.isTrue);
  }

  void expectClearButtonTooltip() {
    final iconButton = tester.widget<IconButton>(
      find.byKey(FileInputKeys.clearButton),
    );
    expect(iconButton.tooltip, i18nEn('modal.generic.file_clear'));
  }

  void expectEmptyPromptUsesTwoSemanticLines() {
    const expectedPrompt = '파일을 선택하거나\n여기로 끌어 놓으세요.';
    final promptFinder = find.text(expectedPrompt);
    final prompt = tester.widget<Text>(promptFinder);

    expect(prompt.data, expectedPrompt);
    expect(prompt.data!.split('\n'), ['파일을 선택하거나', '여기로 끌어 놓으세요.']);
    expect(tester.getSemantics(promptFinder).label, expectedPrompt);
  }

  Border dropTargetBorder() {
    final container = tester.widget<AnimatedContainer>(
      find.byKey(FileInputKeys.dropTarget),
    );
    final decoration = container.decoration! as BoxDecoration;
    return decoration.border! as Border;
  }

  void expectActionFocusRing(Key key, Color expectedColor) {
    final decoratedBox = tester.widget<DecoratedBox>(find.byKey(key));
    final decoration = decoratedBox.decoration as BoxDecoration;
    final border = decoration.border! as Border;
    expect(border.top.color, expectedColor);
  }

  Color focusRingToken(Key key) =>
      tester.element(find.byKey(key)).upeg.focusRing;
}

PickedFileData _picked(String name, List<int> bytes) =>
    (name: name, bytes: Uint8List.fromList(bytes), mime: null);

FileSelectionCandidate _dropped(String name, List<int> bytes) {
  return FileSelectionCandidate.file(
    name: name,
    readBytes: (_) async => Uint8List.fromList(bytes),
  );
}

FileFormValue _existingValue(String name) {
  return FileFormValue(
    CanonicalFileValue(
      name: name,
      isDir: false,
      content: CanonicalFileContent.bytes(bytes: Uint8List.fromList([1])),
      mime: null,
    ),
  );
}

Future<_FileInputFieldRobot> _pumpField(
  WidgetTester tester, {
  required GenericFileValueController controller,
  required FilePickerBridge picker,
  required FileSelectionPolicy policy,
  bool compact = false,
  double? width,
  String? description,
}) async {
  final dropAdapter = _DropAdapter();
  await tester.pumpWidget(
    _FileFieldHarness(
      controller: controller,
      picker: picker,
      dropAdapter: dropAdapter,
      policy: policy,
      compact: compact,
      width: width,
      description: description,
    ),
  );
  return _FileInputFieldRobot(tester, dropAdapter);
}

void main() {
  group('FileInputField', () {
    testWidgets('빈 파일 안내는 좁은 증거 폭에서도 단어를 쪼개지 않고 의미 단위 두 줄로 표시한다', (
      tester,
    ) async {
      final robot = await _pumpField(
        tester,
        controller: GenericFileValueController(),
        picker: _PickerBridge(),
        policy: const FileSelectionPolicy(extensions: [], maxCount: 1),
        width: 240,
      );

      robot.expectEmptyPromptUsesTwoSemanticLines();
      expect(tester.takeException(), isNull);
    });

    testWidgets('설명이 주어지면 라벨 아래에 안내 문구를 표시한다', (tester) async {
      await _pumpField(
        tester,
        controller: GenericFileValueController(),
        picker: _PickerBridge(),
        policy: const FileSelectionPolicy(extensions: [], maxCount: 1),
        description: _sampleDescription,
      );

      expect(find.text(_sampleDescription), findsOneWidget);
    });

    testWidgets('설명이 없으면 안내 문구를 표시하지 않는다', (tester) async {
      await _pumpField(
        tester,
        controller: GenericFileValueController(),
        picker: _PickerBridge(),
        policy: const FileSelectionPolicy(extensions: [], maxCount: 1),
        description: _sampleDescription,
      );
      final withDescription = _fieldTextCount(tester);

      await _pumpField(
        tester,
        controller: GenericFileValueController(),
        picker: _PickerBridge(),
        policy: const FileSelectionPolicy(extensions: [], maxCount: 1),
      );

      expect(find.text(_sampleDescription), findsNothing);
      expect(
        _fieldTextCount(tester),
        withDescription - _descriptionLineTextCount,
      );
      expect(tester.takeException(), isNull);
    });

    testWidgets('compact 모드에서는 설명이 주어져도 표시하지 않는다', (tester) async {
      await _pumpField(
        tester,
        controller: GenericFileValueController(),
        picker: _PickerBridge(),
        policy: const FileSelectionPolicy(extensions: [], maxCount: 1),
        compact: true,
        width: 150,
        description: _sampleDescription,
      );

      expect(find.text(_sampleDescription), findsNothing);
    });

    testWidgets('picker로 여러 파일을 선택하면 개수와 이름을 표시한다', (tester) async {
      final controller = GenericFileValueController();
      final picker = _PickerBridge(
        files: [
          _picked('a.txt', [1]),
          _picked('b.txt', [2]),
        ],
      );
      final robot = await _pumpField(
        tester,
        controller: controller,
        picker: picker,
        policy: const FileSelectionPolicy(extensions: ['txt'], maxCount: 3),
      );

      await robot.tapPickButton();

      robot.expectSelectionSummary(2, ['a.txt', 'b.txt']);
      expect(picker.allowMultiple, isTrue);
      expect(picker.allowedExtensions, ['txt']);
      expect(picker.maxCount, 3);
      expect(controller.value!.value.isDir, isTrue);
    });

    testWidgets('외부 파일이 들어오고 나가면 드롭 영역 강조가 바뀐다', (tester) async {
      final robot = await _pumpField(
        tester,
        controller: GenericFileValueController(),
        picker: _PickerBridge(),
        policy: const FileSelectionPolicy(extensions: [], maxCount: 2),
      );

      final idleBorder = robot.dropTargetBorder();
      final idleSize = tester.getSize(find.byKey(FileInputKeys.dropTarget));
      await robot.enterDrag();
      final activeBorder = robot.dropTargetBorder();
      final activeSize = tester.getSize(find.byKey(FileInputKeys.dropTarget));
      expect(activeBorder.top.color, isNot(idleBorder.top.color));
      expect(activeBorder.top.width, idleBorder.top.width);
      expect(activeSize, idleSize);
      await robot.exitDrag();

      expect(robot.dropTargetBorder().top.color, idleBorder.top.color);
    });

    testWidgets('외부 파일을 드롭하면 picker와 같은 조립 규칙을 사용한다', (tester) async {
      final controller = GenericFileValueController();
      final robot = await _pumpField(
        tester,
        controller: controller,
        picker: _PickerBridge(),
        policy: const FileSelectionPolicy(extensions: [], maxCount: 2),
      );

      await robot.dropFiles([
        _dropped('drop.bin', [7]),
      ]);

      robot.expectSingleSelectionSummary('drop.bin');
      expect(controller.value!.value.isDir, isTrue);
    });

    testWidgets('새 선택이 검증에 실패하면 이전 값을 보존하고 한국어 오류를 표시한다', (tester) async {
      final controller = GenericFileValueController(_existingValue('old.txt'));
      final robot = await _pumpField(
        tester,
        controller: controller,
        picker: _PickerBridge(
          files: [
            _picked('bad.pdf', [1]),
          ],
        ),
        policy: const FileSelectionPolicy(extensions: ['txt'], maxCount: 1),
      );

      await robot.tapPickButton();

      robot.expectSingleSelectionSummary('old.txt');
      robot.expectError('허용되지 않는 파일 형식입니다: bad.pdf');
      expect(controller.value!.value.name, 'old.txt');
    });

    testWidgets('지우기 동작은 canonical 값을 제거한다', (tester) async {
      final controller = GenericFileValueController(_existingValue('old.txt'));
      final robot = await _pumpField(
        tester,
        controller: controller,
        picker: _PickerBridge(),
        policy: const FileSelectionPolicy(extensions: [], maxCount: 1),
      );

      robot.expectClearButtonTooltip();
      await robot.clearSelection();

      expect(controller.value, isNull);
    });

    testWidgets('파일 선택 버튼은 semantics와 키보드 활성화를 제공한다', (tester) async {
      final picker = _PickerBridge(
        files: [
          _picked('key.bin', [1]),
        ],
      );
      final robot = await _pumpField(
        tester,
        controller: GenericFileValueController(),
        picker: picker,
        policy: const FileSelectionPolicy(extensions: [], maxCount: 1),
      );

      robot.expectPickButtonAccessible();
      await robot.activatePickButtonWithSpace();

      expect(picker.calls, 1);
    });

    testWidgets('좁은 compact 폭에서는 선택과 지우기 동작이 줄바꿈되어 넘치지 않는다', (tester) async {
      await _pumpField(
        tester,
        controller: GenericFileValueController(_existingValue('기존.txt')),
        picker: _PickerBridge(),
        policy: const FileSelectionPolicy(extensions: [], maxCount: 1),
        compact: true,
        width: 150,
      );

      expect(tester.takeException(), isNull);
      expect(find.byKey(FileInputKeys.pickButton), findsOneWidget);
      expect(find.byKey(FileInputKeys.clearButton), findsOneWidget);
    });

    testWidgets('긴 혼합 문자 파일 이름은 한 줄 말줄임으로 제한한다', (tester) async {
      const longName = '매우-긴-보고서-final-version-with-many-segments.txt';
      await _pumpField(
        tester,
        controller: GenericFileValueController(_existingValue(longName)),
        picker: _PickerBridge(),
        policy: const FileSelectionPolicy(extensions: [], maxCount: 1),
        compact: true,
        width: 150,
      );

      final name = tester.widget<Text>(find.text(longName));
      expect(name.maxLines, 1);
      expect(name.softWrap, isFalse);
      expect(name.overflow, TextOverflow.ellipsis);
      expect(tester.takeException(), isNull);
    });

    testWidgets('picker 읽기 오류는 이전 값을 보존하고 한국어 오류를 표시한다', (tester) async {
      final controller = GenericFileValueController(_existingValue('기존.txt'));
      final robot = await _pumpField(
        tester,
        controller: controller,
        picker: _PickerBridge(
          errorToThrow: const FilePickerReadFailure('읽을수없음.txt'),
        ),
        policy: const FileSelectionPolicy(extensions: [], maxCount: 1),
      );

      await robot.tapPickButton();

      robot.expectSingleSelectionSummary('기존.txt');
      robot.expectError('파일을 읽지 못했습니다: 읽을수없음.txt');
      expect(controller.value!.value.name, '기존.txt');
    });

    testWidgets('picker 확장자 검증 오류는 이전 값을 보존하고 기존 한국어 오류를 표시한다', (tester) async {
      final controller = GenericFileValueController(_existingValue('기존.txt'));
      final robot = await _pumpField(
        tester,
        controller: controller,
        picker: _PickerBridge(
          errorToThrow: const FilePickerSelectionFailure(
            FilePickerSelectionErrorCode.extensionNotAllowed,
            fileName: '거부됨.pdf',
          ),
        ),
        policy: const FileSelectionPolicy(extensions: ['txt'], maxCount: 1),
      );

      await robot.tapPickButton();

      robot.expectSingleSelectionSummary('기존.txt');
      robot.expectError('허용되지 않는 파일 형식입니다: 거부됨.pdf');
      expect(controller.value!.value.name, '기존.txt');
    });

    testWidgets('picker가 파일을 읽는 동안 선택과 지우기 동작을 비활성화한다', (tester) async {
      final pending = Completer<List<PickedFileData>?>();
      await _pumpField(
        tester,
        controller: GenericFileValueController(_existingValue('기존.txt')),
        picker: _PickerBridge(pendingFiles: pending.future),
        policy: const FileSelectionPolicy(extensions: [], maxCount: 1),
      );

      await tester.tap(find.byKey(FileInputKeys.pickButton));
      await tester.pump();

      expect(
        tester
            .widget<FilledButton>(find.byKey(FileInputKeys.pickButton))
            .onPressed,
        isNull,
      );
      expect(
        tester
            .widget<IconButton>(find.byKey(FileInputKeys.clearButton))
            .onPressed,
        isNull,
      );
      expect(find.byType(CircularProgressIndicator), findsOneWidget);

      pending.complete(null);
      await tester.pumpAndSettle();
    });

    testWidgets('picker의 programmer Error는 사용자 오류로 숨기지 않고 이전 값을 보존한다', (
      tester,
    ) async {
      final controller = GenericFileValueController(_existingValue('기존.txt'));
      final robot = await _pumpField(
        tester,
        controller: controller,
        picker: _PickerBridge(errorToThrow: StateError('programmer bug')),
        policy: const FileSelectionPolicy(extensions: [], maxCount: 1),
      );

      final error = await robot.tapPickButtonAndTakeException();

      expect(error, isA<StateError>());
      robot.expectSingleSelectionSummary('기존.txt');
      expect(controller.value!.value.name, '기존.txt');
    });

    testWidgets('drop reader의 programmer Error는 사용자 오류로 숨기지 않고 이전 값을 보존한다', (
      tester,
    ) async {
      final controller = GenericFileValueController(_existingValue('기존.txt'));
      final robot = await _pumpField(
        tester,
        controller: controller,
        picker: _PickerBridge(),
        policy: const FileSelectionPolicy(extensions: [], maxCount: 1),
      );
      final candidate = FileSelectionCandidate.file(
        name: 'broken.bin',
        readBytes: (_) async => throw StateError('programmer bug'),
      );

      await robot.expectDropThrows([candidate], isA<StateError>());

      robot.expectSingleSelectionSummary('기존.txt');
      expect(controller.value!.value.name, '기존.txt');
    });

    testWidgets('Tab으로 실제 action FocusNode가 이동하면 token focus ring이 표시된다', (
      tester,
    ) async {
      final robot = await _pumpField(
        tester,
        controller: GenericFileValueController(_existingValue('기존.txt')),
        picker: _PickerBridge(),
        policy: const FileSelectionPolicy(extensions: [], maxCount: 1),
      );
      final focusRingColor = robot.focusRingToken(FileInputKeys.pickFocusRing);

      robot.expectActionFocusRing(
        FileInputKeys.pickFocusRing,
        Colors.transparent,
      );
      await robot.focusNextAction();
      robot.expectActionFocusRing(FileInputKeys.pickFocusRing, focusRingColor);

      await robot.focusNextAction();
      robot.expectActionFocusRing(
        FileInputKeys.pickFocusRing,
        Colors.transparent,
      );
      robot.expectActionFocusRing(FileInputKeys.clearFocusRing, focusRingColor);
    });

    testWidgets('선택/지우기 라벨은 i18n 카탈로그에서 오며 로케일을 바꾸면 다시 렌더된다', (tester) async {
      TweaksDto tweaks(String locale) => TweaksDto(
        theme: 'Dark',
        accent: 'Green',
        showHoles: true,
        locale: locale,
        localHttpHost: false,
      );
      final controller = GenericFileValueController(_existingValue('old.txt'));
      final dropAdapter = _DropAdapter();
      final container = ProviderContainer(
        overrides: [
          ...i18nTestOverrides,
          tweaksLoaderProvider.overrideWith(
            (ref) =>
                () => tweaks('En'),
          ),
          tweaksSaverProvider.overrideWith((ref) => (TweaksDto _) {}),
        ],
      );
      addTearDown(container.dispose);
      await container.read(tweaksProvider.future);

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            theme: UpegTheme.lightTheme(),
            home: Scaffold(
              body: FileInputField(
                label: '첨부 파일',
                value: controller.value,
                policy: const FileSelectionPolicy(extensions: [], maxCount: 1),
                pickerBridge: _PickerBridge(),
                dropAdapter: dropAdapter,
                onChanged: (value) => controller.value = value,
                onCleared: () => controller.value = null,
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text(i18nEn('modal.generic.file_pick')), findsOneWidget);
      expect(
        tester
            .widget<IconButton>(find.byKey(FileInputKeys.clearButton))
            .tooltip,
        i18nEn('modal.generic.file_clear'),
      );
      expect(find.text(i18nKo('modal.generic.file_pick')), findsNothing);

      await container.read(tweaksProvider.notifier).save(tweaks('Ko'));
      await tester.pumpAndSettle();

      expect(find.text(i18nKo('modal.generic.file_pick')), findsOneWidget);
      expect(
        tester
            .widget<IconButton>(find.byKey(FileInputKeys.clearButton))
            .tooltip,
        i18nKo('modal.generic.file_clear'),
      );
      expect(find.text(i18nEn('modal.generic.file_pick')), findsNothing);
    });
  });
}
