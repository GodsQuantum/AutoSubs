export function createLatestRequestGate() {
  let version = 0;
  return {
    begin() {
      version += 1;
      return version;
    },
    /** @param {number} requestVersion */
    isCurrent(requestVersion) {
      return requestVersion === version;
    },
    invalidate() {
      version += 1;
    }
  };
}

/** @param {string} path */
export function pathLabel(path) {
  const trimmed = String(path || '').replace(/\/+$/, '');
  if (!trimmed) return '/';
  const parts = trimmed.split('/').filter(Boolean);
  return parts.at(-1) || '/';
}

/** @param {string} path @param {string[]} roots */
export function rootForPath(path, roots) {
  const matches = roots
    .filter((root) => path === root || path.startsWith(root.endsWith('/') ? root : root + '/'))
    .sort((a, b) => b.length - a.length);
  return matches[0] || '';
}
