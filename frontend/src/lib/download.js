/** @param {string} value @param {string} fallback */
function safeFilename(value, fallback) {
  const clean = value.replace(/[\\/\0]/g, '_').trim();
  return clean || fallback;
}

/** @param {string|null} value @param {string} fallback */
export function filenameFromDisposition(value, fallback) {
  if (!value) return fallback;
  const encoded = value.match(/filename\*=UTF-8''([^;]+)/i)?.[1];
  if (encoded) {
    try { return safeFilename(decodeURIComponent(encoded), fallback); }
    catch { return fallback; }
  }
  const plain = value.match(/filename="([^"]+)"/i)?.[1] ?? value.match(/filename=([^;]+)/i)?.[1];
  return plain ? safeFilename(plain, fallback) : fallback;
}

/** @param {string} url @param {string} fallback @param {typeof fetch} fetchImpl */
export async function fetchDownload(url, fallback, fetchImpl = fetch) {
  const response = await fetchImpl(url);
  if (!response.ok) {
    let message = `HTTP ${response.status}`;
    try {
      const body = JSON.parse(await response.text());
      message = body?.error?.message ?? message;
    } catch {
      // Keep the status when the server did not return JSON.
    }
    throw new Error(message);
  }
  return {
    blob: await response.blob(),
    filename: filenameFromDisposition(response.headers.get('content-disposition'), fallback)
  };
}

/** @param {{blob:Blob;filename:string}} payload */
export function saveDownload(payload, documentRef = document, urlApi = URL) {
  const href = urlApi.createObjectURL(payload.blob);
  const link = documentRef.createElement('a');
  link.href = href;
  link.download = payload.filename;
  link.hidden = true;
  documentRef.body.appendChild(link);
  link.click();
  link.remove();
  urlApi.revokeObjectURL(href);
}
