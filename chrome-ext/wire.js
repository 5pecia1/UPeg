'use strict';

const UpegWire = (() => {
  const BASE64_ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
  const BASE64_PADDING = '=';
  const BYTES_PER_MEBIBYTE = 1024 * 1024;
  // The output raw-byte budget below restates the canonical `File` output
  // budget declared once in Rust at `upeg-core/src/input/file_budget.rs`.
  // There is no shared build-time source between JS and Rust, so this
  // number is pinned — not merely mirrored — by
  // `upeg-core/tests/file_budget_cross_language_pin.rs`, which reads this
  // file by path and fails the build if it drifts from Rust.
  const MAX_FILE_OUTPUT_RAW_BYTES = 64 * BYTES_PER_MEBIBYTE;
  const MAX_REQUEST_BODY_BYTES = 1_000_000;
  const BASE64_WIRE_ERROR = Object.freeze({
    INVALID: 'invalid',
    TOO_LARGE: 'too_large',
  });

  class Base64WireError extends Error {
    constructor(code, actualBytes = null) {
      const message =
        code === BASE64_WIRE_ERROR.TOO_LARGE
          ? `Decoded File output is ${actualBytes} bytes; the limit is ${MAX_FILE_OUTPUT_RAW_BYTES}.`
          : 'File bytes must be canonical padded RFC 4648 base64.';
      super(message);
      this.name = 'Base64WireError';
      this.code = code;
      this.actualBytes = actualBytes;
      this.limitBytes = MAX_FILE_OUTPUT_RAW_BYTES;
    }
  }

  class RequestBodyTooLargeError extends Error {
    constructor(actualBytes) {
      super(`Request body is ${actualBytes} bytes; the limit is below ${MAX_REQUEST_BODY_BYTES}.`);
      this.name = 'RequestBodyTooLargeError';
      this.actualBytes = actualBytes;
      this.limitBytes = MAX_REQUEST_BODY_BYTES;
    }
  }

  function encodeBase64(bytes) {
    let encoded = '';
    for (let index = 0; index < bytes.byteLength; index += 3) {
      const first = bytes[index];
      const second = bytes[index + 1];
      const third = bytes[index + 2];
      const chunk = (first << 16) | ((second || 0) << 8) | (third || 0);
      encoded += BASE64_ALPHABET[(chunk >>> 18) & 63];
      encoded += BASE64_ALPHABET[(chunk >>> 12) & 63];
      encoded += second === undefined ? '=' : BASE64_ALPHABET[(chunk >>> 6) & 63];
      encoded += third === undefined ? '=' : BASE64_ALPHABET[chunk & 63];
    }
    return encoded;
  }

  function decodeBase64(encoded) {
    if (typeof encoded !== 'string') throw new Base64WireError(BASE64_WIRE_ERROR.INVALID);
    if (encoded.length === 0) return new Uint8Array();
    if (encoded.length % 4 !== 0) throw new Base64WireError(BASE64_WIRE_ERROR.INVALID);

    const finalCharacter = encoded[encoded.length - 1];
    const penultimateCharacter = encoded[encoded.length - 2];
    const paddingBytes =
      finalCharacter === BASE64_PADDING
        ? penultimateCharacter === BASE64_PADDING
          ? 2
          : 1
        : 0;
    const decodedLength = (encoded.length / 4) * 3 - paddingBytes;
    if (decodedLength > MAX_FILE_OUTPUT_RAW_BYTES) {
      throw new Base64WireError(BASE64_WIRE_ERROR.TOO_LARGE, decodedLength);
    }

    const bytes = new Uint8Array(decodedLength);
    let outputIndex = 0;
    for (let index = 0; index < encoded.length; index += 4) {
      const first = BASE64_ALPHABET.indexOf(encoded[index]);
      const second = BASE64_ALPHABET.indexOf(encoded[index + 1]);
      const finalQuartet = index + 4 === encoded.length;
      let third = BASE64_ALPHABET.indexOf(encoded[index + 2]);
      let fourth = BASE64_ALPHABET.indexOf(encoded[index + 3]);
      let canonicalTail = true;
      if (finalQuartet && paddingBytes === 1) {
        canonicalTail = encoded[index + 3] === BASE64_PADDING && (third & 3) === 0;
        fourth = 0;
      } else if (finalQuartet && paddingBytes === 2) {
        canonicalTail =
          encoded[index + 2] === BASE64_PADDING &&
          encoded[index + 3] === BASE64_PADDING &&
          (second & 15) === 0;
        third = 0;
        fourth = 0;
      }
      if (first < 0 || second < 0 || third < 0 || fourth < 0 || !canonicalTail) {
        throw new Base64WireError(BASE64_WIRE_ERROR.INVALID);
      }
      const chunk = (first << 18) | (second << 12) | (third << 6) | fourth;
      if (outputIndex < bytes.byteLength) bytes[outputIndex++] = (chunk >>> 16) & 255;
      if (outputIndex < bytes.byteLength) bytes[outputIndex++] = (chunk >>> 8) & 255;
      if (outputIndex < bytes.byteLength) bytes[outputIndex++] = chunk & 255;
    }
    return bytes;
  }

  function serializeJsonRequestBody(value) {
    const body = JSON.stringify(value);
    const actualBytes = new TextEncoder().encode(body).byteLength;
    if (actualBytes >= MAX_REQUEST_BODY_BYTES) {
      throw new RequestBodyTooLargeError(actualBytes);
    }
    return body;
  }

  return Object.freeze({
    BASE64_WIRE_ERROR,
    MAX_FILE_OUTPUT_RAW_BYTES,
    MAX_REQUEST_BODY_BYTES,
    Base64WireError,
    RequestBodyTooLargeError,
    decodeBase64,
    encodeBase64,
    serializeJsonRequestBody,
  });
})();

if (typeof module === 'object' && module.exports) {
  module.exports = UpegWire;
}
