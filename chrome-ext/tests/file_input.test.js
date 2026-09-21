'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const {
  FILE_POLICY_CONTROL_PROPERTY,
  FILE_SELECTION_ERROR,
  MAX_EXT_RAW_AGGREGATE_BYTES,
  configureFileInput,
  decodeBase64,
  encodeBase64,
  readFileSelection,
  serializeJsonRequestBody,
} = require('../file_input.js');

const EXTENSION_ROOT = path.join(__dirname, '..');

function fakeFile(name, bytes, type = '') {
  const source = new Uint8Array(bytes);
  const slices = [];
  return {
    name,
    type,
    size: source.byteLength,
    slices,
    slice(start, end) {
      slices.push([start, end]);
      const selected = source.slice(start, end);
      return {
        async arrayBuffer() {
          return selected.buffer;
        },
      };
    },
  };
}

function unreadableSizedFile(name, size) {
  return {
    name,
    type: 'image/png',
    size,
    slice() {
      assert.fail('preflight rejection must happen before reading any file');
    },
  };
}

test('the_file_wire_helper_loads_before_popup_and_is_staged_into_build', () => {
  const html = fs.readFileSync(path.join(EXTENSION_ROOT, 'popup.html'), 'utf8');
  const build = fs.readFileSync(path.join(EXTENSION_ROOT, 'build.sh'), 'utf8');
  const wireIndex = html.indexOf('<script src="wire.js"></script>');
  const helperIndex = html.indexOf('<script src="file_input.js"></script>');
  const popupIndex = html.indexOf('<script src="popup.js"></script>');

  assert.notEqual(wireIndex, -1);
  assert.notEqual(helperIndex, -1);
  assert.ok(wireIndex < helperIndex);
  assert.ok(helperIndex < popupIndex);
  assert.match(build, /FILES=\([^)]*wire\.js[^)]*file_input\.js[^)]*popup\.js/);
});

test('file_policy_sets_the_multiple_and_accept_attributes', () => {
  const control = { multiple: false, accept: '' };
  const schema = {
    'x-upeg-file-policy': {
      maxCount: 100,
      extensions: ['png', 'jpg', 'jpeg'],
      maxFileBytes: 32,
      maxTotalBytes: 64,
    },
  };

  const policy = configureFileInput(control, schema);

  assert.equal(control.multiple, true);
  assert.equal(control.accept, '.png,.jpg,.jpeg');
  assert.deepEqual(control[FILE_POLICY_CONTROL_PROPERTY], policy);
  assert.deepEqual(policy, {
    maxCount: 100,
    extensions: ['png', 'jpg', 'jpeg'],
    maxFileBytes: 32,
    maxTotalBytes: 64,
  });
});

test('compound_extensions_and_dotfiles_pass_via_normalized_filename_suffixes', async () => {
  const policy = configureFileInput({}, {
    'x-upeg-file-policy': {
      maxCount: 2,
      extensions: ['tar.gz', '.env'],
    },
  });

  const value = await readFileSelection(
    [fakeFile('archive.TAR.GZ', []), fakeFile('.env', [])],
    policy,
    'files',
  );

  assert.deepEqual(
    value.content.entries.map((entry) => entry.name),
    ['archive.TAR.GZ', '.env'],
  );
  await assert.rejects(
    readFileSelection([unreadableSizedFile('archive.gz', 0)], policy, 'files'),
    { code: FILE_SELECTION_ERROR.EXTENSION_NOT_ALLOWED },
  );
});

test('a_per_file_0_byte_cap_admits_only_empty_files', async () => {
  const policy = configureFileInput({}, {
    'x-upeg-file-policy': { maxFileBytes: 0 },
  });

  await assert.rejects(
    readFileSelection([fakeFile('data.bin', [1])], policy, 'input'),
    { code: FILE_SELECTION_ERROR.FILE_TOO_LARGE },
  );
  await assert.doesNotReject(
    readFileSelection([fakeFile('empty.bin', [])], policy, 'input'),
  );
});

test('a_total_0_byte_cap_admits_only_0_byte_file_selections', async () => {
  const policy = configureFileInput({}, {
    'x-upeg-file-policy': { maxCount: 2, maxTotalBytes: 0 },
  });

  await assert.rejects(
    readFileSelection([fakeFile('empty.bin', []), fakeFile('data.bin', [1])], policy, 'input'),
    { code: FILE_SELECTION_ERROR.TOTAL_TOO_LARGE },
  );
  await assert.doesNotReject(
    readFileSelection(
      [fakeFile('first.bin', []), fakeFile('second.bin', [])],
      policy,
      'input',
    ),
  );
});

