import test from 'node:test';
import assert from 'node:assert/strict';
import {
  clampPreviewPosition,
  formatRatio,
  previewWidthForRatio,
  safeZoneGuide,
  videoObjectFit
} from '../src/lib/preview.js';

test('formatRatio preserves exact standard and custom canvas ratios', () => {
  assert.equal(formatRatio({ key: 'portrait916', fit: 'cover' }), 9 / 16);
  assert.equal(formatRatio({ key: 'landscape169', fit: 'cover' }), 16 / 9);
  assert.equal(formatRatio({ key: 'square11', fit: 'cover' }), 1);
  assert.equal(formatRatio({ key: 'portrait45', fit: 'cover' }), 4 / 5);
  assert.equal(formatRatio({ key: 'custom', fit: 'cover', width: 900, height: 3200 }), 900 / 3200);
  assert.equal(formatRatio({ key: 'source', fit: 'preserve' }, 4 / 3), 4 / 3);
});

test('previewWidthForRatio fits both width and height without distorting portrait canvases', () => {
  assert.equal(previewWidthForRatio(9 / 16, 720, 520), 292.5);
  assert.equal(previewWidthForRatio(1, 720, 520), 520);
  assert.equal(previewWidthForRatio(16 / 9, 720, 520), 720);
});

test('safeZoneGuide returns conservative non-destructive overlays', () => {
  assert.equal(safeZoneGuide('off'), null);
  const reels = safeZoneGuide('reels');
  assert.equal(reels.label, 'Reels');
  assert.equal(reels.top, 0.14);
  assert.equal(reels.bottom, 0.35);
  assert.ok(reels.right > reels.left);
});

test('videoObjectFit mirrors render fit semantics', () => {
  assert.equal(videoObjectFit({ key: 'source', fit: 'preserve' }), 'contain');
  assert.equal(videoObjectFit({ key: 'portrait916', fit: 'contain' }), 'contain');
  assert.equal(videoObjectFit({ key: 'portrait916', fit: 'cover' }), 'cover');
  assert.equal(videoObjectFit({ key: 'portrait916', fit: 'stretch' }), 'fill');
});

test('clampPreviewPosition keeps drag coordinates inside the canvas', () => {
  assert.deepEqual(clampPreviewPosition(-12, 140), { x: 0, y: 100 });
  assert.deepEqual(clampPreviewPosition(42.5, 67), { x: 42.5, y: 67 });
});

test('preview keeps explicit subtitle lines without browser wrapping', async () => {
  const source = await (await import('node:fs/promises')).readFile(
    new URL('../src/lib/components/FormatPreview.svelte', import.meta.url), 'utf8'
  );
  assert.match(source, /white-space:pre-line/);
  assert.match(source, /overflow-wrap:normal/);
  assert.doesNotMatch(source, /overflow-wrap:anywhere/);
});
