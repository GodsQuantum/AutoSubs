/** @typedef {{ name: string }} NamedFile */

const subtitleExtensions = new Set(['srt', 'ass', 'ssa', 'json']);

/** @param {string} name @returns {string} */
const ext = (name) => name.includes('.') ? name.split('.').pop()?.toLowerCase() ?? '' : '';

/** @param {string} name @returns {string} */
const stem = (name) => name.replace(/\.[^.]+$/, '');

/** @param {string} name @returns {string} */
const lower = (name) => name.toLowerCase();

/** @param {string} name @returns {boolean} */
const isInternalStaging = (name) => {
  const value = lower(name);
  return ext(value) === 'partial' || value.includes('.partial-') || value.endsWith('.uploading') || value.endsWith('_words.json');
};

/**
 * Pair media candidates with basename-matching subtitle sidecars.
 * @param {NamedFile[]} files
 * @returns {Array<{ video: NamedFile, sidecar: NamedFile | undefined }>}
 */
export function pairFiles(files) {
  const candidates = files.filter((file) => !isInternalStaging(file.name));
  /** @type {Map<string, NamedFile>} */
  const sidecars = new Map();
  for (const file of candidates) {
    if (subtitleExtensions.has(ext(file.name))) sidecars.set(stem(file.name).toLowerCase(), file);
  }
  return candidates
    .filter((file) => !subtitleExtensions.has(ext(file.name)))
    .map((video) => ({ video, sidecar: sidecars.get(stem(video.name).toLowerCase()) }));
}
