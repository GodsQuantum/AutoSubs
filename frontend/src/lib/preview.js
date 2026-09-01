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


/** @param {number} x @param {number} y @returns {{x:number,y:number}} */
export function clampPreviewPosition(x, y) {
  return {
    x: Math.max(0, Math.min(100, Number.isFinite(x) ? x : 50)),
    y: Math.max(0, Math.min(100, Number.isFinite(y) ? y : 50))
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
  return words.findIndex(word => time >= word.start && time < word.end);
}

/** @param {string} text */
export function demoSubtitleWords(text) {
  const tokens = text.trim().split(/\s+/).filter(Boolean);
  const duration = tokens.length ? 1 / tokens.length : 0;
  return tokens.map((word, index) => ({ word, start: index * duration, end: (index + 1) * duration }));
}
