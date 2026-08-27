const subtitleExtensions = new Set(['srt', 'ass', 'ssa', 'json']);
const ext = (name) => name.includes('.') ? name.split('.').pop().toLowerCase() : '';
const stem = (name) => name.replace(/\.[^.]+$/, '');
const lower = (name) => name.toLowerCase();
const isInternalStaging = (name) => {
  const value = lower(name);
  return ext(value) === 'partial' || value.includes('.partial-') || value.endsWith('.uploading') || value.endsWith('_words.json');
};

export function pairFiles(files) {
  const candidates = files.filter((file) => !isInternalStaging(file.name));
  const sidecars = new Map();
  for (const file of candidates) {
    if (subtitleExtensions.has(ext(file.name))) sidecars.set(stem(file.name).toLowerCase(), file);
  }
  return candidates
    .filter((file) => !subtitleExtensions.has(ext(file.name)))
    .map((video) => ({ video, sidecar: sidecars.get(stem(video.name).toLowerCase()) }));
}
