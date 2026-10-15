import test from 'node:test';
import assert from 'node:assert/strict';
import { addRange, totalDuration, timeLabel, nextPlaybackTime, restoreRanges } from '../src/lib/cut-selection.js';

test('adds ordered non-overlapping ranges even when selected out of order', () => {
  const ranges = addRange([{start:12,end:15}], 2, 6, 20);
  assert.deepEqual(ranges, [{start:2,end:6},{start:12,end:15}]);
  assert.equal(totalDuration(ranges), 7);
});

test('rejects inverted, overlapping, outside-duration or non-finite ranges', () => {
  const original = [{start:2,end:6}];
  for (const [start,end,duration] of [[5,7,20],[12,10,20],[-1,2,20],[4,21,20],[NaN,11,20],[1,2,0]]) {
    assert.throws(() => addRange(original,start,end,duration), /plage|durée|chevauch/i);
  }
});

test('supports precise hour-long clips and playback skips excluded sections', () => {
  assert.equal(timeLabel(3670.125),'01:01:10.125');
  const ranges=[{start:2,end:4},{start:10,end:13}];
  assert.equal(nextPlaybackTime(ranges, 4.03),10);
  assert.equal(nextPlaybackTime(ranges, 13.03),null);
  assert.equal(nextPlaybackTime(ranges, 2.75),2.75);
});

test('restores only valid saved ranges; never reuses stale or corrupt drafts',()=>{
  assert.deepEqual(restoreRanges('[{"start":1,"end":3}]',20),[{start:1,end:3}]);
  assert.deepEqual(restoreRanges('[{"start":2,"end":6},{"start":5,"end":8}]',20),[]);
  assert.deepEqual(restoreRanges('{bad',20),[]);
  assert.deepEqual(restoreRanges('[{"start":1,"end":30}]',20),[]);
});
