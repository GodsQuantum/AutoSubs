
import test from 'node:test';
import assert from 'node:assert/strict';
import { subtitlesToVtt } from '../src/lib/captions.js';

test('subtitlesToVtt emits valid WebVTT timing and text', () => {
  const vtt = subtitlesToVtt([
    { start: 1.25, end: 2.5, text: 'Bonjour monde' }
  ]);

  assert.match(vtt, /^WEBVTT/);
  assert.match(
    vtt,
    /00:00:01\.250 --> 00:00:02\.500/
  );
  assert.match(vtt, /Bonjour monde/);
});
