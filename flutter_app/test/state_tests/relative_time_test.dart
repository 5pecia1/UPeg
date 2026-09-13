/// Unit tests for the pure "last run · N ago" bucketing formatter.
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/i18n/relative_time.dart';

void main() {
  group('lastRunAgoLabel', () {
    test('1분_미만은_방금으로_버킷된다', () {
      final label = lastRunAgoLabel(Duration.zero);
      expect(label.key, 'pin.last_run.just_now');
      expect(label.args, isEmpty);
      expect(
        lastRunAgoLabel(const Duration(seconds: 59)).key,
        'pin.last_run.just_now',
      );
    });

    test('음수_경과시간은_미래_예측_대신_방금으로_클램프된다', () {
      expect(
        lastRunAgoLabel(const Duration(minutes: -5)).key,
        'pin.last_run.just_now',
      );
    });

    test('1시간_미만은_분_단위로_버킷된다', () {
      final five = lastRunAgoLabel(const Duration(minutes: 5));
      expect(five.key, 'pin.last_run.minutes_ago');
      expect(five.args, {'minutes': '5'});
      final edge = lastRunAgoLabel(const Duration(minutes: 59, seconds: 59));
      expect(edge.key, 'pin.last_run.minutes_ago');
      expect(edge.args, {'minutes': '59'});
    });

    test('하루_미만은_시간_단위로_버킷된다', () {
      final one = lastRunAgoLabel(const Duration(hours: 1));
      expect(one.key, 'pin.last_run.hours_ago');
      expect(one.args, {'hours': '1'});
      final edge = lastRunAgoLabel(const Duration(hours: 23, minutes: 59));
      expect(edge.key, 'pin.last_run.hours_ago');
      expect(edge.args, {'hours': '23'});
    });

    test('하루_이상은_일_단위로_버킷된다', () {
      final one = lastRunAgoLabel(const Duration(days: 1));
      expect(one.key, 'pin.last_run.days_ago');
      expect(one.args, {'days': '1'});
      final month = lastRunAgoLabel(const Duration(days: 30));
      expect(month.key, 'pin.last_run.days_ago');
      expect(month.args, {'days': '30'});
    });
  });
}
