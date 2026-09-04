import { parseApiResponse } from './api-response.js';
import type { Asset, Brand, BrowseResponse, Capabilities, FontFace, Job, JobOutro, Preset, SettingsView, SubtitleLine, Workflow, FormatProfile } from './types';

export class ApiError extends Error {
  constructor(public status: number, message: string) { super(message); }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const headers = new Headers(init?.headers);
  if (init?.body && !(init.body instanceof FormData) && !(init.body instanceof Blob)) headers.set('Content-Type', 'application/json');
  const response = await fetch(path, { ...init, headers });
  if (!response.ok) {
    let message = `HTTP ${response.status}`;
    try { const body = await response.json(); message = body?.error?.message ?? message; } catch { /* non-JSON error */ }
    throw new ApiError(response.status, message);
  }
  return parseApiResponse(response) as Promise<T>;
}

export const api = {
  jobs: () => request<Job[]>('/api/v1/jobs'),
  job: (id: string) => request<Job>(`/api/v1/jobs/${id}`),
  createFromPath: (path: string, sidecarPath?: string, presetId?: string) => request<Job>('/api/v1/jobs/from-path', {
    method: 'POST', body: JSON.stringify({ path, sidecarPath: sidecarPath || undefined, presetId: presetId || undefined })
  }),
  prepare: (id: string) => request<{accepted:boolean}>(`/api/v1/jobs/${id}/prepare`, { method: 'POST' }),
  render: (id: string) => request<{accepted:boolean}>(`/api/v1/jobs/${id}/render`, { method: 'POST' }),
  cancel: (id: string) => request<Job>(`/api/v1/jobs/${id}/cancel`, { method: 'POST' }),
  retranscribe: (id: string) => request<{accepted:boolean}>(`/api/v1/jobs/${id}/retranscribe`, { method: 'POST' }),
  deleteJob: (id: string) => request<void>(`/api/v1/jobs/${id}`, { method: 'DELETE' }),
  updateJob: (id: string, body: { presetId?: string | null; format?: FormatProfile; outro?: JobOutro }) => request<Job>(`/api/v1/jobs/${id}`, { method: 'PUT', body: JSON.stringify(body) }),
  subtitles: (id: string) => request<SubtitleLine[]>(`/api/v1/jobs/${id}/subtitles`),
  saveSubtitles: (id: string, lines: SubtitleLine[]) => request<{lines:SubtitleLine[]; repairedLineOverlaps:number; retimedWordLines:number; droppedEmptyLines:number}>(`/api/v1/jobs/${id}/subtitles`, { method: 'PUT', body: JSON.stringify(lines) }),
  regroup: (id: string, maxChars: number, maxLines: number) => request<SubtitleLine[]>(`/api/v1/jobs/${id}/regroup`, { method: 'POST', body: JSON.stringify({ maxChars, maxLines }) }),
  setSidecar: (id: string, path: string) => request<Job>(`/api/v1/jobs/${id}/sidecar`, { method: 'PUT', body: JSON.stringify({ path }) }),
  removeSidecar: (id: string) => request<Job>(`/api/v1/jobs/${id}/sidecar`, { method: 'DELETE' }),
  uploadSidecar: async (id: string, file: File) => { const form = new FormData(); form.append('file', file); return request<Job>(`/api/v1/jobs/${id}/sidecar/upload`, { method: 'POST', body: form }); },
  presets: () => request<Preset[]>('/api/v1/presets'),
  savePreset: (p: Preset) => request<Preset>('/api/v1/presets', { method: 'POST', body: JSON.stringify(p) }),
  deletePreset: (id: string) => request<void>(`/api/v1/presets/${id}`, { method: 'DELETE' }),
  brands: () => request<Brand[]>('/api/v1/brands'),
  saveBrand: (b: Brand) => request<Brand>('/api/v1/brands', { method: 'POST', body: JSON.stringify(b) }),
  deleteBrand: (id: string) => request<void>(`/api/v1/brands/${id}`, { method: 'DELETE' }),
  workflows: () => request<Workflow[]>('/api/v1/workflows'),
  saveWorkflow: (w: Workflow) => request<Workflow>('/api/v1/workflows', { method: 'POST', body: JSON.stringify(w) }),
  deleteWorkflow: (id: string) => request<void>(`/api/v1/workflows/${id}`, { method: 'DELETE' }),
  settings: () => request<SettingsView>('/api/v1/settings'),
  saveSettings: (body: unknown) => request<SettingsView>('/api/v1/settings', { method: 'PUT', body: JSON.stringify(body) }),
  models: (endpoint: string, apiKey = '') => request<{models:string[]}>('/api/v1/models', { method:'POST', body: JSON.stringify({ endpoint, apiKey }) }),
  assets: () => request<Asset[]>('/api/v1/assets'),
  importAsset: (path: string) => request<Asset>('/api/v1/assets/import', { method: 'POST', body: JSON.stringify({ path }) }),
  uploadAsset: async (file: File) => { const f = new FormData(); f.append('file', file); return request<Asset>('/api/v1/assets', { method: 'POST', body: f }); },
  deleteAsset: (id:string) => request<void>(`/api/v1/assets/${id}`, { method:'DELETE' }),
  browse: (path: string, mode: 'file'|'directory'|'any' = 'any', extensions = '') => request<BrowseResponse>(`/api/v1/browse?path=${encodeURIComponent(path)}&mode=${mode}&extensions=${encodeURIComponent(extensions)}`),
  capabilities: () => request<Capabilities>('/api/v1/capabilities'),
  fonts: () => request<FontFace[]>('/api/v1/fonts')
};

