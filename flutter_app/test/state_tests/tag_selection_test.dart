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
    test('TagAll_instances_compare_equal_like_const_singletons', () {
      expect(const TagAll(), const TagAll());
      expect(const TagAll().hashCode, const TagAll().hashCode);
    });

    test('TagSpecific_instances_with_equal_tag_compare_equal', () {
      expect(const TagSpecific('convert'), const TagSpecific('convert'));
      expect(const TagSpecific('convert') == const TagSpecific('id'), isFalse);
    });

    test('TagAll_and_TagSpecific_are_never_equal', () {
      expect(const TagAll() == const TagSpecific('all'), isFalse);
      expect(const TagSpecific('all') == const TagAll(), isFalse);
    });

    test('TagAll_frbValue_is_the_all_string', () {
      expect(const TagAll().frbValue, 'all');
    });

    test('TagSpecific_frbValue_is_the_wrapped_tag', () {
      expect(const TagSpecific('convert').frbValue, 'convert');
    });

    test('TagAll_persistValue_is_null', () {
      expect(const TagAll().persistValue, isNull);
    });

    test('TagSpecific_persistValue_is_the_wrapped_tag', () {
      expect(const TagSpecific('convert').persistValue, 'convert');
    });

    test('fromPersisted_restores_null_as_TagAll', () {
      expect(TagSelection.fromPersisted(null), const TagAll());
    });

    test('fromPersisted_restores_string_as_TagSpecific', () {
      expect(
        TagSelection.fromPersisted('convert'),
        const TagSpecific('convert'),
      );
    });

    test('fromPersisted_round_trip_is_inverse_of_persistValue', () {
      const samples = [TagAll(), TagSpecific('convert'), TagSpecific('id')];
      for (final s in samples) {
        expect(TagSelection.fromPersisted(s.persistValue), s);
      }
    });

    test('TagAll_and_TagSpecific_named_all_are_distinguished_by_type', () {
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
