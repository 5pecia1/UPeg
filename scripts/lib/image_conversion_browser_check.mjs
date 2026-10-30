// Exercise the real downloadable media worker with self-generated raster/PDF data.
import assert from 'node:assert/strict';
import { inflateRawSync } from 'node:zlib';

function twoPagePdf() {
  const objects = [
    '<< /Type /Catalog /Pages 2 0 R >>',
    '<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 8 4] /Resources << >> /Contents 4 0 R >>',
    '<< /Length 21 >>\nstream\n1 0 0 rg 0 0 8 4 re f\nendstream',
    '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 4 8] /Resources << >> /Contents 6 0 R >>',
    '<< /Length 21 >>\nstream\n0 0 1 rg 0 0 4 8 re f\nendstream',
  ];
  // Derive stream lengths and xref offsets from the exact ASCII bytes.
  let pdf = '%PDF-1.4\n';
  const offsets = [0];
  objects.forEach((object, i) => { offsets.push(Buffer.byteLength(pdf)); pdf += `${i + 1} 0 obj\n${object}\nendobj\n`; });
  const xref = Buffer.byteLength(pdf);
  pdf += `xref\n0 ${offsets.length}\n0000000000 65535 f \n`;
  for (const offset of offsets.slice(1)) pdf += `${String(offset).padStart(10, '0')} 00000 n \n`;
  pdf += `trailer\n<< /Size ${offsets.length} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return Buffer.from(pdf, 'ascii');
}

function zipEntries(zip) {
  const end = zip.lastIndexOf(Buffer.from([0x50, 0x4b, 0x05, 0x06]));
  assert.ok(end >= 0, 'ZIP end record missing');
  const count = zip.readUInt16LE(end + 10);
  let pos = zip.readUInt32LE(end + 16);
  const files = new Map();
  for (let i = 0; i < count; i++) {
    assert.equal(zip.readUInt32LE(pos), 0x02014b50);
    const method = zip.readUInt16LE(pos + 10);
    const size = zip.readUInt32LE(pos + 20);
    const nameLength = zip.readUInt16LE(pos + 28);
    const extraLength = zip.readUInt16LE(pos + 30);
    const commentLength = zip.readUInt16LE(pos + 32);
    const local = zip.readUInt32LE(pos + 42);
    const name = zip.subarray(pos + 46, pos + 46 + nameLength).toString();
    assert.equal(zip.readUInt32LE(local), 0x04034b50);
    const dataStart = local + 30 + zip.readUInt16LE(local + 26) + zip.readUInt16LE(local + 28);
    const bytes = zip.subarray(dataStart, dataStart + size);
    assert.ok(method === 0 || method === 8, `unsupported ZIP method ${method}`);
    files.set(name, method === 8 ? inflateRawSync(bytes) : bytes);
    pos += 46 + nameLength + extraLength + commentLength;
  }
  return files;
}

const inputFile = (name, mime, bytes) => ({
  name, is_dir: false, ...(mime === null ? {} : { mime }), content: { kind: 'bytes', bytes: bytes.toString('base64') },
});
function outputFile(response) {
  const result = JSON.parse(response.resultJson);
  assert.equal(result.ok, true, JSON.stringify(result.error));
  assert.equal(result.primary_output_id, 'result');
  const output = result.outputs.find((item) => item.id === 'result');
  assert.equal(output.kind, 'file');
  return { file: output.value, bytes: Buffer.from(output.value.content.bytes, 'base64') };
}
function assertSvg(bytes) {
  const svg = bytes.toString('utf8');
  assert.match(svg, /<path\b/);
  assert.doesNotMatch(svg, /<image\b/);
  return svg;
}

export async function checkImageConversion({ session, dispatchExpression }) {
  const data = await session.evaluate(`(() => {
    const canvas = document.createElement('canvas'); canvas.width = 32; canvas.height = 16;
    const ctx = canvas.getContext('2d');
    ctx.fillStyle = 'rgba(255,0,0,0.5)'; ctx.fillRect(0, 0, 32, 16);
    return { png: canvas.toDataURL('image/png').split(',')[1], jpeg: canvas.toDataURL('image/jpeg').split(',')[1] };
  })()`);
  const png = inputFile('alpha.png', 'image/png', Buffer.from(data.png, 'base64'));
  const jpeg = inputFile('picture.jpg', 'image/jpeg', Buffer.from(data.jpeg, 'base64'));
  const run = (toolId, args) => session.evaluate(dispatchExpression('media', toolId, JSON.stringify(args)));
  const defaults = outputFile(await run('media.image_convert', { input: png, output_format: 'svg' }));
  assert.equal(defaults.file.name, 'alpha.svg');
  assert.equal(defaults.file.mime, 'image/svg+xml');
  assertSvg(defaults.bytes);
  const poster = outputFile(await run('media.image_convert', { input: png, output_format: 'svg', vector_preset: 'poster' }));
  assert.deepEqual(defaults.bytes, poster.bytes, 'default preset is not poster');
  const black = outputFile(await run('media.image_convert', { input: png, output_format: 'svg', background: '#000000' }));
  assert.notDeepEqual(defaults.bytes, black.bytes, 'SVG tracing ignored alpha background');
  assertSvg(outputFile(await run('media.image_convert', { input: jpeg, output_format: 'svg' })).bytes);
  const batch = outputFile(await run('media.images_convert', {
    images: { name: 'images', is_dir: true, content: { kind: 'directory', entries: [png, jpeg] } },
    output_format: 'svg',
  }));
  assert.equal(batch.file.mime, 'application/zip');
  const entries = zipEntries(batch.bytes);
  assert.deepEqual([...entries.keys()], ['alpha.svg', 'picture.svg']);
  for (const bytes of entries.values()) assertSvg(bytes);
  const ai = inputFile('two-pages.ai', 'application/pdf', twoPagePdf());
  const raster = outputFile(await run('media.image_convert', { input: ai, output_format: 'png' }));
  assert.equal(raster.file.name, 'two-pages.png');
  assert.equal(raster.file.mime, 'image/png');
  assert.deepEqual([...raster.bytes.subarray(0, 8)], [137, 80, 78, 71, 13, 10, 26, 10]);
  assert.equal(raster.bytes.readUInt32BE(16), 16, 'default 144 DPI first page width');
  assert.equal(raster.bytes.readUInt32BE(20), 8, 'default 144 DPI first page height');
  const invalid = JSON.parse((await run('media.image_convert', {
    input: inputFile('invalid.ai', null, Buffer.from('not a PDF')), output_format: 'png',
  })).resultJson);
  assert.equal(invalid.ok, false);
  assert.match(invalid.error.message, /PDF|Illustrator/i);
  console.log('ok image conversion worker: PNG/JPEG to SVG, presets/background, batch ZIP, AI first page and error');
}