test('multi_policy_builds_a_directory_preserving_pick_order_and_mime', async () => {
  const first = fakeFile('첫째.PNG', [1, 2], 'image/png');
  const second = fakeFile('second.jpg', [3], 'image/jpeg');
  const policy = {
    maxCount: 3,
    extensions: ['png', 'jpg'],
    maxFileBytes: 10,
    maxTotalBytes: 10,
  };

  const one = await readFileSelection([first], policy, 'images');
  const two = await readFileSelection([first, second], policy, 'images');

  assert.deepEqual(one, {
    name: 'images',
    is_dir: true,
    content: {
      kind: 'directory',
      entries: [
        {
          name: '첫째.PNG',
          is_dir: false,
          mime: 'image/png',
          content: { kind: 'bytes', bytes: 'AQI=' },
        },
      ],
    },
  });
  assert.deepEqual(
    two.content.entries.map((entry) => [entry.name, entry.mime, entry.content.bytes]),
    [
      ['첫째.PNG', 'image/png', 'AQI='],
      ['second.jpg', 'image/jpeg', 'Aw=='],
    ],
  );
});

test('single_policy_keeps_the_existing_bytes_root_shape', async () => {
  const selected = fakeFile('report.bin', [7, 8], 'application/octet-stream');

  const value = await readFileSelection(
    [selected],
    {
      maxCount: 1,
      extensions: [],
      maxFileBytes: null,
      maxTotalBytes: null,
    },
    'input',
  );

  assert.deepEqual(value, {
    name: 'report.bin',
    is_dir: false,
    mime: 'application/octet-stream',
    content: { kind: 'bytes', bytes: 'Bwg=' },
  });
});

test('count_extension_size_total_and_ext_transport_caps_reject_before_reading', async (t) => {
  const cases = [
    {
      name: 'count',
      files: [unreadableSizedFile('a.png', 1), unreadableSizedFile('b.png', 1)],
      policy: { maxCount: 1, extensions: ['png'], maxFileBytes: 10, maxTotalBytes: 10 },
      code: FILE_SELECTION_ERROR.TOO_MANY_FILES,
    },
    {
      name: 'extension',
      files: [unreadableSizedFile('a.gif', 1)],
      policy: { maxCount: 2, extensions: ['png'], maxFileBytes: 10, maxTotalBytes: 10 },
      code: FILE_SELECTION_ERROR.EXTENSION_NOT_ALLOWED,
    },
    {
      name: 'file size',
      files: [unreadableSizedFile('a.png', 11)],
      policy: { maxCount: 2, extensions: ['png'], maxFileBytes: 10, maxTotalBytes: 20 },
      code: FILE_SELECTION_ERROR.FILE_TOO_LARGE,
    },
    {
      name: 'policy total',
      files: [unreadableSizedFile('a.png', 6), unreadableSizedFile('b.png', 5)],
      policy: { maxCount: 2, extensions: ['png'], maxFileBytes: 10, maxTotalBytes: 10 },
      code: FILE_SELECTION_ERROR.TOTAL_TOO_LARGE,
    },
    {
      name: 'Ext transport cap',
      files: [
        unreadableSizedFile('a.png', MAX_EXT_RAW_AGGREGATE_BYTES),
        unreadableSizedFile('b.png', 1),
      ],
      policy: { maxCount: 2, extensions: ['png'], maxFileBytes: null, maxTotalBytes: null },
      code: FILE_SELECTION_ERROR.TRANSPORT_TOO_LARGE,
    },
  ];

  for (const scenario of cases) {
    await t.test(scenario.name, async () => {
      await assert.rejects(
        readFileSelection(scenario.files, scenario.policy, 'images'),
        (error) => error.code === scenario.code && error.message.length > 0,
      );
    });
  }
});

test('each_file_is_sliced_to_read_one_byte_past_the_remaining_cap', async () => {
  const selected = fakeFile('small.png', [1, 2, 3], 'image/png');

  await readFileSelection(
    [selected],
    {
      maxCount: 2,
      extensions: ['png'],
      maxFileBytes: 4,
      maxTotalBytes: 4,
    },
    'images',
  );

  assert.deepEqual(selected.slices, [[0, 5]]);
});

test('the_bytes_wire_uses_exactly_padded_rfc4648_base64', () => {
  assert.equal(encodeBase64(Uint8Array.from([0, 1, 255])), 'AAH/');
  assert.equal(encodeBase64(new Uint8Array()), '');
  assert.deepEqual(decodeBase64('AAH/'), Uint8Array.from([0, 1, 255]));
  assert.deepEqual(decodeBase64(''), new Uint8Array());
});

test('the_request_body_serialization_helper_exists', () => {
  assert.equal(typeof serializeJsonRequestBody, 'function');
});
