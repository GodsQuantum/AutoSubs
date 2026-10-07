import test from 'node:test';
import assert from 'node:assert/strict';
import {
  splitSubtitleLine,
  mergeSubtitleLines,
  deleteSubtitleLine,
  insertSubtitleLineBreak,
  nudgeSubtitleWordBoundary
} from '../src/lib/subtitle-edit.js';

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

test('insertSubtitleLineBreak changes only visual text and preserves canonical word timings', () => {
  const result = insertSubtitleLineBreak([line], 0, 11);
  assert.equal(result[0].text, 'hello world\nagain');
  assert.deepEqual(result[0].words, line.words);
  assert.equal(result[0].start, line.start);
  assert.equal(result[0].end, line.end);
});

test('nudgeSubtitleWordBoundary respects adjacent word boundaries', () => {
  const moved = nudgeSubtitleWordBoundary([line], 0, 1, 'start', -500);
  assert.equal(moved[0].words[1].start, 1);
  const endMoved = nudgeSubtitleWordBoundary([line], 0, 1, 'end', 250);
  assert.equal(endMoved[0].words[1].end, 2);
  const precise = nudgeSubtitleWordBoundary([line], 0, 1, 'start', 50);
  assert.equal(precise[0].words[1].start, 1.05);
});

test('removeTerminalPeriods strips only caption-ending full stops', async () => {
  const { removeTerminalPeriods } = await import('../src/lib/subtitle-edit.js');
  const input = [
    { ...line, id: 10, text: 'Bonjour.' },
    { ...line, id: 11, text: 'Ça va !' },
    { ...line, id: 12, text: 'Vraiment ?' },
    { ...line, id: 13, text: 'Oui,' },
    { ...line, id: 14, text: 'Attends...' },
    { ...line, id: 15, text: 'M. Dupont.' },
    { ...line, id: 16, text: '« Bonjour. »' },
    { ...line, id: 17, text: 'Première ligne.\nDeuxième ligne.' }
  ];
  assert.deepEqual(removeTerminalPeriods(input).map(({ text }) => text), [
    'Bonjour',
    'Ça va !',
    'Vraiment ?',
    'Oui,',
    'Attends...',
    'M. Dupont',
    '« Bonjour »',
    'Première ligne\nDeuxième ligne'
  ]);
});
