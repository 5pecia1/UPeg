/// Unit tests for [`TagSelection`] sealed class.
///
/// The sealed class replaces the legacy `const kAllTag = 'all'`
/// magic string. These tests pin the variant equality and the
/// persistence round-trip.
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:upeg/src/state/tag_selection.dart';

void main() {
  group('TagSelection sealed (W1)', () {
    test('TagAll은_const_싱글톤처럼_같다', () {
      expect(const TagAll(), const TagAll());
      expect(const TagAll().hashCode, const TagAll().hashCode);
    });

    test('TagSpecific은_tag가_같으면_같다', () {
      expect(const TagSpecific('convert'), const TagSpecific('convert'));
      expect(const TagSpecific('convert') == const TagSpecific('id'), isFalse);
    });

    test('TagAll과_TagSpecific은_서로_다르다', () {
      expect(const TagAll() == const TagSpecific('all'), isFalse);
      expect(const TagSpecific('all') == const TagAll(), isFalse);
    });

    test('TagAll_frbValue는_all_문자열이다', () {
      expect(const TagAll().frbValue, 'all');
    });

    test('TagSpecific_frbValue는_wrapped_tag이다', () {
      expect(const TagSpecific('convert').frbValue, 'convert');
    });

    test('TagAll_persistValue는_null이다', () {
      expect(const TagAll().persistValue, isNull);
    });

    test('TagSpecific_persistValue는_wrapped_tag이다', () {
      expect(const TagSpecific('convert').persistValue, 'convert');
    });

    test('fromPersisted는_null을_TagAll로_복원한다', () {
      expect(TagSelection.fromPersisted(null), const TagAll());
    });

    test('fromPersisted는_문자열을_TagSpecific으로_복원한다', () {
      expect(
        TagSelection.fromPersisted('convert'),
        const TagSpecific('convert'),
      );
    });

    test('fromPersisted_round_trip은_persistValue와_역함수다', () {
      const samples = [TagAll(), TagSpecific('convert'), TagSpecific('id')];
      for (final s in samples) {
        expect(TagSelection.fromPersisted(s.persistValue), s);
      }
    });

    test('TagAll과_TagSpecific_named_all은_타입으로_구별된다', () {
      // The whole point of the sealed class: a tool literally named
      // "all" must NOT collide with the "no filter" selection.
      const all = TagAll();
      const specificAll = TagSpecific('all');
      expect(all == specificAll, isFalse);
      expect(specificAll is TagAll, isFalse);
      expect(all is TagSpecific, isFalse);
    });
  });
}
