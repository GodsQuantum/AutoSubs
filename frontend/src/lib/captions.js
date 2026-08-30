
/**
 * @typedef {{ start: number, end: number, text: string }} CaptionLine
 */

/** @param {number} seconds */
function vttTime(seconds) {
  const total = Math.max(0, Math.round(Number(seconds || 0) * 1000));
  const hours = Math.floor(total / 3600000);
  const minutes = Math.floor((total % 3600000) / 60000);
  const secs = Math.floor((total % 60000) / 1000);
  const millis = total % 1000;
  return `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}:${String(secs).padStart(2, '0')}.${String(millis).padStart(3, '0')}`;
}

/**
 * Build a browser-native WebVTT track for editor preview.
 * @param {CaptionLine[]} lines
 */
export function subtitlesToVtt(lines) {
  const cues = lines.map((line, index) => {
    const end = Math.max(Number(line.end || 0), Number(line.start || 0) + 0.001);
    const text = String(line.text ?? '').replace(/\r/g, '');
    return `${index + 1}\n${vttTime(line.start)} --> ${vttTime(end)}\n${text}\n`;
  }).join('\n');

  return `WEBVTT\n\n${cues}`;
}
