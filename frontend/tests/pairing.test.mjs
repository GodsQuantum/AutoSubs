import test from 'node:test';
import assert from 'node:assert/strict';
import { pairFiles } from '../src/lib/pairing.js';

test('pairs sidecars by basename and ignores internal staging files', () => {
  const items = pairFiles([
    { name: 'clip.mov' }, { name: 'clip.srt' }, { name: 'other.mp4' }, { name: 'other.partial' },
    { name: '.third.partial-123.mp4' }, { name: 'clip_words.json' }, { name: 'fourth.uploading' }
  ]);
  assert.equal(items.length, 2);
  assert.equal(items[0].video.name, 'clip.mov');
  assert.equal(items[0].sidecar?.name, 'clip.srt');
  assert.equal(items[1].video.name, 'other.mp4');
  assert.equal(items[1].sidecar, undefined);
});

test('accepts SSA sidecars', () => {
  const [item] = pairFiles([{ name: 'scene.mkv' }, { name: 'scene.ssa' }]);
  assert.equal(item.sidecar?.name, 'scene.ssa');
});
