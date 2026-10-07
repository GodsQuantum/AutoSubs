/** @typedef {{ word:string, start:number, end:number }} SubtitleWord */
/** @typedef {{ id:number, start:number, end:number, text:string, words?:SubtitleWord[] }} SubtitleLine */

/** @param {SubtitleLine} line */
function copy(line) {
  return { ...line, words: line.words?.map(word => ({ ...word })) };
}

/** @param {SubtitleLine[]} lines @param {number} index @param {number} cursor @returns {SubtitleLine[]} */
export function splitSubtitleLine(lines, index, cursor) {
  const line = lines[index];
  if (!line) return lines;
  const at = Math.max(0, Math.min(line.text.length, cursor));
  const leftText = line.text.slice(0, at).trim();
  const rightText = line.text.slice(at).trim();
  if (!leftText || !rightText) return lines;
  const left = copy(line);
  const right = copy(line);
  left.text = leftText;
  right.text = rightText;
  right.id = Math.max(-1, ...lines.map(item => Number(item.id) || 0)) + 1;
  if (line.words?.length) {
    const boundary = line.text.slice(0, at).endsWith(' ') || line.text.slice(at).startsWith(' ');
    if (boundary) {
      const positions = [...line.text.matchAll(/\S+/g)].map(match => ({ start: match.index, end: match.index + match[0].length }));
      const splitWord = positions.findIndex(word => word.start >= at);
      const pivot = splitWord < 0 ? line.words.length : splitWord;
      left.words = line.words.slice(0, pivot).map(word => ({ ...word }));
      right.words = line.words.slice(pivot).map(word => ({ ...word }));
    } else {
      delete left.words;
      delete right.words;
    }
  }
  if (!left.words && !right.words) {
    const ratio = at / line.text.length;
    const seam = line.start + (line.end - line.start) * ratio;
    left.end = seam;
    right.start = seam;
  }
  return [...lines.slice(0, index), left, right, ...lines.slice(index + 1)];
}

/** @param {SubtitleLine[]} lines @param {number} leftIndex @returns {SubtitleLine[]} */
export function mergeSubtitleLines(lines, leftIndex) {
  if (leftIndex < 0 || leftIndex >= lines.length - 1) return lines;
  const left = copy(lines[leftIndex]);
  const right = lines[leftIndex + 1];
  left.text = `${left.text.trim()} ${right.text.trim()}`.trim();
  left.end = Math.max(left.end, right.end);
  if (left.words && right.words) left.words = [...left.words, ...right.words];
  else delete left.words;
  return [...lines.slice(0, leftIndex), left, ...lines.slice(leftIndex + 2)];
}

/** @param {SubtitleLine[]} lines @param {number} index @returns {SubtitleLine[]} */
export function deleteSubtitleLine(lines, index) {
  return index < 0 || index >= lines.length ? lines : lines.filter((_, position) => position !== index);
}

/** @param {SubtitleLine[]} lines @param {number} index @param {number} cursor @returns {SubtitleLine[]} */
export function insertSubtitleLineBreak(lines, index, cursor) {
  const line = lines[index];
  if (!line) return lines;
  const at = Math.max(0, Math.min(line.text.length, cursor));
  if (at === 0 || at === line.text.length) return lines;
  const next = copy(line);
  next.text = `${line.text.slice(0, at).trimEnd()}\n${line.text.slice(at).trimStart()}`;
  return [...lines.slice(0, index), next, ...lines.slice(index + 1)];
}

/**
 * @param {SubtitleLine[]} lines
 * @param {number} lineIndex
 * @param {number} wordIndex
 * @param {'start'|'end'} boundary
 * @param {number} deltaMs
 * @returns {SubtitleLine[]}
 */
export function nudgeSubtitleWordBoundary(lines, lineIndex, wordIndex, boundary, deltaMs) {
  const line = lines[lineIndex];
  const sourceWord = line?.words?.[wordIndex];
  if (!line || !sourceWord || !Number.isFinite(deltaMs)) return lines;
  const next = copy(line);
  const word = next.words?.[wordIndex];
  if (!word || !next.words) return lines;
  const delta = deltaMs / 1000;

  if (boundary === 'start') {
    const lower = wordIndex > 0 ? next.words[wordIndex - 1].end : line.start;
    const upper = word.end - 0.001;
    word.start = Math.max(lower, Math.min(upper, word.start + delta));
    if (wordIndex === 0) next.start = word.start;
  } else {
    const lower = word.start + 0.001;
    const upper = wordIndex + 1 < next.words.length ? next.words[wordIndex + 1].start : line.end;
    word.end = Math.max(lower, Math.min(upper, word.end + delta));
    if (wordIndex === next.words.length - 1) next.end = word.end;
  }

  word.start = Math.round(word.start * 1000) / 1000;
  word.end = Math.round(word.end * 1000) / 1000;
  next.start = Math.round(next.start * 1000) / 1000;
  next.end = Math.round(next.end * 1000) / 1000;
  return [...lines.slice(0, lineIndex), next, ...lines.slice(lineIndex + 1)];
}

/** @param {string} value */
function stripVisualLineTerminalPeriod(value) {
  return value.replace(/(?<!\.)\.(\s*)([”’"'»)\]}]*)(\s*)$/u, '$1$2$3');
}

/** Remove sentence-ending full stops at visual-line boundaries while preserving commas, !, ?, and ellipses.
 *  This edits caption text rather than hiding punctuation at render time, so selected punctuation can be re-added manually.
 *  @param {SubtitleLine[]} lines @returns {SubtitleLine[]}
 */
export function removeTerminalPeriods(lines) {
  return lines.map(line => {
    const next = copy(line);
    next.text = line.text.split('\n').map(stripVisualLineTerminalPeriod).join('\n');
    return next;
  });
}
