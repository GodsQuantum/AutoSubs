const MODEL_PREFS = [
  'handy-computer/whisper-large-v3-turbo-gguf',
  'deepdml/faster-whisper-large-v3-turbo-ct2',
];

/** @param {string} value */
export function normalizeSpeachesUrl(value) {
  const parsed = new URL(String(value || '').trim());
  if (!['http:', 'https:'].includes(parsed.protocol)) {
    throw new Error('Cloud9-Speaches must use HTTP or HTTPS');
  }
  if (parsed.username || parsed.password || parsed.search || parsed.hash) {
    throw new Error('Cloud9-Speaches address must not contain credentials or parameters');
  }
  if (!['', '/', '/v1', '/v1/', '/v1/audio/transcriptions'].includes(parsed.pathname)) {
    throw new Error('Use the server address, not an arbitrary API path');
  }
  const h = parsed.hostname.toLowerCase().replace(/^\[|\]$/g, '');
  const local = h === 'localhost' || h === '127.0.0.1' || h === '::1'
    || h.endsWith('.local') || /^10\.\d+\.\d+\.\d+$/.test(h)
    || /^192\.168\.\d+\.\d+$/.test(h)
    || /^172\.(1[6-9]|2\d|3[01])\.\d+\.\d+$/.test(h);
  if (parsed.protocol === 'http:' && !local) {
    throw new Error('Use HTTPS outside localhost/private LAN');
  }
  return parsed.origin;
}

/** @param {string} endpoint @param {typeof fetch} fetcher */
async function readJson(endpoint, fetcher) {
  const response = await fetcher(endpoint, { method: 'GET', signal: AbortSignal.timeout(8000) });
  if (!response.ok) throw new Error('Cloud9-Speaches endpoint failed (' + response.status + '): ' + endpoint);
  return response.json();
}

/** @param {string} value @param {typeof fetch} [fetcher] */
export async function probeSpeaches(value, fetcher = fetch) {
  const origin = normalizeSpeachesUrl(value);
  const [health, models, alignment] = await Promise.all([
    readJson(origin + '/health', fetcher),
    readJson(origin + '/v1/models', fetcher),
    readJson(origin + '/v1/audio/alignment/health', fetcher),
  ]);
  if (!health || !Array.isArray(models?.data)) throw new Error('Invalid Cloud9-Speaches API response');
  const ids = /** @type {{id?:string}[]} */ (models.data).map(x => x?.id).filter(x => typeof x === 'string');
  const model = MODEL_PREFS.find(x => ids.includes(x));
  if (!model) throw new Error('No supported Whisper Turbo model is installed');
  if (alignment?.status !== 'ok' || !/** @type {{id:string,available:boolean}[]} */ (alignment.models)?.some(x => x?.id === 'fastconformer-fr-q4' && x?.available)) {
    throw new Error('FastConformer acoustic alignment is not available');
  }
  return {
    origin,
    transcriptionUrl: origin + '/v1/audio/transcriptions',
    alignmentUrl: origin + '/align',
    model,
    alignmentModel: 'fastconformer-fr-q4',
    backend: alignment.backend || 'unknown',
    backendVerified: alignment.backend_selection !== 'configured_not_hardware_probed',
  };
}

/** Probe through this application's local Rust API, not across browser origins.
 * The Rust server pins validated private IPs before contacting Cloud9-Speaches.
 * @param {string} value
 * @param {typeof fetch} [fetcher]
 */
export async function probeSpeachesViaApp(value, fetcher = fetch) {
  const origin = normalizeSpeachesUrl(value);
  const response = await fetcher('/api/v1/speaches/probe', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ url: origin }),
    signal: AbortSignal.timeout(12000),
  });
  if (!response.ok) {
    const data = await response.json().catch(() => ({}));
    throw new Error(data?.error?.message || ('Cloud9-Speaches probe failed (' + response.status + ')'));
  }
  return response.json();
}