export const subtitleExportUrl = (id:string, format:'srt'|'ass'|'json') => `/api/v1/jobs/${id}/subtitles/${format}`;
export const videoUrl = (id:string) => `/api/v1/jobs/${id}/video`;
export const sourceVideoUrl = (id:string) => `/api/v1/jobs/${id}/video/source`;
export const renderedVideoUrl = (id:string) => `/api/v1/jobs/${id}/video/output`;
export const assetUrl = (id:string) => `/api/v1/assets/${id}/content`;

const TUS = '1.0.0';
const chunkSize = 8 * 1024 * 1024;
const fingerprint = (file: File) => `autosubs:tus:${file.name}:${file.size}:${file.lastModified}`;

async function headUpload(url: string): Promise<{offset:number; jobId:string}> {
  const response = await fetch(url, { method: 'HEAD', headers: { 'Tus-Resumable': TUS } });
  if (!response.ok) throw new Error('resume unavailable');
  return { offset: Number(response.headers.get('Upload-Offset') || 0), jobId: response.headers.get('Upload-Final-Job') || '' };
}

export async function tusUpload(file: File, onProgress: (value:number) => void, signal?: AbortSignal): Promise<string> {
  const key = fingerprint(file);
  let url = localStorage.getItem(key) ?? '';
  let offset = 0;
  let jobId = '';
  if (url) {
    try { const head = await headUpload(url); offset = head.offset; jobId = head.jobId; }
    catch { localStorage.removeItem(key); url = ''; }
  }
  if (jobId && offset === file.size) { localStorage.removeItem(key); onProgress(100); return jobId; }
  if (!url) {
    const meta = btoa(unescape(encodeURIComponent(file.name)));
    const response = await fetch('/api/v1/uploads', { method: 'POST', signal, headers: { 'Tus-Resumable': TUS, 'Upload-Length': String(file.size), 'Upload-Metadata': `filename ${meta}` } });
    if (!response.ok) throw new Error(`Upload creation failed (${response.status})`);
    url = response.headers.get('Location') ?? '';
    if (!url) throw new Error('Upload Location missing');
    localStorage.setItem(key, url);
  }
  while (offset < file.size) {
    if (signal?.aborted) throw new DOMException('Upload cancelled', 'AbortError');
    const end = Math.min(offset + chunkSize, file.size);
    const response = await fetch(url, { method: 'PATCH', signal, headers: { 'Tus-Resumable': TUS, 'Upload-Offset': String(offset), 'Content-Type': 'application/offset+octet-stream' }, body: file.slice(offset, end) });
    if (!response.ok) throw new Error(`Upload failed (${response.status})`);
    offset = Number(response.headers.get('Upload-Offset') ?? end);
    jobId = response.headers.get('Upload-Final-Job') ?? jobId;
    onProgress(file.size ? Math.round(offset / file.size * 100) : 100);
  }
  if (!jobId) jobId = (await headUpload(url)).jobId;
  if (!jobId) throw new Error('Upload completed but job id is missing');
  localStorage.removeItem(key);
  return jobId;
}
