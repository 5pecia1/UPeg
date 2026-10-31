// PDF worker checks use independently generated ReportLab/Pillow fixtures.
// Assertions compare report claims with decoded artifact bytes and Chromium's
// saved Blob, rather than using the compressor to calculate expected output.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { waitFor } from './cdp_client.mjs';

const digest = (bytes) => createHash('sha256').update(bytes).digest('hex');
const fixtures = ['rgb_jpeg.pdf', 'rgb_flate.pdf', 'gray_jpeg.pdf', 'mixed_hangul.pdf'];

function artifact(response, inputBytes, targetBytes, expectedStatus, expectedPages = 2) {
  const result = JSON.parse(response.resultJson);
  assert.equal(result.ok, true, JSON.stringify(result.error));
  assert.equal(result.primary_output_id, 'result');
  assert.equal(result.outputs.length, 2);
  const file = result.outputs.find((output) => output.id === 'result');
  const report = result.outputs.find((output) => output.id === 'report');
  assert.equal(file?.kind, 'file');
  assert.equal(report?.kind, 'json');
  assert.equal(file.value.is_dir, false);
  assert.equal(file.value.mime, 'application/pdf');
  assert.equal(file.value.content.kind, 'bytes');
  assert.equal(typeof file.value.content.bytes, 'string');
  const bytes = Buffer.from(file.value.content.bytes, 'base64');
  assert.equal(bytes.toString('base64'), file.value.content.bytes, 'File bytes are not canonical base64');
  assert.equal(bytes.toString('ascii', 0, 5), '%PDF-');
  assert.equal(report.value.status, expectedStatus);
  assert.equal(report.value.input_bytes, inputBytes.length);
  assert.equal(report.value.output_bytes, bytes.length);
  assert.equal(report.value.target_bytes, targetBytes);
  assert.equal(report.value.saved_bytes, inputBytes.length - bytes.length);
  if (report.value.selected_profile !== null) {
    const selected = report.value.attempts.find((attempt) => attempt.profile === report.value.selected_profile);
    assert.equal(selected?.size_bytes, bytes.length, 'selected profile size differs from artifact');
  }
  assert.ok(bytes.length <= inputBytes.length, 'PDF result grew');
  assert.equal(report.value.page_count, expectedPages);
  if (expectedStatus === 'target_met') assert.ok(bytes.length <= targetBytes);
  if (expectedStatus === 'target_unmet') assert.ok(bytes.length > targetBytes);
  if (expectedStatus === 'already_within_target') {
    assert.deepEqual(bytes, inputBytes, 'within-target PDF changed');
    assert.deepEqual(report.value.attempts, []);
  }
  return { bytes, report: report.value };
}

