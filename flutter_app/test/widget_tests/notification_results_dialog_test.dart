import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/i18n/t.dart';
import 'package:upeg/src/rust/api/i18n.dart';
import 'package:upeg/src/rust/api/notification_results.dart';
import 'package:upeg/src/state/locale_provider.dart';
import 'package:upeg/src/state/notification_results_provider.dart';
import 'package:upeg/src/widgets/notification_results_dialog.dart';

void main() {
  testWidgets('acknowledges each result only after it is displayed', (
    tester,
  ) async {
    final acknowledged = <String>[];
    NotificationResultDto result(String id) => NotificationResultDto(
      id: id,
      toolId: id,
      title: 'Title $id',
      summary: 'Summary $id',
      success: true,
      createdAtMs: 0,
    );
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          localeProvider.overrideWithValue(LocaleDto.en),
          i18nTranslateOverride.overrideWithValue((key, _) => key),
          unreadNotificationResultsProvider.overrideWith(
            (ref) => const Stream<bool>.empty(),
          ),
          notificationResultsLoaderProvider.overrideWithValue(
            () async => [result('a'), result('b')],
          ),
          notificationResultsAcknowledgerProvider.overrideWithValue(
            (ids) async => acknowledged.addAll(ids),
          ),
        ],
        child: MaterialApp(
          navigatorKey: notificationResultsNavigatorKey,
          home: const Scaffold(),
        ),
      ),
    );
    showNotificationResults();
    await tester.pumpAndSettle();
    expect(find.text('Title a'), findsOneWidget);
    expect(acknowledged, ['a']);
    await tester.tap(find.text('notifications.next'));
    await tester.pumpAndSettle();
    expect(find.text('Title b'), findsOneWidget);
    expect(acknowledged, ['a', 'b']);
    await tester.tap(find.text('notifications.close'));
    await tester.pumpAndSettle();
  });
}
