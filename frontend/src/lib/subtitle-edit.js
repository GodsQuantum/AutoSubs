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
