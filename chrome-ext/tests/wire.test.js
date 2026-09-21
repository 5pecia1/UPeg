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

test('base64_round_trips_exact_bytes_and_empty', () => {
  assert.equal(encodeBase64(Uint8Array.from([0, 1, 255])), 'AAH/');
  assert.equal(encodeBase64(new Uint8Array()), '');
  assert.deepEqual(decodeBase64('AAH/'), Uint8Array.from([0, 1, 255]));
  assert.deepEqual(decodeBase64(''), new Uint8Array());
});

test('the_base64_decoder_rejects_noncanonical_wire_and_legacy_numeric_arrays', () => {
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

test('the_base64_decoder_handles_a_16mib_canonical_payload_without_stack_overflow', () => {
  const rawBytes = 16 * 1024 * 1024;
  const encoded = Buffer.alloc(rawBytes).toString('base64');

  const decoded = decodeBase64(encoded);

  assert.equal(decoded.byteLength, rawBytes);
  assert.equal(decoded[0], 0);
  assert.equal(decoded[decoded.byteLength - 1], 0);
});

test('the_base64_decoder_rejects_over_64mib_with_a_typed_error_before_allocating', () => {
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

test('the_file_output_cap_error_is_surfaced_in_every_popup_locale', () => {
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

test('the_serialized_request_body_stays_under_the_server_cap', () => {
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

test('serialization_emits_only_base64_strings_never_numeric_arrays', () => {
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

test('the_worst_case_filevalue_wire_stays_under_the_server_body_cap', () => {
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
