import test from 'node:test';
import assert from 'node:assert/strict';
import {
  clampPreviewPosition,
  formatRatio,
  previewWidthForRatio,
  safeZoneGuide,
  videoObjectFit,
  activeWordIndex,
  demoSubtitleWords,
  previewSubtitleTokens,
  subtitlePositionBounds,
  karaokeProgress,
  loopedPreviewTime,
  customFontMatch,
  verifyCustomFont
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

test('subtitle bounds keep the complete rendered block inside five-percent margins', () => {
  assert.deepEqual(subtitlePositionBounds(1000, 500, 400, 100), {
    minX: 25, maxX: 75, minY: 15, maxY: 85
  });
  assert.deepEqual(clampPreviewPosition(2, 98, { minX: 25, maxX: 75, minY: 15, maxY: 85 }), { x: 25, y: 85 });
});

test('preview keeps explicit subtitle lines without browser wrapping', async () => {
  const source = await (await import('node:fs/promises')).readFile(
    new URL('../src/lib/components/FormatPreview.svelte', import.meta.url), 'utf8'
  );
  assert.match(source, /white-space:pre-line/);
  assert.match(source, /overflow-wrap:normal/);
  assert.doesNotMatch(source, /overflow-wrap:anywhere/);
});

test('preview selects the current timed word and deterministic preset demo words', () => {
  const words = [{ word: 'one', start: 1, end: 1.3 }, { word: 'two', start: 1.4, end: 1.9 }];
  assert.equal(activeWordIndex(words, 1.2), 0);
  assert.equal(activeWordIndex(words, 1.5), 1);
  assert.deepEqual(demoSubtitleWords('ONE TWO').map(({ word, start, end }) => ({ word, start, end })), [
    { word: 'ONE', start: 0, end: 0.5 }, { word: 'TWO', start: 0.5, end: 1 }
  ]);
});

test('preview preserves authored spaces and explicit line breaks with timed words', () => {
  const words = [{ word: "J'arrive", start: 0, end: 0.4 }, { word: 'demain.', start: 0.4, end: 1 }];
  assert.deepEqual(previewSubtitleTokens("J'arrive\ndemain.", words), [
    { word: "J'arrive", start: 0, end: 0.4, separator: '' },
    { word: 'demain.', start: 0.4, end: 1, separator: '\n' }
  ]);
  assert.deepEqual(previewSubtitleTokens('deux   mots', words).map(token => token.separator), ['', ' ']);
});

test('word highlight remains continuous through timing gaps and karaoke follows elapsed time', () => {
  const words = [{ start: 1, end: 1.3 }, { start: 1.6, end: 2 }];
  assert.equal(activeWordIndex(words, 1.5), 0);
  assert.equal(activeWordIndex(words, 0.5), 0);
  assert.equal(activeWordIndex(words, 3), 1);
  assert.ok(Math.abs(karaokeProgress(1.75, 1.6, 2) - 37.5) < 1e-9);
  assert.equal(karaokeProgress(4, 1.6, 2), 100);
});

test('preview exposes every animation family and timed-word inputs', async () => {
  const source = await (await import('node:fs/promises')).readFile(
    new URL('../src/lib/components/FormatPreview.svelte', import.meta.url), 'utf8'
  );
  for (const style of ['pop', 'highlight', 'bounce', 'karaoke', 'fade', 'slide-up', 'none']) {
    assert.match(source, new RegExp(`animation-${style.replace('-', '\\-')}`));
  }
  assert.match(source, /export let words/);
  assert.match(source, /export let currentTime/);
  assert.match(source, /animation-play-state/);
  assert.match(source, /preview-floating/);
  assert.match(source, /wobbleSpeed/);
  assert.match(source, /preview-animation-inner/);
  assert.match(source, /--event-time/);
  assert.match(source, /--word-time/);
  assert.match(source, /videoElement\.currentTime/);
  assert.match(source, /on:pause/);
  assert.match(source, /{controls}/);
  assert.doesNotMatch(source, /toggleMedia|media-controls|source'\?':'fill'/);
  assert.match(source, /awaiting-source/);
  assert.match(source, /font-synthesis:none/);
  assert.match(source, /font-family:\s*\\?"\$\{renderedFamily/);
  assert.doesNotMatch(source, /p\.fontFamily\},sans-serif/);
});

test('applying a selected preset enforces its segmentation on the job', async () => {  const source = await (await import('node:fs/promises')).readFile(    new URL('../src/lib/views/EditorView.svelte', import.meta.url), 'utf8'  );  assert.match(source, /on:change={applyPresetDefaults}/);  assert.ok(source.includes('if(selectedPreset&&lines.length)lines=await api.regroup(job.id,maxChars,maxLines)')); });
test('demo preview time loops without losing pause continuity', () => {
  assert.equal(loopedPreviewTime(0, 3), 0);
  assert.equal(loopedPreviewTime(3.25, 3), 0.25);
  assert.equal(loopedPreviewTime(7, 3), 1);
});

test('preview metrics scale exactly from rendered canvas height', async () => { const { scalePreviewMetric } = await import('../src/lib/preview.js'); assert.equal(scalePreviewMetric(28, 280, 1920), 28 * 280 / 1920); assert.equal(scalePreviewMetric(60, 225, 1080), 12.5); });test('font matching distinguishes every file by its full face name', () => { const fonts = [{ family:'League Spartan', fullName:'League Spartan Light', fileName:'LeagueSpartan-Light.ttf', weight:300, italic:false }, { family:'League Spartan', fullName:'League Spartan Bold', fileName:'LeagueSpartan-Bold.ttf', weight:700, italic:false }]; assert.deepEqual(customFontMatch(fonts, 'League Spartan Light'), fonts[0]); assert.deepEqual(customFontMatch(fonts, 'League Spartan Bold'), fonts[1]); });test('successful empty API responses do not require JSON', async () => { const module = await import('../src/lib/api-response.js').catch(() => ({})); assert.equal(typeof module.parseApiResponse, 'function'); assert.equal(await module.parseApiResponse(new Response(null, { status:200 })), undefined); });
test('preview metrics keep a constant visual ratio from the 1080p preset baseline', async () => {
  const { scalePreviewMetric } = await import('../src/lib/preview.js');
  assert.equal(scalePreviewMetric(28, 280), 28 * 280 / 1080);
  assert.equal(scalePreviewMetric(60, 225), 12.5);
});
test('custom font verification distinguishes catalog fonts from system fonts', async () => {
  const fonts = [{ family: 'Studio Sans', fileName: 'studio-sans.woff2', weight: 700, italic: true }];
  assert.deepEqual(customFontMatch(fonts, 'Studio Sans'), fonts[0]);
  assert.deepEqual(customFontMatch(fonts, 'Studio Sans,Legacy Alias', 700, true), fonts[0]);
  assert.equal(customFontMatch(fonts, 'Inter'), null);

  const calls = [];
  const fontSet = {
    load: async query => { calls.push(['load', query]); return [{}]; },
    check: query => { calls.push(['check', query]); return true; }
  };
  assert.deepEqual(await verifyCustomFont(fontSet, fonts[0]), { status: 'loaded', fileName: 'studio-sans.woff2' });
  assert.deepEqual(calls, [['load', 'italic 700 16px "Studio Sans"'], ['check', 'italic 700 16px "Studio Sans"']]);
  assert.deepEqual(await verifyCustomFont(undefined, fonts[0]), { status: 'unavailable', fileName: 'studio-sans.woff2' });
});
test('subtitle download keeps the server filename and surfaces API errors', async () => {
  const { fetchDownload, filenameFromDisposition } = await import('../src/lib/download.js');
  assert.equal(filenameFromDisposition('attachment; filename="clip.srt"', 'fallback.srt'), 'clip.srt');
  assert.equal(filenameFromDisposition("attachment; filename*=UTF-8''sous-titres%20final.srt", 'fallback.srt'), 'sous-titres final.srt');
  const payload = await fetchDownload('/subtitles', 'fallback.srt', async () =>
    new Response('1\n00:00:00,000 --> 00:00:01,000\nBonjour', {
      headers: { 'content-disposition': 'attachment; filename="bonjour.srt"' }
    })
  );
  assert.equal(payload.filename, 'bonjour.srt');
  assert.equal(await payload.blob.text(), '1\n00:00:00,000 --> 00:00:01,000\nBonjour');
  await assert.rejects(
    fetchDownload('/subtitles', 'fallback.srt', async () =>
      new Response(JSON.stringify({ error: { message: 'export unavailable' } }), { status: 409 })
    ),
    /export unavailable/
  );
});
