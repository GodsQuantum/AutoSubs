import test from 'node:test';
import assert from 'node:assert/strict';
import { splitSubtitleLine, mergeSubtitleLines, deleteSubtitleLine } from '../src/lib/subtitle-edit.js';

const line = {
  id: 4, start: 0, end: 3, text: 'hello world again',
  words: [
    { word: 'hello', start: 0, end: 1 },
    { word: 'world', start: 1, end: 2 },
    { word: 'again', start: 2, end: 3 }
  ]
};

test('splitSubtitleLine keeps word timing on each side of the cursor', () => {
  const result = splitSubtitleLine([line], 0, 11);
  assert.deepEqual(result.map(({ text }) => text), ['hello world', 'again']);
  assert.deepEqual(result[0].words, line.words.slice(0, 2));
  assert.deepEqual(result[1].words, line.words.slice(2));
});

test('mergeSubtitleLines combines adjacent timed words', () => {
  const result = mergeSubtitleLines(splitSubtitleLine([line], 0, 11), 0);
  assert.equal(result.length, 1);
  assert.equal(result[0].text, line.text);
  assert.deepEqual(result[0].words, line.words);
});

test('deleteSubtitleLine removes only the selected block', () => {
  assert.deepEqual(deleteSubtitleLine([line, { ...line, id: 5 }], 0).map(({ id }) => id), [5]);
});
