/** @param {Response} response */
export async function parseApiResponse(response) { if (response.status === 204) return undefined; const text = await response.text(); return text.trim() ? JSON.parse(text) : undefined; }
