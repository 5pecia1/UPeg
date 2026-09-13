'use strict';

const UpegFileWire =
  typeof module === 'object' && module.exports ? require('./wire.js') : UpegWire;

const UpegFileInput = (() => {
  const UPEG_FILE_POLICY_FIELD = 'x-upeg-file-policy';
  const FILE_POLICY_CONTROL_PROPERTY = 'upegFilePolicy';
  const BYTES_PER_KIBIBYTE = 1024;
  const DEFAULT_FILE_MAX_COUNT = 1;
  const READ_OVERFLOW_SENTINEL_BYTES = 1;
  // `MAX_FILE_POLICY_COUNT`, `MAX_FILE_VALUE_NODE_COUNT`, and
  // `MAX_FILE_VALUE_METADATA_BYTES` restate the canonical `File` budget
  // declared once in Rust at `upeg-core/src/input/file_budget.rs`. There is
  // no shared build-time source between JS and Rust, so these numbers are
  // pinned — not merely mirrored — by
  // `upeg-core/tests/file_budget_cross_language_pin.rs`, which reads this
  // file by path and fails the build if it drifts from Rust.
  // `MAX_EXT_RAW_AGGREGATE_BYTES` (below) has no Rust counterpart — it is an
  // extension-specific transport cap and is intentionally not pinned.
  const MAX_FILE_POLICY_COUNT = 100;
  const MAX_FILE_VALUE_NODE_COUNT = 128;
  const MAX_FILE_VALUE_METADATA_BYTES = 16 * BYTES_PER_KIBIBYTE;
  const MAX_EXT_RAW_AGGREGATE_BYTES = 640 * BYTES_PER_KIBIBYTE;
  const UTF8_ENCODER = new TextEncoder();
  const { encodeBase64 } = UpegFileWire;

  const FILE_SELECTION_ERROR = Object.freeze({
    EMPTY: 'empty',
    TOO_MANY_FILES: 'too_many_files',
    TOO_MANY_NODES: 'too_many_nodes',
    EXTENSION_NOT_ALLOWED: 'extension_not_allowed',
    FILE_TOO_LARGE: 'file_too_large',
    TOTAL_TOO_LARGE: 'total_too_large',
    TRANSPORT_TOO_LARGE: 'transport_too_large',
    METADATA_TOO_LARGE: 'metadata_too_large',
    READ_TOO_LARGE: 'read_too_large',
    READ_SIZE_MISMATCH: 'read_size_mismatch',
  });

  class FileSelectionError extends Error {
    constructor(code, details, message) {
      super(message);
      this.name = 'FileSelectionError';
      this.code = code;
      this.details = details;
    }
  }

  function positiveSafeInteger(value) {
    return Number.isSafeInteger(value) && value > 0 ? value : null;
  }

  function nonNegativeSafeInteger(value) {
    return Number.isSafeInteger(value) && value >= 0 ? value : null;
  }

  function normalizeExtensions(value) {
    if (!Array.isArray(value)) return [];
    const unique = new Set();
    for (const extension of value) {
      if (typeof extension !== 'string') continue;
      const normalized = extension.replace(/^\./, '').toLowerCase();
      if (normalized.length > 0) unique.add(normalized);
    }
    return Array.from(unique);
  }

  function parseFilePolicy(propertySchema) {
    const raw =
      propertySchema &&
      typeof propertySchema[UPEG_FILE_POLICY_FIELD] === 'object' &&
      propertySchema[UPEG_FILE_POLICY_FIELD] !== null
        ? propertySchema[UPEG_FILE_POLICY_FIELD]
        : {};
    const requestedMaxCount = positiveSafeInteger(raw.maxCount) || DEFAULT_FILE_MAX_COUNT;
    return {
      maxCount: Math.min(requestedMaxCount, MAX_FILE_POLICY_COUNT),
      extensions: normalizeExtensions(raw.extensions),
      maxFileBytes: nonNegativeSafeInteger(raw.maxFileBytes),
      maxTotalBytes: nonNegativeSafeInteger(raw.maxTotalBytes),
    };
  }

  function configureFileInput(control, propertySchema) {
    const policy = parseFilePolicy(propertySchema);
    control.multiple = policy.maxCount > DEFAULT_FILE_MAX_COUNT;
    control.accept = policy.extensions.map((extension) => `.${extension}`).join(',');
    control[FILE_POLICY_CONTROL_PROPERTY] = policy;
    return policy;
  }

  function fileHasAllowedExtension(name, allowedExtensions) {
    const normalizedName = name.toLowerCase();
    return allowedExtensions.some((extension) => normalizedName.endsWith(`.${extension}`));
  }

  function selectionError(code, details) {
    let message;
    switch (code) {
      case FILE_SELECTION_ERROR.EMPTY:
        message = 'Select at least one file.';
        break;
      case FILE_SELECTION_ERROR.TOO_MANY_FILES:
        message = `Selected ${details.actual} files; the limit is ${details.limit}.`;
        break;
      case FILE_SELECTION_ERROR.TOO_MANY_NODES:
        message = `The File value has ${details.actual} nodes; the limit is ${details.limit}.`;
        break;
      case FILE_SELECTION_ERROR.EXTENSION_NOT_ALLOWED:
        message = `"${details.fileName}" is not an allowed file type.`;
        break;
      case FILE_SELECTION_ERROR.FILE_TOO_LARGE:
        message = `"${details.fileName}" is ${details.actual} bytes; the per-file limit is ${details.limit} bytes.`;
        break;
      case FILE_SELECTION_ERROR.TOTAL_TOO_LARGE:
        message = `The selected files total ${details.actual} bytes; the policy limit is ${details.limit} bytes.`;
        break;
      case FILE_SELECTION_ERROR.TRANSPORT_TOO_LARGE:
        message = `The selected files total ${details.actual} bytes; the extension transport limit is ${details.limit} bytes.`;
        break;
      case FILE_SELECTION_ERROR.METADATA_TOO_LARGE:
        message = `File names and MIME types total ${details.actual} UTF-8 bytes; the limit is ${details.limit} bytes.`;
        break;
      case FILE_SELECTION_ERROR.READ_TOO_LARGE:
        message = `"${details.fileName}" exceeded the ${details.limit}-byte limit while being read.`;
        break;
      default:
        message = `"${details.fileName}" changed size while being read.`;
        break;
    }
    return new FileSelectionError(code, details, message);
  }

  function preflightFileSelection(fileList, policy, rootName = '') {
    const files = [...(fileList || [])];
    if (files.length === 0) {
      throw selectionError(FILE_SELECTION_ERROR.EMPTY, {});
    }
    const maxCount = Math.min(policy.maxCount, MAX_FILE_POLICY_COUNT);
    if (files.length > maxCount) {
      throw selectionError(FILE_SELECTION_ERROR.TOO_MANY_FILES, {
        actual: files.length,
        limit: maxCount,
      });
    }
    const isDirectory = policy.maxCount > DEFAULT_FILE_MAX_COUNT;
    const nodeCount = files.length + (isDirectory ? 1 : 0);
    if (nodeCount > MAX_FILE_VALUE_NODE_COUNT) {
      throw selectionError(FILE_SELECTION_ERROR.TOO_MANY_NODES, {
        actual: nodeCount,
        limit: MAX_FILE_VALUE_NODE_COUNT,
      });
    }

    let totalBytes = 0;
    let metadataBytes = isDirectory ? UTF8_ENCODER.encode(rootName).byteLength : 0;
    for (const file of files) {
      if (
        policy.extensions.length > 0 &&
        !fileHasAllowedExtension(file.name, policy.extensions)
      ) {
        throw selectionError(FILE_SELECTION_ERROR.EXTENSION_NOT_ALLOWED, {
          fileName: file.name,
          allowed: policy.extensions,
        });
      }
      if (policy.maxFileBytes !== null && file.size > policy.maxFileBytes) {
        throw selectionError(FILE_SELECTION_ERROR.FILE_TOO_LARGE, {
          fileName: file.name,
          actual: file.size,
          limit: policy.maxFileBytes,
        });
      }
      totalBytes += file.size;
      if (policy.maxTotalBytes !== null && totalBytes > policy.maxTotalBytes) {
        throw selectionError(FILE_SELECTION_ERROR.TOTAL_TOO_LARGE, {
          actual: totalBytes,
          limit: policy.maxTotalBytes,
        });
      }
      if (totalBytes > MAX_EXT_RAW_AGGREGATE_BYTES) {
        throw selectionError(FILE_SELECTION_ERROR.TRANSPORT_TOO_LARGE, {
          actual: totalBytes,
          limit: MAX_EXT_RAW_AGGREGATE_BYTES,
        });
      }
      metadataBytes += UTF8_ENCODER.encode(file.name).byteLength;
      metadataBytes += UTF8_ENCODER.encode(file.type).byteLength;
      if (metadataBytes > MAX_FILE_VALUE_METADATA_BYTES) {
        throw selectionError(FILE_SELECTION_ERROR.METADATA_TOO_LARGE, {
          actual: metadataBytes,
          limit: MAX_FILE_VALUE_METADATA_BYTES,
        });
      }
    }
    return files;
  }

  function readLimit(policy, bytesRead) {
    const limits = [MAX_EXT_RAW_AGGREGATE_BYTES - bytesRead];
    if (policy.maxFileBytes !== null) limits.push(policy.maxFileBytes);
    if (policy.maxTotalBytes !== null) limits.push(policy.maxTotalBytes - bytesRead);
    return Math.min(...limits);
  }

  async function readByteFile(file, policy, bytesRead) {
    const limit = readLimit(policy, bytesRead);
    const buffer = await file.slice(0, limit + READ_OVERFLOW_SENTINEL_BYTES).arrayBuffer();
    const bytes = new Uint8Array(buffer);
    if (bytes.byteLength > limit) {
      throw selectionError(FILE_SELECTION_ERROR.READ_TOO_LARGE, {
        fileName: file.name,
        limit,
      });
    }
    if (bytes.byteLength !== file.size) {
      throw selectionError(FILE_SELECTION_ERROR.READ_SIZE_MISMATCH, {
        fileName: file.name,
        expected: file.size,
        actual: bytes.byteLength,
      });
    }

    const value = {
      name: file.name,
      is_dir: false,
      content: { kind: 'bytes', bytes: encodeBase64(bytes) },
    };
    if (file.type) value.mime = file.type;
    return value;
  }

  async function readFileSelection(fileList, policy, rootName) {
    const files = preflightFileSelection(fileList, policy, rootName);
    const entries = [];
    let bytesRead = 0;
    for (const file of files) {
      const entry = await readByteFile(file, policy, bytesRead);
      entries.push(entry);
      bytesRead += file.size;
    }

    if (policy.maxCount === DEFAULT_FILE_MAX_COUNT) return entries[0];
    return {
      name: rootName,
      is_dir: true,
      content: { kind: 'directory', entries },
    };
  }

  return Object.freeze({
    FILE_POLICY_CONTROL_PROPERTY,
    FILE_SELECTION_ERROR,
    MAX_EXT_RAW_AGGREGATE_BYTES,
    MAX_FILE_POLICY_COUNT,
    MAX_FILE_VALUE_METADATA_BYTES,
    MAX_FILE_VALUE_NODE_COUNT,
    FileSelectionError,
    configureFileInput,
    parseFilePolicy,
    preflightFileSelection,
    readFileSelection,
    ...UpegFileWire,
  });
})();

if (typeof module === 'object' && module.exports) {
  module.exports = UpegFileInput;
}
