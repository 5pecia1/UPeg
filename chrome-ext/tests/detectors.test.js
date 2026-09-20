'use strict';

// Detector table (chrome-ext/detectors.js) — the in-page capability's whole
// decision layer. Pure functions, so no DOM and no chrome.* here.

const assert = require('node:assert/strict');
const test = require('node:test');

const {
  DETECTORS,
  DETECTOR_ID,
  EPOCH_MILLIS_DIGITS,
  EPOCH_SECONDS_DIGITS,
  detect,
  detectorFor,
  dispatchArgsFor,
  hasDetection,
  labelKeyFor,
  previewFor,
  toolIdFor,
} = require('../detectors.js');

const detectionIds = (text) => detect(text).map((d) => d.detectorId);

test('the_detector_table_gives_every_row_the_same_data_shape', () => {
  assert.ok(DETECTORS.length >= 3, 'the table must actually carry rows');
  const seen = new Set();
  for (const detector of DETECTORS) {
    assert.equal(seen.has(detector.id), false, `duplicate detector id ${detector.id}`);
    seen.add(detector.id);
    assert.match(detector.labelKey, /^detectorLabel/);
    assert.equal(detector.pattern.flags.includes('g'), true, 'patterns must be global');
    assert.equal(typeof detector.normalize, 'function');
    assert.ok(detector.preview === null || typeof detector.preview === 'function');
    if (detector.tool !== null) {
      assert.match(detector.tool.id, /^[a-z_]+\.[a-z0-9_]+$/);
      assert.equal(typeof detector.tool.arg, 'string');
      assert.ok(detector.tool.arg.length > 0, 'a tool row must name the argument');
    }
    assert.equal(Object.isFrozen(detector), true, 'rows must be data, not mutable state');
  }
});

test('hex_tokens_are_detected_as_hex_rows_with_a_decimal_preview', () => {
  const [detection] = detect('gas used 0xff today');
  assert.equal(detection.detectorId, DETECTOR_ID.HEX);
  assert.equal(detection.raw, '0xff');
  assert.equal(detection.value, '0xff');
  assert.equal(previewFor(detection), '255');
  assert.equal(toolIdFor(detection), 'num.hex_to_decimal');
  assert.deepEqual(dispatchArgsFor(detection), { input: '0xff' });
  assert.equal(labelKeyFor(detection), 'detectorLabelHex');
});

test('a_40_digit_ethereum_address_becomes_decimal_without_bigint_truncation', () => {
  const address = `0x${'d'.repeat(40)}`;
  const [detection] = detect(address);
  assert.equal(detection.raw, address);
  assert.equal(previewFor(detection), BigInt(address).toString());
});

test('epoch_seconds_and_millis_preview_as_utc_iso_strings', () => {
  const seconds = detect('at 1699999999 utc')[0];
  assert.equal(seconds.detectorId, DETECTOR_ID.EPOCH);
  assert.equal(seconds.value.length, EPOCH_SECONDS_DIGITS);
  assert.equal(previewFor(seconds), '2023-11-14T22:13:19.000Z');

  const millis = detect('at 1699999999123 utc')[0];
  assert.equal(millis.value.length, EPOCH_MILLIS_DIGITS);
  assert.equal(previewFor(millis), '2023-11-14T22:13:19.123Z');
});

test('the_epoch_row_has_no_host_tool_so_it_builds_no_dispatch_args', () => {
  // `time.epoch_now`/`time.iso_now` take no epoch input and no other tool
  // converts one, so this row is preview-only by design — content.js must
  // not be able to build a dead deep link out of it.
  const [detection] = detect('1699999999');
  assert.equal(toolIdFor(detection), null);
  assert.equal(dispatchArgsFor(detection), null);
  assert.notEqual(previewFor(detection), null);
});

test('a_base64_blob_routes_to_convert_base64_decode_with_no_offline_preview', () => {
  const [detection] = detect('payload SGVsbG8sIHdvcmxkIQ== end');
  assert.equal(detection.detectorId, DETECTOR_ID.BASE64);
  assert.equal(detection.value, 'SGVsbG8sIHdvcmxkIQ==');
  assert.equal(toolIdFor(detection), 'convert.base64_decode');
  assert.deepEqual(dispatchArgsFor(detection), { input: 'SGVsbG8sIHdvcmxkIQ==' });
  assert.equal(previewFor(detection), null);
});

test('the_base64_row_ignores_strings_that_read_as_hashes_or_addresses', () => {
  // Pure hex (a digest), a non-multiple-of-4 run, and anything under the
  // minimum length are all more likely to be something else.
  for (const text of [
    'a'.repeat(32),
    'DEADBEEFDEADBEEFDEADBEEF',
    'SGVsbG8sIHdvcmxkIQ=',
    'SGVsbG8=',
  ]) {
    assert.equal(detectionIds(text).includes(DETECTOR_ID.BASE64), false, text);
  }
});

test('overlapping_candidates_resolve_by_row_priority_and_results_do_not_overlap', () => {
  // `0x...` is a hex token even though its tail is also a legal base64
  // alphabet run of the right length.
  const hexish = '0xdeadbeefdeadbeefdeadbeef';
  assert.deepEqual(detectionIds(hexish), [DETECTOR_ID.HEX]);

  const mixed = 'tx 0xff at 1699999999 blob SGVsbG8sIHdvcmxkIQ==';
  const detections = detect(mixed);
  assert.deepEqual(
    detections.map((d) => d.detectorId),
    [DETECTOR_ID.HEX, DETECTOR_ID.EPOCH, DETECTOR_ID.BASE64],
  );
  for (let i = 1; i < detections.length; i += 1) {
    const previousEnd = detections[i - 1].index + detections[i - 1].length;
    assert.ok(detections[i].index >= previousEnd, 'detections must not overlap');
  }
});

test('detection_index_and_length_point_into_the_source_text_verbatim', () => {
  const text = 'gas 0xff used at 1699999999';
  for (const detection of detect(text)) {
    assert.equal(text.slice(detection.index, detection.index + detection.length), detection.raw);
  }
});

test('repeated_scans_leave_no_global_regex_state_behind', () => {
  // Each scan clones its row's pattern, so `lastIndex` never leaks between
  // calls — the failure mode the single-detector content script had.
  const text = 'a 0xff b 0xaa c';
  const first = detect(text);
  for (let i = 0; i < 5; i += 1) {
    assert.deepEqual(detect(text), first);
  }
  assert.equal(hasDetection(text), true);
  assert.equal(hasDetection('nothing here'), false);
  assert.equal(hasDetection(null), false);
});

test('an_unknown_detector_id_returns_null', () => {
  assert.equal(detectorFor('no-such-detector'), null);
  assert.equal(toolIdFor({ detectorId: 'no-such-detector', value: 'x' }), null);
  assert.equal(dispatchArgsFor({ detectorId: 'no-such-detector', value: 'x' }), null);
  assert.equal(previewFor({ detectorId: 'no-such-detector', value: 'x' }), null);
  assert.equal(labelKeyFor({ detectorId: 'no-such-detector', value: 'x' }), null);
});
