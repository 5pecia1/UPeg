'use strict';

// upeg in-page detectors — the page-side capability that only a browser
// extension can offer (docs/ui-ux-surface-contract.md "Chrome extension
// contract").
//
// The detector set is DATA, not code: `DETECTORS` is a frozen table and
// every behaviour below is a total function of a row. Adding a detector
// means adding a row — a pattern, a label key, an optional host tool plus
// the argument name that receives the match, and (when the value can be
// derived offline) a preview function. Nothing else in the extension
// branches on which detector fired.
//
// A row's `tool` is `null` when no upeg tool can resolve it. That is not a
// placeholder: `epoch` has no host tool today (the `time` toolkit ships
// `time.epoch_now`/`time.iso_now`, neither of which takes an epoch), so it
// renders its offline preview and is deliberately NOT wired to a
// click affordance — a dead deep link is worse than a plain annotation.
//
// Dependency-free and chrome-free on purpose: content.js supplies the
// chrome.i18n lookup and the DOM, this file supplies the decisions, and
// chrome-ext/tests/detectors.test.js exercises them with neither.

const UpegDetectors = (() => {
  const DETECTOR_ID = Object.freeze({
    HEX: 'hex',
    BASE64: 'base64',
    EPOCH: 'epoch',
  });

  // Tool ids and the input argument each match is passed as. Both are
  // pinned against the real toolbox by
  // `upeg-cli/tests/chrome_ext.rs::tool_ids_and_args_named_by_the_detector_table_exist_in_the_toolbox`.
  const DETECTOR_TOOL = Object.freeze({
    HEX_TO_DECIMAL: Object.freeze({ id: 'num.hex_to_decimal', arg: 'input' }),
    BASE64_DECODE: Object.freeze({ id: 'convert.base64_decode', arg: 'input' }),
  });

  const HEX_PATTERN = /\b0[xX][0-9a-fA-F]{2,64}\b/g;
  const HEX_PREFIX_PATTERN = /^0[xX]/;
  const HEX_PREFIX_LENGTH = '0x'.length;

  // Padded RFC 4648 base64, delimited by anything outside the alphabet so a
  // blob is never clipped out of a longer run. Lookbehind keeps the `0x`
  // of a hex token from seeding a false base64 match at the `x`.
  const BASE64_PATTERN = /(?<![A-Za-z0-9+/=])[A-Za-z0-9+/]{16,}={0,2}(?![A-Za-z0-9+/=])/g;
  const BASE64_MIN_LENGTH = 16;
  const BASE64_GROUP_SIZE = 4;
  const HEX_ONLY_PATTERN = /^[0-9a-fA-F]+$/;

  // 13 digits first: the alternation is ordered so a millisecond stamp is
  // never clipped to its leading 10 digits.
  const EPOCH_PATTERN = /\b(?:\d{13}|\d{10})\b/g;
  const EPOCH_SECONDS_DIGITS = 10;
  const EPOCH_MILLIS_DIGITS = 13;
  const MILLIS_PER_SECOND = 1000;

  function normalizeHex(raw) {
    const token = raw.trim();
    return HEX_PREFIX_PATTERN.test(token) && token.length > HEX_PREFIX_LENGTH ? token : null;
  }

  function previewHex(value) {
    try {
      return BigInt(value).toString();
    } catch {
      return null;
    }
  }

  /// A base64 blob is only worth annotating when it *cannot* be read as
  /// something more specific. A run of hex digits is a hash or an address
  /// (both common on the pages this extension runs on) and a run that is
  /// not a whole number of 4-character groups cannot be padded base64.
  function normalizeBase64(raw) {
    const token = raw.trim();
    if (token.length < BASE64_MIN_LENGTH) return null;
    if (token.length % BASE64_GROUP_SIZE !== 0) return null;
    if (HEX_ONLY_PATTERN.test(token)) return null;
    return token;
  }

  function normalizeEpoch(raw) {
    const token = raw.trim();
    return token.length === EPOCH_SECONDS_DIGITS || token.length === EPOCH_MILLIS_DIGITS
      ? token
      : null;
  }

  function previewEpoch(value) {
    const millis =
      value.length === EPOCH_MILLIS_DIGITS
        ? Number(value)
        : Number(value) * MILLIS_PER_SECOND;
    if (!Number.isSafeInteger(millis)) return null;
    const instant = new Date(millis);
    return Number.isNaN(instant.getTime()) ? null : instant.toISOString();
  }

  /// Detector rows, in resolution priority order: when two rows match at
  /// the same offset the earlier row wins (see [`detect`]). `hex` before
  /// `base64` keeps `0xDEADBEEF…` from being read as a blob; `base64`
  /// before `epoch` keeps a long blob from being clipped at a 10-digit run
  /// inside it.
  const DETECTORS = Object.freeze([
    Object.freeze({
      id: DETECTOR_ID.HEX,
      pattern: HEX_PATTERN,
      labelKey: 'detectorLabelHex',
      tool: DETECTOR_TOOL.HEX_TO_DECIMAL,
      normalize: normalizeHex,
      preview: previewHex,
    }),
    Object.freeze({
      id: DETECTOR_ID.BASE64,
      pattern: BASE64_PATTERN,
      labelKey: 'detectorLabelBase64',
      tool: DETECTOR_TOOL.BASE64_DECODE,
      // No offline preview: decoding base64 to text also means validating
      // UTF-8, which is exactly what `convert.base64_decode` exists to do.
      // The extension asks the host rather than growing a second decoder
      // that could disagree with it.
      preview: null,
      normalize: normalizeBase64,
    }),
    Object.freeze({
      id: DETECTOR_ID.EPOCH,
      pattern: EPOCH_PATTERN,
      labelKey: 'detectorLabelEpoch',
      tool: null,
      normalize: normalizeEpoch,
      preview: previewEpoch,
    }),
  ]);

  function detectorFor(detectorId) {
    return DETECTORS.find((detector) => detector.id === detectorId) || null;
  }

  /// Every detection in `text`, non-overlapping, in document order.
  ///
  /// Each row's pattern is cloned per scan, so a stateful `/g` regex can
  /// never leak `lastIndex` between two calls — the class of bug the old
  /// single-detector content script had to defuse with manual resets.
  function detect(text) {
    if (typeof text !== 'string' || text.length === 0) return [];

    const candidates = [];
    DETECTORS.forEach((detector, priority) => {
      const pattern = new RegExp(detector.pattern.source, detector.pattern.flags);
      let match = pattern.exec(text);
      while (match !== null) {
        if (match[0].length === 0) {
          pattern.lastIndex += 1;
        } else {
          const value = detector.normalize(match[0]);
          if (value !== null) {
            candidates.push({
              detectorId: detector.id,
              priority,
              index: match.index,
              length: match[0].length,
              raw: match[0],
              value,
            });
          }
        }
        match = pattern.exec(text);
      }
    });

    candidates.sort(
      (left, right) =>
        left.index - right.index ||
        left.priority - right.priority ||
        right.length - left.length,
    );

    const detections = [];
    let cursor = 0;
    for (const candidate of candidates) {
      if (candidate.index < cursor) continue;
      detections.push(
        Object.freeze({
          detectorId: candidate.detectorId,
          index: candidate.index,
          length: candidate.length,
          raw: candidate.raw,
          value: candidate.value,
        }),
      );
      cursor = candidate.index + candidate.length;
    }
    return detections;
  }

  function hasDetection(text) {
    return detect(text).length > 0;
  }

  /// The `{ argName: value }` body a detection dispatches with, or `null`
  /// when its row declares no host tool.
  function dispatchArgsFor(detection) {
    const detector = detectorFor(detection.detectorId);
    if (detector === null || detector.tool === null) return null;
    return { [detector.tool.arg]: detection.value };
  }

  function toolIdFor(detection) {
    const detector = detectorFor(detection.detectorId);
    return detector === null || detector.tool === null ? null : detector.tool.id;
  }

  function labelKeyFor(detection) {
    const detector = detectorFor(detection.detectorId);
    return detector === null ? null : detector.labelKey;
  }

  /// Offline value for a detection, or `null` when the row has no preview
  /// (the host is then the only source of an answer).
  function previewFor(detection) {
    const detector = detectorFor(detection.detectorId);
    if (detector === null || detector.preview === null) return null;
    return detector.preview(detection.value);
  }

  return Object.freeze({
    BASE64_MIN_LENGTH,
    DETECTORS,
    DETECTOR_ID,
    DETECTOR_TOOL,
    EPOCH_MILLIS_DIGITS,
    EPOCH_SECONDS_DIGITS,
    detect,
    detectorFor,
    dispatchArgsFor,
    hasDetection,
    labelKeyFor,
    previewFor,
    toolIdFor,
  });
})();

if (typeof module === 'object' && module.exports) {
  module.exports = UpegDetectors;
}
