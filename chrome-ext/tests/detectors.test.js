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

test('감지기_표는_각_행마다_같은_모양의_데이터를_가진다', () => {
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

test('hex_토큰은_hex_행으로_감지되고_십진수_미리보기를_가진다', () => {
  const [detection] = detect('gas used 0xff today');
  assert.equal(detection.detectorId, DETECTOR_ID.HEX);
  assert.equal(detection.raw, '0xff');
  assert.equal(detection.value, '0xff');
  assert.equal(previewFor(detection), '255');
  assert.equal(toolIdFor(detection), 'num.hex_to_decimal');
  assert.deepEqual(dispatchArgsFor(detection), { input: '0xff' });
  assert.equal(labelKeyFor(detection), 'detectorLabelHex');
});

test('40자_이더리움_주소는_bigint로_잘리지_않고_십진수가_된다', () => {
  const address = `0x${'d'.repeat(40)}`;
  const [detection] = detect(address);
  assert.equal(detection.raw, address);
  assert.equal(previewFor(detection), BigInt(address).toString());
});

test('epoch_초와_밀리초는_utc_iso_문자열로_미리보기된다', () => {
  const seconds = detect('at 1699999999 utc')[0];
  assert.equal(seconds.detectorId, DETECTOR_ID.EPOCH);
  assert.equal(seconds.value.length, EPOCH_SECONDS_DIGITS);
  assert.equal(previewFor(seconds), '2023-11-14T22:13:19.000Z');

  const millis = detect('at 1699999999123 utc')[0];
  assert.equal(millis.value.length, EPOCH_MILLIS_DIGITS);
  assert.equal(previewFor(millis), '2023-11-14T22:13:19.123Z');
});

test('epoch_행은_호스트_도구가_없으므로_디스패치_인자를_만들지_않는다', () => {
  // `time.epoch_now`/`time.iso_now` take no epoch input and no other tool
  // converts one, so this row is preview-only by design — content.js must
  // not be able to build a dead deep link out of it.
  const [detection] = detect('1699999999');
  assert.equal(toolIdFor(detection), null);
  assert.equal(dispatchArgsFor(detection), null);
  assert.notEqual(previewFor(detection), null);
});

test('base64_blob은_convert_base64_decode로_라우팅되고_오프라인_미리보기가_없다', () => {
  const [detection] = detect('payload SGVsbG8sIHdvcmxkIQ== end');
  assert.equal(detection.detectorId, DETECTOR_ID.BASE64);
  assert.equal(detection.value, 'SGVsbG8sIHdvcmxkIQ==');
  assert.equal(toolIdFor(detection), 'convert.base64_decode');
  assert.deepEqual(dispatchArgsFor(detection), { input: 'SGVsbG8sIHdvcmxkIQ==' });
  assert.equal(previewFor(detection), null);
});

test('base64_행은_해시나_주소로_읽히는_문자열을_감지하지_않는다', () => {
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

test('겹치는_후보는_행_우선순위로_결정되고_결과는_겹치지_않는다', () => {
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

test('감지_결과의_index와_length는_원문을_그대로_가리킨다', () => {
  const text = 'gas 0xff used at 1699999999';
  for (const detection of detect(text)) {
    assert.equal(text.slice(detection.index, detection.index + detection.length), detection.raw);
  }
});

test('반복_스캔은_전역_regex_상태를_남기지_않는다', () => {
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

test('알_수_없는_감지기_id는_null을_돌려준다', () => {
  assert.equal(detectorFor('no-such-detector'), null);
  assert.equal(toolIdFor({ detectorId: 'no-such-detector', value: 'x' }), null);
  assert.equal(dispatchArgsFor({ detectorId: 'no-such-detector', value: 'x' }), null);
  assert.equal(previewFor({ detectorId: 'no-such-detector', value: 'x' }), null);
  assert.equal(labelKeyFor({ detectorId: 'no-such-detector', value: 'x' }), null);
});
