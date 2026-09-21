/// Unit tests for the pure "last run · N ago" bucketing formatter.
library;

import 'package:flutter_test/flutter_test.dart';

import 'package:upeg/src/i18n/relative_time.dart';

void main() {
  group('lastRunAgoLabel', () {
    test('under_a_minute_buckets_as_just_now', () {
      final label = lastRunAgoLabel(Duration.zero);
      expect(label.key, 'pin.last_run.just_now');
      expect(label.args, isEmpty);
      expect(
        lastRunAgoLabel(const Duration(seconds: 59)).key,
        'pin.last_run.just_now',
      );
    });

    test(
      'negative_elapsed_time_clamps_to_just_now_instead_of_predicting_future',
      () {
        expect(
          lastRunAgoLabel(const Duration(minutes: -5)).key,
          'pin.last_run.just_now',
        );
      },
    );

    test('under_an_hour_buckets_in_minutes', () {
      final five = lastRunAgoLabel(const Duration(minutes: 5));
      expect(five.key, 'pin.last_run.minutes_ago');
      expect(five.args, {'minutes': '5'});
      final edge = lastRunAgoLabel(const Duration(minutes: 59, seconds: 59));
      expect(edge.key, 'pin.last_run.minutes_ago');
      expect(edge.args, {'minutes': '59'});
    });

    test('under_a_day_buckets_in_hours', () {
      final one = lastRunAgoLabel(const Duration(hours: 1));
      expect(one.key, 'pin.last_run.hours_ago');
      expect(one.args, {'hours': '1'});
      final edge = lastRunAgoLabel(const Duration(hours: 23, minutes: 59));
      expect(edge.key, 'pin.last_run.hours_ago');
      expect(edge.args, {'hours': '23'});
    });

    test('a_day_or_more_buckets_in_days', () {
      final one = lastRunAgoLabel(const Duration(days: 1));
      expect(one.key, 'pin.last_run.days_ago');
      expect(one.args, {'days': '1'});
      final month = lastRunAgoLabel(const Duration(days: 30));
      expect(month.key, 'pin.last_run.days_ago');
      expect(month.args, {'days': '30'});
    });
  });
}
