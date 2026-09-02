/** @typedef {'source'|'portrait916'|'landscape169'|'square11'|'portrait45'|'custom'} FormatKey */
/** @typedef {'preserve'|'contain'|'cover'|'stretch'} FitMode */
/** @typedef {{ key: FormatKey, fit: FitMode, width?: number, height?: number }} FormatProfile */
/** @typedef {'off'|'generic'|'tiktok'|'reels'|'shorts'} SafeZoneKey */
/** @typedef {{ label: string, top: number, right: number, bottom: number, left: number }} SafeZoneGuide */

/**
 * Conservative editing guides, not pixel-exact platform contracts. TikTok and
 * Shorts explicitly vary overlays by placement/device; Reels UI also changes.
 * These guides only affect preview and are never exported.
 * @type {Readonly<Record<Exclude<SafeZoneKey, 'off'>, Readonly<SafeZoneGuide>>>}
 */
const SAFE_ZONES = Object.freeze({
  generic: Object.freeze({ label: 'Generic', top: 0.08, right: 0.08, bottom: 0.12, left: 0.08 }),
  tiktok: Object.freeze({ label: 'TikTok', top: 0.12, right: 0.18, bottom: 0.24, left: 0.06 }),
  reels: Object.freeze({ label: 'Reels', top: 0.14, right: 0.12, bottom: 0.35, left: 0.06 }),
  shorts: Object.freeze({ label: 'Shorts', top: 0.10, right: 0.18, bottom: 0.22, left: 0.06 })
});


/** @param {number} frameWidth @param {number} frameHeight @param {number} blockWidth @param {number} blockHeight @param {{top?:number,right?:number,bottom?:number,left?:number}} [margin] */
export function subtitlePositionBounds(frameWidth, frameHeight, blockWidth, blockHeight, margin = {}) {
  const width = Math.max(1, Number(frameWidth) || 1);
  const height = Math.max(1, Number(frameHeight) || 1);
  const halfWidth = Math.min(width / 2, Math.max(0, Number(blockWidth) || 0) / 2);
  const halfHeight = Math.min(height / 2, Math.max(0, Number(blockHeight) || 0) / 2);
  const left = Math.max(0, Number(margin.left ?? 0.05)) * width;
  const right = Math.max(0, Number(margin.right ?? 0.05)) * width;
  const top = Math.max(0, Number(margin.top ?? 0.05)) * height;
  const bottom = Math.max(0, Number(margin.bottom ?? 0.05)) * height;
  const minX = Math.min(50, (left + halfWidth) / width * 100);
  const maxX = Math.max(50, (width - right - halfWidth) / width * 100);
  const minY = Math.min(50, (top + halfHeight) / height * 100);
  const maxY = Math.max(50, (height - bottom - halfHeight) / height * 100);
  return { minX, maxX, minY, maxY };
}

/** @param {number} x @param {number} y @param {{minX?:number,maxX?:number,minY?:number,maxY?:number}} [bounds] @returns {{x:number,y:number}} */
export function clampPreviewPosition(x, y, bounds = {}) {
  const minX = typeof bounds.minX === 'number' && Number.isFinite(bounds.minX) ? bounds.minX : 0;
  const maxX = typeof bounds.maxX === 'number' && Number.isFinite(bounds.maxX) ? bounds.maxX : 100;
  const minY = typeof bounds.minY === 'number' && Number.isFinite(bounds.minY) ? bounds.minY : 0;
  const maxY = typeof bounds.maxY === 'number' && Number.isFinite(bounds.maxY) ? bounds.maxY : 100;
  return {
    x: Math.max(minX, Math.min(maxX, Number.isFinite(x) ? x : 50)),
    y: Math.max(minY, Math.min(maxY, Number.isFinite(y) ? y : 50))
  };
}

/** @param {FormatProfile | undefined} format @param {number} [sourceRatio] */
export function formatRatio(format, sourceRatio = 16 / 9) {
  switch (format?.key) {
    case 'portrait916': return 9 / 16;
    case 'landscape169': return 16 / 9;
    case 'square11': return 1;
    case 'portrait45': return 4 / 5;
    case 'custom': {
      const width = Number(format.width);
      const height = Number(format.height);
      return Number.isFinite(width) && Number.isFinite(height) && width > 0 && height > 0
        ? width / height
        : sourceRatio;
    }
    default: return sourceRatio;
  }
}

