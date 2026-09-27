'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { FIELD_TYPE, initialValue, scalarValue } = require('../form_values.js');

test('saved_values_win_over_schema_defaults_including_false_zero_and_empty', () => {
  const preset = Object.assign(Object.create(null), { enabled: false, count: 0, label: '' });
  assert.deepEqual(initialValue(preset, 'enabled', { default: true }), { hasValue: true, value: false });
  assert.deepEqual(initialValue(preset, 'count', { default: 7 }), { hasValue: true, value: 0 });
  assert.deepEqual(initialValue(preset, 'label', { default: 'default' }), { hasValue: true, value: '' });
  assert.deepEqual(initialValue({}, 'missing', { default: 'schema' }), { hasValue: true, value: 'schema' });
});

test('explicit_form_edits_win_over_saved_values_and_optional_empty_is_not_omitted', () => {
  assert.deepEqual(
    scalarValue({ fieldType: 'string', raw: '', checked: false, required: false, hasInitial: true, touched: true }),
    { ok: true, include: true, value: '' },
  );
  assert.deepEqual(
    scalarValue({ fieldType: FIELD_TYPE.BOOLEAN, raw: '', checked: false, required: false, hasInitial: true, touched: true }),
    { ok: true, include: true, value: false },
  );
  assert.deepEqual(
    scalarValue({ fieldType: FIELD_TYPE.NUMBER, raw: '0', checked: false, required: false, hasInitial: true, touched: true }),
    { ok: true, include: true, value: 0 },
  );
});

test('blank_numeric_with_a_saved_or_edited_value_is_rejected_never_coerced_to_zero', () => {
  for (const details of [
    { required: false, hasInitial: true, touched: true },
    { required: false, hasInitial: false, touched: true },
    { required: true, hasInitial: false, touched: false },
  ]) {
    assert.deepEqual(
      scalarValue({ fieldType: FIELD_TYPE.NUMBER, raw: '', checked: false, ...details }),
      { ok: false, reason: 'number_required' },
    );
  }
});

test('numeric_controls reject_infinity_and_fractional_integers', () => {
  assert.deepEqual(
    scalarValue({ fieldType: FIELD_TYPE.NUMBER, raw: '1e999', checked: false, required: false, hasInitial: false, touched: true }),
    { ok: false, reason: 'number_invalid' },
  );
  assert.deepEqual(
    scalarValue({ fieldType: FIELD_TYPE.INTEGER, raw: '1.5', checked: false, required: false, hasInitial: false, touched: true }),
    { ok: false, reason: 'number_invalid' },
  );
});

test('unmodified_optional_omitted_fields_stay_out_of_the_request', () => {
  assert.deepEqual(
    scalarValue({ fieldType: 'string', raw: '', checked: false, required: false, hasInitial: false, touched: false }),
    { ok: true, include: false, value: '' },
  );
});
