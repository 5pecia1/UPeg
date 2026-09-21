'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const {
  FILE_SELECTION_ERROR,
  MAX_EXT_RAW_AGGREGATE_BYTES,
  MAX_FILE_POLICY_COUNT,
  MAX_FILE_VALUE_METADATA_BYTES,
  MAX_FILE_VALUE_NODE_COUNT,
  configureFileInput,
  readFileSelection,
} = require('../file_input.js');

function unreadableSizedFile(name, size, type = '') {
  return {
    name,
    type,
    size,
    slice() {
      assert.fail('every resource limit must apply before a file is read');
    },
  };
}

function emptyFile(name, type = '') {
  return {
    name,
    type,
    size: 0,
    slice() {
      return {
        async arrayBuffer() {
          return new ArrayBuffer(0);
        },
      };
    },
  };
}

test('policy_file_count_is_capped_at_100_and_generated_nodes_stay_under_128', async () => {
  const policy = configureFileInput({}, {
    'x-upeg-file-policy': { maxCount: MAX_FILE_POLICY_COUNT + 1 },
  });
  const allowed = Array.from(
    { length: MAX_FILE_POLICY_COUNT },
    (_, index) => emptyFile(`${index}.bin`),
  );
  const rejected = [
    ...allowed.map((file) => unreadableSizedFile(file.name, file.size)),
    unreadableSizedFile('overflow.bin', 0),
  ];

  const value = await readFileSelection(allowed, policy, 'files');

  assert.equal(policy.maxCount, MAX_FILE_POLICY_COUNT);
  assert.equal(value.content.entries.length + 1, MAX_FILE_POLICY_COUNT + 1);
  assert.ok(value.content.entries.length + 1 <= MAX_FILE_VALUE_NODE_COUNT);
  await assert.rejects(
    readFileSelection(rejected, policy, 'files'),
    { code: FILE_SELECTION_ERROR.TOO_MANY_FILES },
  );
});

test('utf8_metadata_is_allowed_up_to_the_16kib_boundary', async () => {
  const threeByteCharacters = Math.floor(MAX_FILE_VALUE_METADATA_BYTES / 3);
  const exactName =
    '가'.repeat(threeByteCharacters) +
    'a'.repeat(MAX_FILE_VALUE_METADATA_BYTES - threeByteCharacters * 3);
  const exact = emptyFile(exactName);

  await assert.doesNotReject(
    readFileSelection(
      [exact],
      { maxCount: 1, extensions: [], maxFileBytes: null, maxTotalBytes: null },
      'file',
    ),
  );
});

test('utf8_metadata_over_16kib_is_rejected_before_reading', async () => {
  const oversized = unreadableSizedFile('a'.repeat(MAX_FILE_VALUE_METADATA_BYTES + 1), 0);

  await assert.rejects(
    readFileSelection(
      [oversized],
      { maxCount: 1, extensions: [], maxFileBytes: null, maxTotalBytes: null },
      'file',
    ),
    { code: FILE_SELECTION_ERROR.METADATA_TOO_LARGE },
  );
});

test('the_ext_raw_total_allows_up_to_640kib_and_rejects_over_before_reading', async () => {
  const exact = {
    name: 'exact.bin',
    type: '',
    size: MAX_EXT_RAW_AGGREGATE_BYTES,
    slice(start, end) {
      const length = Math.min(end, this.size) - start;
      return {
        async arrayBuffer() {
          return new ArrayBuffer(length);
        },
      };
    },
  };
  const oversized = unreadableSizedFile('oversized.bin', MAX_EXT_RAW_AGGREGATE_BYTES + 1);
  const policy = { maxCount: 1, extensions: [], maxFileBytes: null, maxTotalBytes: null };

  await assert.doesNotReject(readFileSelection([exact], policy, 'file'));
  await assert.rejects(
    readFileSelection([oversized], policy, 'file'),
    { code: FILE_SELECTION_ERROR.TRANSPORT_TOO_LARGE },
  );
});