/** @param {number} ratio @param {number} [maxWidth] @param {number} [maxHeight] */
export function previewWidthForRatio(ratio, maxWidth = 720, maxHeight = 520) {
  const safeRatio = Number.isFinite(ratio) && ratio > 0 ? ratio : 16 / 9;
  return Math.min(maxWidth, maxHeight * safeRatio);
}

/** @param {SafeZoneKey} key @returns {Readonly<SafeZoneGuide> | null} */
export function safeZoneGuide(key) {
  if (key === 'off') return null;
  return SAFE_ZONES[key] ?? SAFE_ZONES.generic;
}

/** @param {FormatProfile | undefined} format @returns {'contain'|'cover'|'fill'} */
export function videoObjectFit(format) {
  if (format?.key === 'source' || format?.fit === 'preserve' || format?.fit === 'contain') return 'contain';
  if (format?.fit === 'stretch') return 'fill';
  return 'cover';
}

/** @param {Array<{start:number,end:number}>} words @param {number} time */
export function activeWordIndex(words, time) {
  const last = words.at(-1);
  if (!words.length || !last) return -1;
  if (time < words[0].start) return 0;
  if (time >= last.end) return words.length - 1;
  for (let index = words.length - 1; index >= 0; index--) {
    if (time >= words[index].start) return index;
  }
  return -1;
}

/** @param {string} text */
export function demoSubtitleWords(text) {
  const tokens = text.trim().split(/\s+/).filter(Boolean);
  const duration = tokens.length ? 1 / tokens.length : 0;
  return tokens.map((word, index) => ({ word, start: index * duration, end: (index + 1) * duration }));
}

/** @param {string} text @param {Array<{word:string,start:number,end:number}>} words */
export function previewSubtitleTokens(text, words = []) {
  const matches = [...text.matchAll(/\S+/gu)];
  const fallback = demoSubtitleWords(text);
  let previousEnd = 0;
  return matches.map((match, index) => {
    const whitespace = text.slice(previousEnd, match.index);
    previousEnd = match.index + match[0].length;
    const timing = words[index] ?? fallback[index] ?? { start: 0, end: 0 };
    return {
      word: match[0],
      start: timing.start,
      end: timing.end,
      separator: index === 0 ? '' : whitespace.includes('\n') ? '\n' : ' '
    };
  });
}

/** @param {number} time @param {number} start @param {number} end */
export function karaokeProgress(time, start, end) {
  const duration = Math.max(0.001, end - start);
  return Math.max(0, Math.min(100, (time - start) / duration * 100));
}

/** @param {number} elapsed @param {number} duration */
export function loopedPreviewTime(elapsed, duration) {
  const safeDuration = Number.isFinite(duration) && duration > 0 ? duration : 1;
  const safeElapsed = Number.isFinite(elapsed) ? Math.max(0, elapsed) : 0;
  return safeElapsed % safeDuration;
}

/** @param {number} value @param {number} displayHeight @param {number} outputHeight */
export function scalePreviewMetric(value, displayHeight, outputHeight) {
  return Math.max(0, value) * Math.max(0, displayHeight) / Math.max(1, outputHeight);
}

/** @template {{family:string,fullName?:string,weight?:number,italic?:boolean}} T @param {T[]} fonts @param {string} family @param {number} [weight] @param {boolean} [italic] @returns {T|null} */
export function customFontMatch(fonts, family, weight = 400, italic = false) {
  const key = family.split(',')[0].trim().toLocaleLowerCase();
  const exact = fonts.find(font => font.fullName?.toLocaleLowerCase() === key);
  if (exact) return exact;
  return fonts.filter(font => font.family.toLocaleLowerCase() === key).sort((a,b) => Number(Boolean(b.italic) === italic) - Number(Boolean(a.italic) === italic) || Math.abs((a.weight ?? 400) - weight) - Math.abs((b.weight ?? 400) - weight))[0] ?? null;
}

/** @param {{load:(query:string)=>Promise<unknown>,check:(query:string)=>boolean}|undefined} fontSet @param {{family:string,fullName?:string,fileName:string,weight:number,italic:boolean}} font */
export async function verifyCustomFont(fontSet, font) {
  if (!fontSet) return { status: 'unavailable', fileName: font.fileName };
  const family = font.fullName || font.family;
  const query = `${font.italic ? 'italic ' : ''}${font.weight || 400} 16px "${family.replaceAll('"', '\\"')}"`;
  try {
    await fontSet.load(query);
    return { status: fontSet.check(query) ? 'loaded' : 'fallback', fileName: font.fileName };
  } catch {
    return { status: 'fallback', fileName: font.fileName };
  }
}
