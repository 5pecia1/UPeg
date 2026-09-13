'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const {
  BASE64_WIRE_ERROR,
  MAX_EXT_RAW_AGGREGATE_BYTES,
  MAX_FILE_OUTPUT_RAW_BYTES,
  MAX_FILE_POLICY_COUNT,
  MAX_FILE_VALUE_METADATA_BYTES,
  MAX_REQUEST_BODY_BYTES,
  Base64WireError,
  RequestBodyTooLargeError,
  decodeBase64,
  encodeBase64,
  serializeJsonRequestBody,
} = require('../file_input.js');

test('base64는_exact_bytes와_empty를_round_trip한다', () => {
  assert.equal(encodeBase64(Uint8Array.from([0, 1, 255])), 'AAH/');
  assert.equal(encodeBase64(new Uint8Array()), '');
  assert.deepEqual(decodeBase64('AAH/'), Uint8Array.from([0, 1, 255]));
  assert.deepEqual(decodeBase64(''), new Uint8Array());
});

test('base64_decoder는_비정규_wire와_legacy_numeric_array를_거부한다', () => {
  const rejected = [
    [0, 1, 255],
    'AAH_',
    'AA H/',
    'AAH',
    'AAH/=',
    'AB==',
  ];

  for (const value of rejected) {
    assert.throws(() => decodeBase64(value));
  }
});

test('base64_decoder는_16MiB_canonical_payload를_stack_overflow없이_해독한다', () => {
  const rawBytes = 16 * 1024 * 1024;
  const encoded = Buffer.alloc(rawBytes).toString('base64');

  const decoded = decodeBase64(encoded);

  assert.equal(decoded.byteLength, rawBytes);
  assert.equal(decoded[0], 0);
  assert.equal(decoded[decoded.byteLength - 1], 0);
});

test('base64_decoder는_64MiB_output_cap_초과를_allocation_전에_typed_error로_거부한다', () => {
  const rawBytes = MAX_FILE_OUTPUT_RAW_BYTES + 1;
  const encodedLength = Math.ceil(rawBytes / 3) * 4;
  const encoded = `${'A'.repeat(encodedLength - 1)}=`;

  assert.throws(
    () => decodeBase64(encoded),
    (error) =>
      error instanceof Base64WireError &&
      error.code === BASE64_WIRE_ERROR.TOO_LARGE &&
      error.actualBytes === rawBytes &&
      error.limitBytes === MAX_FILE_OUTPUT_RAW_BYTES,
  );
});

test('File_output_cap_오류는_popup에서_모든_locale로_안내한다', () => {
  const extensionRoot = path.join(__dirname, '..');
  const popup = fs.readFileSync(path.join(extensionRoot, 'popup.js'), 'utf8');
  const en = JSON.parse(
    fs.readFileSync(path.join(extensionRoot, '_locales/en/messages.json'), 'utf8'),
  );
  const ko = JSON.parse(
    fs.readFileSync(path.join(extensionRoot, '_locales/ko/messages.json'), 'utf8'),
  );

  assert.match(popup, /BASE64_WIRE_ERROR\.TOO_LARGE/);
  assert.match(popup, /i18nMessage\('fileOutputTooLarge'/);
  assert.ok(en.fileOutputTooLarge.message.length > 0);
  assert.ok(ko.fileOutputTooLarge.message.length > 0);
});

test('직렬화된_요청_body는_server_cap보다_작아야_한다', () => {
  const emptyBodyBytes = new TextEncoder().encode(JSON.stringify({ value: '' })).byteLength;
  const allowedValue = 'a'.repeat(MAX_REQUEST_BODY_BYTES - emptyBodyBytes - 1);
  const rejectedValue = `${allowedValue}a`;

  const body = serializeJsonRequestBody({ value: allowedValue });

  assert.equal(new TextEncoder().encode(body).byteLength, MAX_REQUEST_BODY_BYTES - 1);
  assert.throws(
    () => serializeJsonRequestBody({ value: rejectedValue }),
    (error) =>
      error instanceof RequestBodyTooLargeError &&
      error.actualBytes === MAX_REQUEST_BODY_BYTES &&
      error.limitBytes === MAX_REQUEST_BODY_BYTES,
  );
});

test('numeric_array를_만들지_않고_base64_string만_직렬화한다', () => {
  const body = serializeJsonRequestBody({
    file: {
      name: 'bytes.bin',
      content: { kind: 'bytes', bytes: encodeBase64(Uint8Array.from([0, 1, 255])) },
    },
  });
  const parsed = JSON.parse(body);

  assert.equal(parsed.file.content.bytes, 'AAH/');
  assert.equal(Array.isArray(parsed.file.content.bytes), false);
});

test('최악의_FileValue_wire도_server_body_cap_아래에_머문다', () => {
  const rawBytes = encodeBase64(new Uint8Array(MAX_EXT_RAW_AGGREGATE_BYTES));
  const escapedMetadata = '\0'.repeat(MAX_FILE_VALUE_METADATA_BYTES);
  const metadataPerEntry = Math.floor(
    MAX_FILE_VALUE_METADATA_BYTES / MAX_FILE_POLICY_COUNT,
  );
  const entries = Array.from({ length: MAX_FILE_POLICY_COUNT }, (_, index) => ({
    name: escapedMetadata.slice(
      index * metadataPerEntry,
      (index + 1) * metadataPerEntry,
    ),
    is_dir: false,
    content: { kind: 'bytes', bytes: index === 0 ? rawBytes : '' },
  }));
  entries[entries.length - 1].name += escapedMetadata.slice(
    metadataPerEntry * MAX_FILE_POLICY_COUNT,
  );

  const body = serializeJsonRequestBody({
    file: {
      name: '',
      is_dir: true,
      content: { kind: 'directory', entries },
    },
    companion: '\\\"\n',
  });

  assert.ok(new TextEncoder().encode(body).byteLength < MAX_REQUEST_BODY_BYTES);
});