export async function checkPdfCompression({ session, connection, fixtureRoot, downloadDir, dispatchExpression }) {
  const inputs = new Map(fixtures.map((name) => [name, readFileSync(join(fixtureRoot, name))]));
  const argsFor = (name, targetBytes, mode = 'balanced') => JSON.stringify({
    input: {
      name, is_dir: false, mime: 'application/pdf',
      content: { kind: 'bytes', bytes: inputs.get(name).toString('base64') },
    },
    target_bytes: targetBytes, mode,
  });
  const run = async (name, targetBytes, mode, status, save = false) => {
    const argsJson = argsFor(name, targetBytes, mode);
    const response = await session.evaluate(`
      ${dispatchExpression('media', 'media.pdf_compress', argsJson)}.then((response) => {
        ${save ? 'globalThis.pdfDownloadResult = JSON.parse(response.resultJson);' : ''}
        return response;
      })
    `);
    const expectedPages = name === 'mixed_hangul.pdf' ? 3 : 2;
    return { ...artifact(response, inputs.get(name), targetBytes, status, expectedPages), argsJson };
  };

  // One byte is intrinsically too small for a PDF; no implementation-derived
  // expected size is needed to establish a missed target for these real files.
  for (const name of fixtures) {
    const missed = await run(name, 1, 'balanced', 'target_unmet');
    assert.equal(missed.report.attempts[0]?.profile, 'lossless');
    assert.ok(missed.report.attempts.some((attempt) => attempt.images_reencoded > 0), `${name}: no image recompression`);
    assert.ok(missed.report.attempts.every((attempt) => !attempt.profile.startsWith('hq_')));
  }
  const name = 'rgb_jpeg.pdf';
  await run(name, inputs.get(name).length, 'balanced', 'already_within_target');
  const highQuality = await run(name, 1, 'high_quality', 'target_unmet');
  assert.deepEqual(highQuality.report.attempts.slice(0, 3).map((attempt) => attempt.profile), [
    'lossless', 'hq_q92_original', 'hq_q85_75pct',
  ]);
  assert.ok(highQuality.report.attempts.some((attempt) => attempt.profile.startsWith('balanced_')));
  const targetBytes = Math.floor(inputs.get(name).length / 2);
  const highQualityMet = await run(name, targetBytes, 'high_quality', 'target_met');
  assert.match(highQualityMet.report.selected_profile, /^hq_q92_original$/);
  assert.ok(highQualityMet.report.attempts.some((attempt) => attempt.images_reencoded > 0));
  const met = await run(name, targetBytes, 'balanced', 'target_met', true);
  assert.match(met.report.selected_profile, /^balanced_/);
  assert.ok(met.report.attempts.some((attempt) => attempt.images_reencoded > 0));

  // Decode the same canonical File in the browser, exercise Blob.download,
  // then compare Chromium's completed file with the worker artifact in Node.
  mkdirSync(downloadDir, { recursive: true });
  await connection.send('Browser.setDownloadBehavior', { behavior: 'allow', downloadPath: downloadDir });
  const browserBlob = await session.evaluate(`(async () => {
    const file = pdfDownloadResult.outputs.find((output) => output.id === 'result').value;
    const bytes = Uint8Array.from(atob(file.content.bytes), (char) => char.charCodeAt(0));
    const blob = new Blob([bytes], { type: file.mime });
    const digest = await crypto.subtle.digest('SHA-256', await blob.arrayBuffer());
    const link = document.createElement('a');
    const url = URL.createObjectURL(blob);
    link.href = url;
    link.download = 'worker-compressed.pdf';
    document.body.append(link);
    link.click();
    link.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    return { size: blob.size, sha256: Array.from(new Uint8Array(digest),
      (byte) => byte.toString(16).padStart(2, '0')).join('') };
  })()`);
  assert.equal(browserBlob.size, met.bytes.length);
  assert.equal(browserBlob.sha256, digest(met.bytes));
  const downloaded = join(downloadDir, 'worker-compressed.pdf');
  await waitFor('PDF browser download', 10000, async () => ({ ok: existsSync(downloaded) }));
  assert.deepEqual(readFileSync(downloaded), met.bytes, 'browser download differs from worker artifact');
  console.log('ok PDF worker: four real fixtures, File/report bytes, target states, modes, Blob download');
  return { inputBytes: inputs.get(name), argsJson: met.argsJson, targetBytes, sha256: digest(met.bytes) };
}

export async function checkCachedPdfOffline(session, abiDigest, previous) {
  const response = await session.evaluate(`(async () => {
    const synced = await UpegToolkitLoader.sync({ abiDigest: ${JSON.stringify(abiDigest)} });
    return UpegToolkitLoader.dispatch({
      catalogDigest: synced.catalog.catalog_digest,
      toolkitId: 'media', toolId: 'media.pdf_compress',
      argsJson: ${JSON.stringify(previous.argsJson)},
    });
  })()`);
  const offline = artifact(response, previous.inputBytes, previous.targetBytes, 'target_met');
  assert.equal(digest(offline.bytes), previous.sha256, 'cached offline PDF differs from online artifact');
  console.log('ok PDF worker: cached media pack after Worker restart while offline');
}
