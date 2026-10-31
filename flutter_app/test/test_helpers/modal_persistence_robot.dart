import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/pages/expanded_modal_page.dart';
import 'package:upeg/src/rust/api/tools.dart';
import 'package:upeg/src/state/pin_provider.dart';
import 'package:upeg/src/theme/upeg_theme.dart';
import 'package:upeg/src/widgets/expanded_modal/primary_button.dart';
import 'package:upeg/src/widgets/expanded_modal/tool_args.dart';

class ModalPersistenceRobot {
  ModalPersistenceRobot(this.tester);

  final WidgetTester tester;

  Future<void> pumpScreen({
    required ProviderContainer container,
    required ToolDto tool,
    PinKey? pinKey,
    ToolArgs? initialInput,
  }) async {
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          theme: UpegTheme.darkTheme(),
          home: Scaffold(
            body: Builder(
              builder: (context) => TextButton(
                onPressed: () => unawaited(
                  ExpandedModalPage.open(
                    context,
                    tool,
                    pinKey: pinKey,
                    initialInput: initialInput,
                  ),
                ),
                child: const Text('Open tool'),
              ),
            ),
          ),
        ),
      ),
    );
    await tester.tap(find.text('Open tool'));
    await tester.pumpAndSettle();
  }

  Future<void> openSettingsRoute() async {
    final context = tester.element(
      find.byKey(const Key('expanded-modal-close-btn')),
    );
    unawaited(
      Navigator.of(context).push<void>(
        MaterialPageRoute(
          builder: (_) => const Scaffold(body: Text('Settings route')),
        ),
      ),
    );
    await tester.pumpAndSettle();
  }

  void expectSettingsRouteVisible() =>
      expect(find.text('Settings route'), findsOneWidget);
  void expectModalNotVisible() =>
      expect(find.byType(ExpandedModalPage, skipOffstage: false), findsNothing);

  Future<void> editField(String value, {String key = 'field-value'}) async {
    await tester.enterText(find.byKey(Key(key)), value);
    await tester.pump();
  }

  void expectField(String value, {String key = 'field-value'}) {
    final finder = find.byKey(Key(key));
    final textField = finder.evaluate().single.widget is TextField
        ? finder
        : find.descendant(of: finder, matching: find.byType(TextField));
    expect(tester.widget<TextField>(textField).controller!.text, value);
  }

  Future<void> retrySave() async {
    await tester.tap(find.byKey(const Key('pin-state-retry')));
    await tester.pumpAndSettle();
  }

  Future<void> tryFocusBlockedField({String key = 'field-value'}) async {
    final field = find.byKey(Key(key));
    expect(field.hitTestable(), findsNothing);
    await tester.tap(field, warnIfMissed: false);
    await tester.pump();
    final editable = find.descendant(
      of: field,
      matching: find.byType(EditableText),
    );
    expect(tester.widget<EditableText>(editable).focusNode.hasFocus, isFalse);
  }

  void expectEditingAllowed({String key = 'field-value'}) =>
      expect(find.byKey(Key(key)).hitTestable(), findsOneWidget);

  Future<void> pressF1OutsideEditor() async {
    final context = tester.element(
      find.byKey(const Key('expanded-modal-close-btn')),
    );
    Focus.of(context).requestFocus();
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.f1);
    await tester.pumpAndSettle();
  }

  Future<void> pressEnterOutsideEditor() async {
    FocusScope.of(
      tester.element(find.byKey(const Key('expanded-modal-run-btn'))),
    ).requestFocus();
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
  }

  Future<void> run({bool settle = true}) async {
    await tester.tap(find.byKey(const Key('expanded-modal-run-btn')));
    if (settle) {
      await tester.pumpAndSettle();
    } else {
      await tester.pump();
    }
  }

  Future<void> pressF2() async {
    await tester.sendKeyEvent(LogicalKeyboardKey.f2);
    await tester.pump();
  }

  Future<void> pressF3() async {
    await tester.sendKeyEvent(LogicalKeyboardKey.f3);
    await tester.pump();
  }

  void expectPinDisabled() => expect(
    tester
        .widget<TextButton>(find.byKey(const Key('expanded-modal-pin-btn')))
        .onPressed,
    isNull,
  );

  void expectCopiedVisible() =>
      expect(find.text('Copied').hitTestable(), findsWidgets);
  void expectCopiedNotVisible() => expect(find.text('Copied'), findsNothing);
  void expectCopyFailureVisible() => expect(
    find.text('Could not copy. Try again.').hitTestable(),
    findsWidgets,
  );

  Future<void> pin({bool settle = true}) async {
    await tester.tap(find.byKey(const Key('expanded-modal-pin-btn')));
    if (settle) {
      await tester.pumpAndSettle();
    } else {
      await tester.pump();
    }
  }

  Future<void> close() async {
    await tester.tap(find.byKey(const Key('expanded-modal-close-btn')));
    await tester.pumpAndSettle();
  }

  Future<void> cancel() async {
    await tester.tap(find.byKey(const Key('expanded-modal-cancel-btn')));
    await tester.pump();
  }

  void expectPinFailureVisible() => expect(
    find.text('Could not pin tool. Try again.').hitTestable(),
    findsWidgets,
  );
  void expectPinFailureNotVisible() =>
      expect(find.text('Could not pin tool. Try again.'), findsNothing);

  void expectRunEnabled() => expect(
    tester
        .widget<ExpandedModalPrimaryButton>(
          find.byKey(const Key('expanded-modal-run-btn')),
        )
        .enabled,
    isTrue,
  );

  void expectRestoredVisible() =>
      expect(find.text('Saved result'), findsOneWidget);
  void expectRestoredNotVisible() =>
      expect(find.text('Saved result'), findsNothing);
  void expectStaleVisible() => expect(
    find.text('Result from previous input · run again'),
    findsOneWidget,
  );
  void expectStaleNotVisible() =>
      expect(find.text('Result from previous input · run again'), findsNothing);
  void expectResultVisible(String result) =>
      expect(find.text(result), findsOneWidget);
  void expectResultNotVisible(String result) =>
      expect(find.text(result), findsNothing);
  void expectCancelNotVisible() =>
      expect(find.byKey(const Key('expanded-modal-cancel-btn')), findsNothing);
  void expectCancelVisible() => expect(
    find.byKey(const Key('expanded-modal-cancel-btn')),
    findsOneWidget,
  );
  void expectCancelRequested() {
    expect(find.text('Cancel requested…'), findsOneWidget);
    expect(
      tester
          .widget<TextButton>(
            find.byKey(const Key('expanded-modal-cancel-btn')),
          )
          .onPressed,
      isNull,
    );
    expect(
      tester
          .widget<ExpandedModalPrimaryButton>(
            find.byKey(const Key('expanded-modal-run-btn')),
          )
          .enabled,
      isFalse,
    );
  }
}
