import React, { useState, useEffect, useRef } from 'react';
import { 
  Settings, Play, Pause, Square, Save, Trash2, Upload, Video, Layers, 
  CheckCircle, Clock, Edit3, X, Wand2, Plus, AlertTriangle, Download, 
  Copy, Sparkles, Search, Check, Sliders, Smartphone, Cpu, Rewind, FastForward
} from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';

export interface Preset {
  name: string;
  animationStyle: 'pop' | 'karaoke' | 'fade' | 'slide-up' | 'bounce' | 'none';
  size: number;
  positionX: number;
  positionY: number;
  baseColor: string;
  outlineColor: string;
  highlightColor: string;
  fontFamily: string;
  uppercase: boolean;
  outlineThickness: number;
  shadowThickness?: number;
  shadowColor?: string;
  borderStyle?: number;
  floating?: boolean;
  maxChars?: number;
  maxLines?: number;
  wobbleSpeed?: number;
  bold?: boolean;
  italic?: boolean;
  matchKeywords?: string;
  lineSpacing?: number;
  aspectRatio?: '9:16' | '16:9' | '1:1' | '4:5';
  brand?: string;
  outroVideo?: string;
}

export interface SubtitleWord {
  word: string;
  start: number;
  end: number;
}

export interface SubtitleLine {
  id: number;
  start: number;
  end: number;
  text: string;
  words?: SubtitleWord[];
}

export interface BatchFile {
  id: string;
  file: File;
  subtitleFile?: File;
  originalName: string;
  status: 'pending' | 'uploading' | 'transcribing' | 'ready' | 'burning' | 'done' | 'error' | 'cancelled';
  lines?: SubtitleLine[];
  progress?: number;
  selected?: boolean;
  uploaded?: boolean;
  error?: string;
  videoUrl?: string;
}

const defaultPreset: Preset = {
  name: 'Défaut',
  animationStyle: 'pop',
  size: 26,
  positionX: 50,
  positionY: 66,
  baseColor: '#ffffff',
  outlineColor: '#000000',
  highlightColor: '#00d2ff',
  fontFamily: 'Roboto',
  uppercase: true,
  outlineThickness: 2.5,
  shadowThickness: 1.5,
  shadowColor: '#000000',
  borderStyle: 1,
  floating: false,
  maxChars: 25,
  maxLines: 2,
  wobbleSpeed: 1,
  bold: true,
  italic: false,
  aspectRatio: '9:16'
};

const DEFAULT_FONTS = ['Roboto', 'Montserrat', 'Anton', 'League Spartan', 'Liberation Sans', 'DejaVu Sans', 'Arial', 'Outfit', 'Inter'];

interface Toast {
  id: string;
  type: 'success' | 'error' | 'info' | 'warning';
  message: string;
}

const getAspectRatioStyle = (ratio?: string) => {
  switch (ratio) {
    case '16:9': return { aspectRatio: '16/9', maxWidth: '500px', maxHeight: '300px' };
    case '1:1': return { aspectRatio: '1/1', maxWidth: '320px', maxHeight: '320px' };
    case '4:5': return { aspectRatio: '4/5', maxWidth: '300px', maxHeight: '350px' };
    case '9:16':
    default: return { aspectRatio: '9/16', maxWidth: '280px', maxHeight: '500px' };
  }
};

export default function App() {
  const [workflows, setWorkflows] = useState<any[]>([]);
  const [browserOpen, setBrowserOpen] = useState(false);
  const [browserPath, setBrowserPath] = useState('/');
  const [browserEntries, setBrowserEntries] = useState<{name:string, path:string, is_dir:bool}[]>([]);
  const [browserTarget, setBrowserTarget] = useState<{wfId: string, field: string} | null>(null);

  const fetchBrowser = async (path: string) => {
    try {
      const res = await fetch(`/api/browse?path=${encodeURIComponent(path)}`);
      const data = await res.json();
      setBrowserPath(data.current_path);
      setBrowserEntries(data.entries);
    } catch(e) {}
  };

  const openBrowser = (wfId: string, field: string, currentPath: string) => {
    setBrowserTarget({ wfId, field });
    fetchBrowser(currentPath || '/');
    setBrowserOpen(true);
  };

  const [activeTab, setActiveTab] = useState<'batch' | 'presets' | 'workflows' | 'settings'>('batch');
  const [presets, setPresets] = useState<Preset[]>([]);
  const [currentPreset, setCurrentPreset] = useState<Preset>(defaultPreset);
  const [isEditingPreset, setIsEditingPreset] = useState(false);
  const [fonts, setFonts] = useState<string[]>(DEFAULT_FONTS);
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [availableLocalModels, setAvailableLocalModels] = useState<string[]>([]);
  const [outros, setOutros] = useState<string[]>([]);
  const [toasts, setToasts] = useState<Toast[]>([]);

  const [batchFiles, setBatchFiles] = useState<BatchFile[]>([]);
  const [globalPreset, setGlobalPreset] = useState<string>('');
  const [globalOutro, setGlobalOutro] = useState<string>('preset_default');
  const fileInputRef = useRef<HTMLInputElement>(null);

  const [settings, setSettings] = useState({
    transcriptionUrl: '',
    transcriptionApiKey: '',
    transcriptionModel: '',
    language: 'fr',
    localTranscriptionEnabled: false,
    localFallbackEnabled: true,
    localTranscriptionUrl: '',
    localTranscriptionApiKey: '',
    localTranscriptionModel: '',
    llmEnabled: false,
    llmEndpoint: '',
    llmApiKey: '',
    llmModel: '',
    llmPrompt: '',
    hardwareAccel: 'auto'
  });
  const [activeSettingsTab, setActiveSettingsTab] = useState<'external' | 'local' | 'llm' | 'performance'>('external');

  const [editingFileId, setEditingFileId] = useState<string | null>(null);
  const [editingLines, setEditingLines] = useState<SubtitleLine[]>([]);
  const [currentTime, setCurrentTime] = useState<number>(0);
  const [isPlaying, setIsPlaying] = useState<boolean>(false);
  const [shiftMs, setShiftMs] = useState<number>(0);
  const [editorMaxChars, setEditorMaxChars] = useState<number>(25);
  const [editorMaxLines, setEditorMaxLines] = useState<number>(2);
  const [searchQuery, setSearchQuery] = useState<string>('');
  const [replaceQuery, setReplaceQuery] = useState<string>('');
  const [showSearchReplace, setShowSearchReplace] = useState<boolean>(false);
  const [previewSampleText, setPreviewSampleText] = useState<string>("C'EST UN EXEMPLE");
  
  const videoPlayerRef = useRef<HTMLVideoElement>(null);
  const previewBoxRef = useRef<HTMLDivElement>(null);

  const addToast = (type: Toast['type'], message: string) => {
    const id = Math.random().toString(36).substring(7);
    setToasts(prev => [...prev, { id, type, message }]);
    setTimeout(() => {
      setToasts(prev => prev.filter(t => t.id !== id));
    }, 4000);
  };

  const fixOverlaps = (lines: SubtitleLine[], gapMs: number = 10): SubtitleLine[] => {
    if (!lines || lines.length === 0) return [];
    const gap = Math.max(0.005, gapMs / 1000);
    const minLineDuration = 0.08;

    const sorted = lines
      .map((l, index) => ({
        id: l.id !== undefined ? l.id : index,
        start: Math.max(0, Number(l.start) || 0),
        end: Math.max(0, Number(l.end) || 0),
        text: (l.text || '').trim(),
        words: l.words
      }))
      .filter(l => l.text.length > 0)
      .sort((a, b) => a.start - b.start || a.end - b.end);

    if (sorted.length === 0) return [];

    for (let i = 0; i < sorted.length; i++) {
      if (sorted[i].end < sorted[i].start + minLineDuration) {
        sorted[i].end = sorted[i].start + minLineDuration;
      }
    }

    for (let i = 0; i < sorted.length - 1; i++) {
      const current = sorted[i];
      const next = sorted[i + 1];

      if (current.end > next.start - gap) {
        const targetEnd = next.start - gap;
        if (targetEnd >= current.start + minLineDuration) {
          current.end = targetEnd;
        } else {
          current.end = current.start + minLineDuration;
          next.start = current.end + gap;
          if (next.end < next.start + minLineDuration) {
            next.end = next.start + minLineDuration;
          }
        }
      }
    }

    return sorted.map((line, idx) => {
      const rawTokens = line.text.trim().split(/\s+/).filter(w => w.length > 0);
      const totalChars = rawTokens.reduce((s, t) => s + Math.max(1, t.length), 0);
      const duration = line.end - line.start;
      let offset = 0;
      const words: SubtitleWord[] = rawTokens.map((t, i) => {
        const charFraction = Math.max(1, t.length) / totalChars;
        const wDur = Math.max(0.03, charFraction * duration);
        const wStart = line.start + offset;
        const isLast = i === rawTokens.length - 1;
        const wEnd = isLast ? line.end : Math.min(line.end - 0.01, wStart + wDur);
        offset += wDur;
        return { word: t, start: wStart, end: Math.max(wStart + 0.02, wEnd) };
      });

      return {
        ...line,
        id: idx,
        words
      };
    });
  };

  useEffect(() => {
    fetchPresets();
    fetchWorkflows();
    fetchSettings();
    fetchFonts();
    fetchActiveJobs();
    fetchOutros();

    const eventSource = new EventSource('/api/events');
    eventSource.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        setBatchFiles(prev => prev.map(f => {
          if (f.id === data.id) {
            return { ...f, status: data.status, progress: data.progress, error: data.error };
          }
          return f;
        }));
      } catch {}
    };

    return () => eventSource.close();
  }, []);

  useEffect(() => {
    if (currentPreset.fontFamily) {
      const fontName = currentPreset.fontFamily;
      if (!fonts.includes(fontName) && !document.getElementById(`google-font-${fontName}`)) {
        const link = document.createElement('link');
        link.id = `google-font-${fontName}`;
        link.rel = 'stylesheet';
        link.href = `https://fonts.googleapis.com/css2?family=${fontName.replace(/ /g, '+')}:ital,wght@0,400;0,700;1,400;1,700&display=swap`;
        document.head.appendChild(link);
      }
    }
  }, [currentPreset.fontFamily, fonts]);

  useEffect(() => {
    if (!videoPlayerRef.current || !editingFileId) return;
    const video = videoPlayerRef.current;
    
    const handleTimeUpdate = () => {
      setCurrentTime(video.currentTime);
    };

    const handlePlay = () => setIsPlaying(true);
    const handlePause = () => setIsPlaying(false);

    video.addEventListener('timeupdate', handleTimeUpdate);
    video.addEventListener('play', handlePlay);
    video.addEventListener('pause', handlePause);

    return () => {
      video.removeEventListener('timeupdate', handleTimeUpdate);
      video.removeEventListener('play', handlePlay);
      video.removeEventListener('pause', handlePause);
    };
  }, [editingFileId]);

  
  const fetchWorkflows = async () => {
    try {
      const res = await fetch('/api/workflows');
      if (res.ok) setWorkflows(await res.json());
    } catch {}
  };
const fetchPresets = async () => {
    try {
      const res = await 
    fetch('/api/outros').then(r => r.json()).then(data => setOutros(data)).catch(() => {});
    fetch('/api/presets');
      if (res.ok) {
        const data = await res.json();
        if (Array.isArray(data) && data.length > 0) {
          setPresets(data);
          if (!globalPreset) setGlobalPreset(data[0].name);
        }
      }
    } catch {}
  };

  const fetchSettings = async () => {
    try {
      const res = await fetch('/api/settings');
      if (res.ok) {
        const data = await res.json();
        setSettings(data);
        if (data.transcriptionUrl) fetchAvailableModels(data.transcriptionUrl, data.transcriptionApiKey, false, false);
        if (data.localTranscriptionUrl) fetchAvailableModels(data.localTranscriptionUrl, data.localTranscriptionApiKey, false, true);
      }
    } catch {}
  };

  const fetchFonts = async () => {
    try {
      const res = await fetch('/api/fonts');
      if (res.ok) {
        const customFonts = await res.json();
        const fontNames = customFonts.map((f: string) => f.replace(/\.[^/.]+$/, ""));
        const style = document.createElement("style");

        let fontFaceRules = "";

        for (const font of customFonts) {

          const fontName = font.replace(/\.[^/.]+$/, "");

          fontFaceRules += `@font-face { font-family: "${fontName}"; src: url("/fonts/${font}"); }\n`;

        }

        style.innerHTML = fontFaceRules;

        document.head.appendChild(style);
        setFonts([...new Set([...DEFAULT_FONTS, ...fontNames])]);
      }
    } catch {}
  };

  const fetchOutros = async () => {
    try {
      const res = await fetch('/api/outros');
      if (res.ok) setOutros(await res.json());
    } catch {}
  };

  const fetchActiveJobs = async () => {
    try {
      const res = await fetch('/api/active-jobs');
      if (res.ok) {
        const jobs = await res.json();
        if (jobs && jobs.length > 0) {
          setBatchFiles(prev => {
            const currentIds = new Set(prev.map(f => f.id));
            const newJobs: BatchFile[] = [];
            jobs.forEach((job: any) => {
              if (!currentIds.has(job.id)) {
                newJobs.push({
                  id: job.id,
                  originalName: job.originalName,
                  status: job.status,
                  progress: job.progress,
                  lines: job.lines,
                  file: new File([], job.originalName),
                  uploaded: true,
                  selected: true
                });
              }
            });
            return [...prev, ...newJobs];
          });
        }
      }
    } catch {}
  };

  const fetchAvailableModels = async (url: string, apiKey: string, showNotification = false, isLocal = false) => {
    if (!url) return;
    try {
      const res = await fetch('/api/models', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ transcriptionUrl: url, transcriptionApiKey: apiKey })
      });
      if (res.ok) {
        const models = await res.json();
        if (isLocal) {
          setAvailableLocalModels(models);
        } else {
          setAvailableModels(models);
        }
        if (showNotification) {
          addToast('success', `${models.length} modèle(s) détecté(s)`);
        }
      }
    } catch {
      if (showNotification) addToast('error', 'Erreur réseau lors de la détection des modèles');
    }
  };

  const handleSaveSettings = async () => {
    try {
      const res = await fetch('/api/settings', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(settings)
      });
      if (res.ok) {
        addToast('success', 'Paramètres enregistrés avec succès');
      } else {
        addToast('error', 'Erreur lors de la sauvegarde des paramètres');
      }
    } catch {
      addToast('error', 'Erreur réseau lors de la sauvegarde');
    }
  };

  const handleSavePreset = async () => {
    if (!currentPreset.name.trim()) return addToast('warning', 'Veuillez renseigner un nom pour le preset');
    try {
      const res = await fetch('/api/presets', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(currentPreset),
      });
      if (res.ok) {
        await fetchPresets();
    fetchWorkflows();
        setIsEditingPreset(false);
        addToast('success', `Preset "${currentPreset.name}" enregistré`);
      }
    } catch {
      addToast('error', 'Erreur lors de la sauvegarde du preset');
    }
  };

  const handleDeletePreset = async (name: string) => {
    if (name === 'Défaut') return addToast('warning', 'Le preset "Défaut" est protégé');
    if (!window.confirm(`Supprimer définitivement le preset "${name}" ?`)) return;
    try {
      const res = await fetch(`/api/presets/${encodeURIComponent(name)}`, { method: 'DELETE' });
      if (res.ok) {
        await fetchPresets();
    fetchWorkflows();
        setIsEditingPreset(false);
        addToast('info', `Preset "${name}" supprimé`);
      }
    } catch {}
  };

  const handleDuplicatePreset = (preset: Preset) => {
    const duplicated: Preset = {
      ...preset,
      name: `${preset.name} (Copie)`
    };
    setCurrentPreset(duplicated);
    setIsEditingPreset(true);
  };

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (!e.target.files) return;
    const files = Array.from(e.target.files) as File[];

    const fileGroups: Record<string, { video?: File; subtitle?: File }> = {};

    for (const file of files) {
      const ext = file.name.substring(file.name.lastIndexOf('.')).toLowerCase();
      const base = file.name.substring(0, file.name.lastIndexOf('.'));

      if (['.mp4', '.mov', '.avi', '.mkv', '.webm'].includes(ext)) {
        if (!fileGroups[base]) fileGroups[base] = { video: file };
        else fileGroups[base].video = file;
      } else if (['.srt', '.ass', '.json'].includes(ext)) {
        if (!fileGroups[base]) fileGroups[base] = { subtitle: file };
        else fileGroups[base].subtitle = file;
      }
    }

    const newBatch: BatchFile[] = [];
    Object.entries(fileGroups).forEach(([_, group]) => {
      if (!group.video) return;
      const fileId = Math.random().toString(36).substring(7);
      const localVideoUrl = URL.createObjectURL(group.video);

      const batchItem: BatchFile = {
        id: fileId,
        file: group.video,
        subtitleFile: group.subtitle,
        originalName: group.video.name,
        status: group.subtitle ? 'ready' : 'pending',
        selected: true,
        videoUrl: localVideoUrl
      };

      if (group.subtitle) {
        const reader = new FileReader();
        reader.onload = (ev) => {
          const content = ev.target?.result as string;
          const ext = group.subtitle!.name.split('.').pop()?.toLowerCase();
          let parsed: SubtitleLine[] = [];

          if (ext === 'srt') {
            const blocks = content.replace(/\r\n/g, '\n').trim().split('\n\n');
            blocks.forEach(block => {
              const parts = block.split('\n');
              if (parts.length >= 3) {
                const [startStr, endStr] = parts[1].split(' --> ');
                const parseTime = (ts: string) => {
                  const [hms, ms] = ts.split(',');
                  const [h, m, s] = hms.split(':').map(Number);
                  return h * 3600 + m * 60 + s + (Number(ms) || 0) / 1000;
                };
                parsed.push({
                  id: parsed.length,
                  start: parseTime(startStr),
                  end: parseTime(endStr),
                  text: parts.slice(2).join('\n')
                });
              }
            });
          }
          parsed = fixOverlaps(parsed);
          setBatchFiles(prev => prev.map(f => f.id === fileId ? { ...f, lines: parsed, status: 'ready' } : f));
        };
        reader.readAsText(group.subtitle);
      }

      newBatch.push(batchItem);
    });

    setBatchFiles(prev => [...prev, ...newBatch]);
    if (fileInputRef.current) fileInputRef.current.value = '';
    addToast('info', `${newBatch.length} vidéo(s) ajoutée(s) à la file`);
  };

  const processFileTranscription = async (batchFile: BatchFile): Promise<BatchFile | null> => {
    setBatchFiles(prev => prev.map(f => f.id === batchFile.id ? { ...f, status: 'uploading' } : f));
    
    const formData = new FormData();
    formData.append('video', batchFile.file);
    if (batchFile.lines && batchFile.lines.length > 0) {
      formData.append('lines', JSON.stringify(fixOverlaps(batchFile.lines)));
    } else if (batchFile.subtitleFile) {
      formData.append('subtitle', batchFile.subtitleFile);
    }

    const defaultP = presets.find(p => p.name === 'Défaut') || defaultPreset;
    const globalP = presets.find(p => p.name === globalPreset);
    let filePreset = globalP || defaultP;

    const match = batchFile.originalName.match(/\(([^)]+)\)/);
    if (match) {
      const extracted = match[1];
      const found = presets.find(p => p.name === extracted);
      if (found) filePreset = found;
    }

    if (filePreset?.maxChars) formData.append('maxChars', filePreset.maxChars.toString());
    if (filePreset?.maxLines) formData.append('maxLines', filePreset.maxLines.toString());

    try {
      const res = await fetch('/api/upload-and-transcribe', {
        method: 'POST',
        body: formData,
      });
      if (!res.ok) {
        const errData = await res.json();
        throw new Error(errData.error || 'Erreur transcription');
      }
      const data = await res.json();
      const safeLines = fixOverlaps(data.lines || []);

      setBatchFiles(prev => prev.map(f => 
        f.id === batchFile.id ? { ...f, id: data.id, status: 'ready', lines: safeLines, uploaded: true } : f
      ));

      return { ...batchFile, id: data.id, status: 'ready', lines: safeLines, uploaded: true };
    } catch (error: any) {
      setBatchFiles(prev => prev.map(f => f.id === batchFile.id ? { ...f, status: 'error', error: error.message } : f));
      return null;
    }
  };

  const handleStartTranscriptions = () => {
    const toProcess = batchFiles.filter(f => f.status === 'pending' && f.selected);
    toProcess.forEach(f => processFileTranscription(f));
  };

  const handleBurnSingle = async (id: string) => {
    const file = batchFiles.find(f => f.id === id);
    if (!file || !file.lines) return;

    let target = file;
    if (!file.uploaded) {
      const processed = await processFileTranscription(file);
      if (!processed) return;
      target = processed;
    }

    const safeLines = fixOverlaps(target.lines || []);
    setBatchFiles(prev => prev.map(f => f.id === target.id ? { ...f, status: 'burning', progress: 0 } : f));

    try {
      const res = await fetch('/api/batch-burn', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          files: [{ id: target.id, originalName: target.originalName, lines: safeLines }],
          presetName: globalPreset,
          globalOutroVideo: globalOutro === 'preset_default' ? undefined : (globalOutro === 'none' ? '' : globalOutro)
        })
      });
      if (!res.ok) throw new Error('Échec du lancement du rendu');
      addToast('info', `Rendu démarré pour ${target.originalName}`);
    } catch (err: any) {
      setBatchFiles(prev => prev.map(f => f.id === target.id ? { ...f, status: 'error', error: err.message } : f));
    }
  };

  const handleBurnAll = async () => {
    const selected = batchFiles.filter(f => f.selected && (f.status === 'ready' || (f.status === 'pending' && f.subtitleFile)));
    if (selected.length === 0) return addToast('warning', 'Aucun fichier prêt pour le rendu');

    const filesToBurn: BatchFile[] = [];

    for (const f of selected) {
      if (!f.uploaded) {
        const processed = await processFileTranscription(f);
        if (processed) filesToBurn.push(processed);
      } else if (f.status === 'ready') {
        filesToBurn.push(f);
      }
    }

    if (filesToBurn.length === 0) return;

    filesToBurn.forEach(f => {
      setBatchFiles(prev => prev.map(x => x.id === f.id ? { ...x, status: 'burning', progress: 0 } : x));
    });

    try {
      const res = await fetch('/api/batch-burn', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          files: filesToBurn.map(f => ({
            id: f.id,
            originalName: f.originalName,
            lines: fixOverlaps(f.lines || [])
          })),
          presetName: globalPreset,
          globalOutroVideo: globalOutro === 'preset_default' ? undefined : (globalOutro === 'none' ? '' : globalOutro)
        })
      });
      if (!res.ok) throw new Error('Échec batch burn');
      addToast('success', `Rendu par lot lancé pour ${filesToBurn.length} fichier(s)`);
    } catch {
      addToast('error', 'Erreur lors du lancement du rendu par lot');
    }
  };

  const handleCancelJob = async (id: string) => {
    try {
      await fetch(`/api/jobs/${id}/cancel`, { method: 'POST' });
      setBatchFiles(prev => prev.map(f => f.id === id ? { ...f, status: 'cancelled' } : f));
      addToast('info', 'Tâche annulée');
    } catch {}
  };

  const handleRemoveBatchFile = (id: string) => {
    setBatchFiles(prev => prev.filter(f => f.id !== id));
  };

  const handleDownloadSRT = (lines: SubtitleLine[], filename: string) => {
    const safeLines = fixOverlaps(lines);
    const formatTime = (seconds: number) => {
      const date = new Date(seconds * 1000);
      const hh = String(Math.floor(seconds / 3600)).padStart(2, '0');
      const mm = String(date.getUTCMinutes()).padStart(2, '0');
      const ss = String(date.getUTCSeconds()).padStart(2, '0');
      const ms = String(date.getUTCMilliseconds()).padStart(3, '0');
      return `${hh}:${mm}:${ss},${ms}`;
    };

    const srtContent = safeLines.map((line, i) => 
      `${i + 1}\n${formatTime(line.start)} --> ${formatTime(line.end)}\n${line.text}\n`
    ).join('\n');

    const blob = new Blob([srtContent], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename.replace(/\.[^/.]+$/, "") + '.srt';
    a.click();
    URL.revokeObjectURL(url);
  };

  const openEditor = (file: BatchFile) => {
    setEditingFileId(file.id);
    const safe = fixOverlaps(JSON.parse(JSON.stringify(file.lines || [])));
    setEditingLines(safe);
    setCurrentTime(0);
  };

  const saveEditedLines = () => {
    const normalized = fixOverlaps(editingLines);
    setBatchFiles(prev => prev.map(f => f.id === editingFileId ? { ...f, lines: normalized } : f));
    setEditingFileId(null);
    addToast('success', 'Sous-titres synchronisés et validés sans chevauchements');
  };

  const handleApplyShift = () => {
    if (!shiftMs) return;
    const shiftSec = shiftMs / 1000;
    const shifted = editingLines.map(line => ({
      ...line,
      start: Math.max(0, line.start + shiftSec),
      end: Math.max(0.1, line.end + shiftSec)
    }));
    setEditingLines(fixOverlaps(shifted));
    setShiftMs(0);
    addToast('info', `Décalage de ${shiftMs > 0 ? '+' : ''}${shiftMs}ms appliqué`);
  };

  const handleAutoFixOverlaps = () => {
    const fixed = fixOverlaps(editingLines);
    setEditingLines(fixed);
    addToast('success', 'Tous les chevauchements ont été nettoyés avec succès');
  };

  const handleAddLine = (afterIndex?: number) => {
    const currentVideoTime = currentTime;
    const newLines = [...editingLines];
    const insertAt = afterIndex !== undefined ? afterIndex + 1 : newLines.length;

    let start = currentVideoTime;
    let end = currentVideoTime + 2.0;

    if (afterIndex !== undefined && newLines[afterIndex]) {
      start = newLines[afterIndex].end + 0.05;
      end = start + 2.0;
    }

    const newLine: SubtitleLine = {
      id: Math.random(),
      start,
      end,
      text: 'Nouveau sous-titre'
    };

    newLines.splice(insertAt, 0, newLine);
    setEditingLines(fixOverlaps(newLines));
    addToast('info', 'Nouvelle ligne ajoutée');
  };

  const handleDeleteLine = (index: number) => {
    const newLines = [...editingLines];
    newLines.splice(index, 1);
    setEditingLines(fixOverlaps(newLines));
  };

  const handleSearchReplace = () => {
    if (!searchQuery) return;
    let matchCount = 0;
    const updated = editingLines.map(line => {
      if (line.text.includes(searchQuery)) {
        matchCount++;
        return {
          ...line,
          text: line.text.replaceAll(searchQuery, replaceQuery)
        };
      }
      return line;
    });

    setEditingLines(fixOverlaps(updated));
    addToast('success', `${matchCount} occurrence(s) remplacée(s)`);
  };

  const handleRegroup = async () => {
    const allWords: SubtitleWord[] = [];
    editingLines.forEach(l => {
      if (l.words) allWords.push(...l.words);
    });

    if (allWords.length === 0) return addToast('warning', 'Aucun mot disponible pour le regroupement');

    try {
      const res = await fetch('/api/regroup', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          words: allWords,
          maxChars: editorMaxChars,
          maxLines: editorMaxLines
        })
      });
      if (res.ok) {
        const data = await res.json();
        setEditingLines(fixOverlaps(data.lines));
        addToast('success', 'Regroupement intelligent effectué');
      }
    } catch {
      addToast('error', 'Erreur lors du regroupement');
    }
  };

  const activeEditingFile = batchFiles.find(f => f.id === editingFileId);
  const currentActiveLine = editingLines.find(l => currentTime >= l.start && currentTime <= l.end);

  return (
    <div className="min-h-screen text-gray-100 font-sans relative overflow-x-hidden bg-[#070709] selection:bg-indigo-500 selection:text-white">
      <div className="fixed inset-0 pointer-events-none z-0">
        <div className="absolute top-[-10%] left-[-10%] w-[50vw] h-[50vw] rounded-full bg-indigo-600/10 blur-[130px]" />
        <div className="absolute bottom-[-10%] right-[-10%] w-[50vw] h-[50vw] rounded-full bg-purple-600/10 blur-[130px]" />
      </div>

      <div className="fixed top-5 right-5 z-50 flex flex-col gap-2 pointer-events-none">
        <AnimatePresence>
          {toasts.map(toast => (
            <motion.div
              key={toast.id}
              initial={{ opacity: 0, x: 50, scale: 0.9 }}
              animate={{ opacity: 1, x: 0, scale: 1 }}
              exit={{ opacity: 0, scale: 0.9, transition: { duration: 0.2 } }}
              className={`pointer-events-auto px-4 py-3 rounded-2xl border shadow-2xl backdrop-blur-xl flex items-center gap-3 text-sm font-medium ${
                toast.type === 'success' ? 'bg-emerald-950/80 border-emerald-500/30 text-emerald-200' :
                toast.type === 'error' ? 'bg-red-950/80 border-red-500/30 text-red-200' :
                toast.type === 'warning' ? 'bg-amber-950/80 border-amber-500/30 text-amber-200' :
                'bg-indigo-950/80 border-indigo-500/30 text-indigo-200'
              }`}
            >
              {toast.type === 'success' && <CheckCircle className="w-4 h-4 text-emerald-400 shrink-0" />}
              {toast.type === 'error' && <AlertTriangle className="w-4 h-4 text-red-400 shrink-0" />}
              {toast.type === 'warning' && <AlertTriangle className="w-4 h-4 text-amber-400 shrink-0" />}
              {toast.type === 'info' && <Sparkles className="w-4 h-4 text-indigo-400 shrink-0" />}
              <span>{toast.message}</span>
            </motion.div>
          ))}
        </AnimatePresence>

      {browserOpen && (
        <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/80 backdrop-blur-sm p-4">
          <div className="bg-[#0b0c10] border border-white/10 rounded-2xl shadow-2xl w-full max-w-2xl max-h-[80vh] flex flex-col overflow-hidden">
            <div className="p-4 border-b border-white/10 flex items-center justify-between bg-white/[0.02]">
              <h3 className="text-lg font-bold text-white">Sélectionner un dossier</h3>
              <button onClick={() => setBrowserOpen(false)} className="text-gray-400 hover:text-white"><X className="w-5 h-5" /></button>
            </div>
            <div className="p-3 bg-black/40 border-b border-white/5 text-sm font-mono text-gray-300 break-all">
              {browserPath}
            </div>
            <div className="flex-1 overflow-y-auto p-2">
              {browserPath !== '/' && (
                <div 
                  onClick={() => fetchBrowser(browserPath.split('/').slice(0, -1).join('/') || '/')}
                  className="flex items-center gap-3 p-3 hover:bg-white/5 rounded-xl cursor-pointer text-gray-400"
                >
                  <Folder className="w-5 h-5" /> .. (Retour)
                </div>
              )}
              {browserEntries.filter(e => e.is_dir).map(e => (
                <div 
                  key={e.path}
                  onClick={() => fetchBrowser(e.path)}
                  className="flex items-center gap-3 p-3 hover:bg-white/5 rounded-xl cursor-pointer text-gray-300"
                >
                  <Folder className="w-5 h-5 text-indigo-400" /> {e.name}
                </div>
              ))}
            </div>
            <div className="p-4 border-t border-white/10 flex justify-end gap-3 bg-white/[0.02]">
              <button onClick={() => setBrowserOpen(false)} className="px-4 py-2 text-sm text-gray-400 hover:text-white transition-colors">Annuler</button>
              <button 
                onClick={() => {
                  if (browserTarget) {
                    setWorkflows(wfs => wfs.map(w => w.id === browserTarget.wfId ? { ...w, [browserTarget.field]: browserPath } : w));
                  }
                  setBrowserOpen(false);
                }} 
                className="px-6 py-2 bg-indigo-600 hover:bg-indigo-500 text-white text-sm font-semibold rounded-xl shadow-lg shadow-indigo-500/20"
              >
                Sélectionner ce dossier
              </button>
            </div>
          </div>
        </div>
      )}

      </div>

      <div className="relative z-10 max-w-7xl mx-auto p-4 sm:p-8 space-y-8">
        <header className="flex flex-col md:flex-row md:items-center justify-between border-b border-white/10 pb-6 gap-6">
          <div className="flex items-center space-x-4">
            <div className="p-3.5 bg-gradient-to-br from-indigo-500/20 to-purple-500/20 rounded-2xl border border-indigo-500/30 backdrop-blur-2xl shadow-lg shadow-indigo-500/10">
              <Sparkles className="w-7 h-7 text-indigo-400" />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <h1 className="text-2xl sm:text-3xl font-display font-bold tracking-tight bg-clip-text text-transparent bg-gradient-to-r from-white via-gray-200 to-gray-400">
                  AutoSubs AI
                </h1>
                <span className="text-[10px] font-mono uppercase bg-indigo-500/20 text-indigo-300 px-2 py-0.5 rounded-full border border-indigo-500/30">
                  v3.0 Ultra
                </span>
              </div>
              <p className="text-gray-400 text-xs sm:text-sm font-medium mt-0.5">
                Sous-titres animés automatiques & synchronisation temps réel
              </p>
            </div>
          </div>

          <div className="flex bg-black/40 p-1.5 rounded-2xl border border-white/10 backdrop-blur-xl shadow-inner overflow-x-auto">
            <button 
              onClick={() => setActiveTab('batch')}
              className={`flex items-center gap-2 px-5 py-2.5 rounded-xl font-medium text-sm transition-all whitespace-nowrap ${
                activeTab === 'batch' 
                  ? 'bg-gradient-to-r from-indigo-600 to-indigo-500 text-white shadow-lg shadow-indigo-500/25 font-semibold' 
                  : 'text-gray-400 hover:text-white hover:bg-white/5'
              }`}
            >
              <Layers className="w-4 h-4" />
              Batch & Traitement
              {batchFiles.length > 0 && (
                <span className="bg-black/40 text-xs px-2 py-0.5 rounded-full border border-white/10 font-mono">
                  {batchFiles.length}
                </span>
              )}
            </button>
            <button 
              onClick={() => setActiveTab('presets')}
              className={`flex items-center gap-2 px-5 py-2.5 rounded-xl font-medium text-sm transition-all whitespace-nowrap ${
                activeTab === 'presets' 
                  ? 'bg-gradient-to-r from-indigo-600 to-indigo-500 text-white shadow-lg shadow-indigo-500/25 font-semibold' 
                  : 'text-gray-400 hover:text-white hover:bg-white/5'
              }`}
            >
              <Sliders className="w-4 h-4" />
              Studio Presets
            </button>
            
              <button 
                onClick={() => setActiveTab('workflows')}
                className={`relative px-4 py-2.5 rounded-xl text-sm font-medium transition-all ${
                  activeTab === 'workflows' 
                    ? 'text-white bg-indigo-500/10 shadow-[inset_0_1px_1px_rgba(255,255,255,0.1)]' 
                    : 'text-gray-400 hover:text-gray-200 hover:bg-white/5'
                }`}
              >
                <div className="flex items-center gap-2">
                  <FastForward className="w-4 h-4" />
                  Workflows
                </div>
              </button>
<button 
              onClick={() => setActiveTab('settings')}
              className={`flex items-center gap-2 px-5 py-2.5 rounded-xl font-medium text-sm transition-all whitespace-nowrap ${
                activeTab === 'settings' 
                  ? 'bg-gradient-to-r from-indigo-600 to-indigo-500 text-white shadow-lg shadow-indigo-500/25 font-semibold' 
                  : 'text-gray-400 hover:text-white hover:bg-white/5'
              }`}
            >
              <Settings className="w-4 h-4" />
              Paramètres
            </button>
          </div>
        </header>

        <AnimatePresence mode="wait">
          {activeTab === 'batch' && (
            <motion.div 
              key="batch"
              initial={{ opacity: 0, y: 15 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -15 }}
              className="space-y-6"
            >
              <div className="bg-white/[0.02] backdrop-blur-3xl border border-white/5 shadow-2xl rounded-3xl p-6 sm:p-8 shadow-2xl space-y-6">
                
                <div className="flex flex-col lg:flex-row lg:justify-between lg:items-center gap-4 pb-6 border-b border-white/10">
                  <div className="flex items-center gap-3">
                    <h2 className="text-xl font-semibold flex items-center text-white">
                      <Layers className="w-5 h-5 mr-2 text-indigo-400" />
                      File de Traitement
                    </h2>
                    <span className="text-xs font-mono text-gray-400 bg-white/5 px-2.5 py-1 rounded-full border border-white/10">
                      {batchFiles.filter(f => f.selected).length} sélectionné(s)
                    </span>
                  </div>

                  <div className="flex flex-wrap items-center gap-3">
                    <label className="flex items-center space-x-2 text-sm text-gray-300 cursor-pointer bg-black/40 px-3 py-2 rounded-xl border border-white/10 hover:border-white/20 transition-all select-none">
                      <input 
                        type="checkbox" 
                        className="rounded border-white/20 bg-black/50 text-indigo-500 focus:ring-indigo-500"
                        checked={batchFiles.length > 0 && batchFiles.every(f => f.selected)}
                        onChange={e => {
                          const checked = e.target.checked;
                          setBatchFiles(prev => prev.map(f => ({ ...f, selected: checked })));
                        }}
                      />
                      <span>Tout</span>
                    </label>

                    <div className="flex items-center bg-black/40 rounded-xl border border-white/10 px-3 py-1.5">
                      <span className="text-xs text-gray-400 mr-2">Preset:</span>
                      <select 
                        value={globalPreset}
                        onChange={e => setGlobalPreset(e.target.value)}
                        className="bg-transparent text-white text-sm focus:outline-none cursor-pointer"
                      >
                        {presets.map(p => <option key={p.name} value={p.name} className="bg-gray-900">{p.name}</option>)}
                      </select>
                    </div>

                    <div className="flex items-center bg-black/40 rounded-xl border border-white/10 px-3 py-1.5">
                      <span className="text-xs text-gray-400 mr-2">Outro:</span>
                      <select
                        value={globalOutro}
                        onChange={e => setGlobalOutro(e.target.value)}
                        className="bg-transparent text-white text-sm focus:outline-none cursor-pointer"
                      >
                        <option value="preset_default" className="bg-gray-900">Par défaut (Preset)</option>
                        <option value="none" className="bg-gray-900">Aucune Outro</option>
                        {outros.map(o => <option key={`batch-outro-${o}`} value={o} className="bg-gray-900">{o}</option>)}
                      </select>
                    </div>

                    <button 
                      onClick={handleStartTranscriptions}
                      disabled={batchFiles.filter(f => f.status === 'pending' && f.selected).length === 0}
                      className="bg-white/10 hover:bg-white/20 border border-white/10 text-white px-4 py-2 rounded-xl text-sm font-medium transition-all disabled:opacity-40 disabled:cursor-not-allowed flex items-center gap-2"
                    >
                      <Sparkles className="w-4 h-4 text-indigo-400" />
                      Transcrire
                    </button>

                    <button 
                      onClick={handleBurnAll}
                      disabled={batchFiles.filter(f => f.selected && (f.status === 'ready' || (f.status === 'pending' && f.subtitleFile))).length === 0}
                      className="bg-gradient-to-r from-indigo-600 to-purple-600 hover:from-indigo-500 hover:to-purple-500 text-white px-5 py-2 rounded-xl text-sm font-semibold transition-all shadow-lg shadow-indigo-500/25 disabled:opacity-40 disabled:cursor-not-allowed flex items-center gap-2"
                    >
                      <Play className="w-4 h-4 fill-current" />
                      Lancer le Rendu (Burn)
                    </button>
                  </div>
                </div>

                <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
                  <div 
                    onClick={() => fileInputRef.current?.click()}
                    className="lg:col-span-4 border-2 border-dashed border-white/15 rounded-3xl flex flex-col items-center justify-center p-8 text-center cursor-pointer hover:bg-white/[0.04] hover:border-indigo-400/50 transition-all min-h-[220px] group bg-black/20"
                  >
                    <div className="p-4 bg-indigo-500/10 rounded-2xl border border-indigo-500/20 group-hover:scale-110 transition-transform mb-3">
                      <Upload className="w-8 h-8 text-indigo-400" />
                    </div>
                    <p className="font-semibold text-gray-200 text-sm group-hover:text-white">Déposer des vidéos ici</p>
                    <p className="text-xs text-gray-500 mt-1">MP4, MOV, MKV + .SRT, .ASS compagnons</p>
                    <input 
                      type="file" 
                      multiple 
                      accept="video/*,.srt,.ass,.json" 
                      className="hidden" 
                      ref={fileInputRef}
                      onChange={handleFileSelect}
                    />
                  </div>

                  <div className="lg:col-span-8 space-y-3">
                    {batchFiles.length === 0 ? (
                      <div className="h-full min-h-[220px] flex flex-col items-center justify-center text-gray-500 italic border border-white/5 rounded-3xl bg-black/20 p-6 text-center">
                        <Video className="w-10 h-10 text-gray-600 mb-2 stroke-[1.5]" />
                        <p className="text-sm">Aucune vidéo en attente.</p>
                        <p className="text-xs text-gray-600 mt-1">Glissez vos fichiers ou cliquez sur l'encadré pour commencer.</p>
                      </div>
                    ) : (
                      batchFiles.map(file => (
                        <div 
                          key={file.id} 
                          className={`flex flex-col sm:flex-row sm:items-center justify-between p-4 rounded-2xl border backdrop-blur-md gap-4 transition-all ${
                            file.status === 'burning' ? 'bg-purple-950/20 border-purple-500/30 shadow-lg shadow-purple-500/10' :
                            file.status === 'ready' ? 'bg-indigo-950/20 border-indigo-500/30' :
                            file.status === 'done' ? 'bg-emerald-950/20 border-emerald-500/30' :
                            file.status === 'error' ? 'bg-red-950/20 border-red-500/30' :
                            'bg-black/40 border-white/10 hover:border-white/20'
                          }`}
                        >
                          <div className="flex items-center space-x-3 min-w-0">
                            <input 
                              type="checkbox" 
                              checked={file.selected || false}
                              onChange={e => {
                                const checked = e.target.checked;
                                setBatchFiles(prev => prev.map(f => f.id === file.id ? { ...f, selected: checked } : f));
                              }}
                              className="rounded border-white/20 bg-black/50 text-indigo-500 focus:ring-indigo-500"
                            />
                            <div className="p-2 bg-white/5 rounded-xl border border-white/10 shrink-0">
                              <Video className="w-5 h-5 text-indigo-400" />
                            </div>
                            <div className="min-w-0">
                              <p className="font-medium text-gray-200 truncate text-sm">{file.originalName}</p>
                              {file.lines && (
                                <p className="text-[11px] text-gray-500 font-mono">
                                  {file.lines.length} bloc(s) de sous-titres
                                </p>
                              )}
                            </div>
                          </div>
                          
                          <div className="flex flex-wrap items-center gap-2.5 ml-auto sm:ml-0">
                            {file.status === 'pending' && (
                              <span className="text-gray-400 bg-white/5 px-2.5 py-1 rounded-lg border border-white/10 text-xs flex items-center">
                                <Clock className="w-3.5 h-3.5 mr-1 text-gray-400"/> En attente
                              </span>
                            )}
                            
                            {file.status === 'uploading' && (
                              <span className="text-amber-300 bg-amber-500/10 px-2.5 py-1 rounded-lg border border-amber-500/20 text-xs flex items-center animate-pulse">
                                <Upload className="w-3.5 h-3.5 mr-1"/> Envoi...
                              </span>
                            )}

                            {file.status === 'transcribing' && (
                              <div className="flex items-center gap-2">
                                <span className="text-indigo-300 bg-indigo-500/10 px-2.5 py-1 rounded-lg border border-indigo-500/20 text-xs flex items-center animate-pulse">
                                  <Sparkles className="w-3.5 h-3.5 mr-1"/> Transcription IA...
                                </span>
                                <button 
                                  onClick={() => handleCancelJob(file.id)} 
                                  className="p-1 hover:bg-white/10 rounded-lg text-gray-400 hover:text-red-400 transition-colors" 
                                  title="Arrêter"
                                >
                                  <Square className="w-4 h-4 fill-current" />
                                </button>
                              </div>
                            )}

                            {file.status === 'burning' && (
                              <div className="flex items-center gap-3 min-w-[170px]">
                                <div className="flex-1">
                                  <div className="flex justify-between items-center text-[11px] mb-1 font-mono text-purple-300">
                                    <span>Encodage vidéo</span>
                                    <span>{file.progress !== undefined ? `${file.progress}%` : '…'}</span>
                                  </div>
                                  <div className="w-full bg-white/10 rounded-full h-1.5 overflow-hidden">
                                    <div 
                                      className="bg-gradient-to-r from-indigo-500 to-purple-500 h-full transition-all duration-300"
                                      style={{ width: `${file.progress || 0}%` }}
                                    />
                                  </div>
                                </div>
                                <button 
                                  onClick={() => handleCancelJob(file.id)} 
                                  className="p-1 hover:bg-white/10 rounded-lg text-gray-400 hover:text-red-400 transition-colors shrink-0" 
                                  title="Arrêter"
                                >
                                  <Square className="w-4 h-4 fill-current" />
                                </button>
                              </div>
                            )}

                            {file.status === 'done' && (
                              <span className="text-emerald-300 bg-emerald-500/10 px-2.5 py-1 rounded-lg border border-emerald-500/20 text-xs flex items-center font-medium">
                                <CheckCircle className="w-3.5 h-3.5 mr-1 text-emerald-400"/> Terminé
                              </span>
                            )}

                            {file.status === 'error' && (
                              <div className="flex items-center gap-2">
                                <span className="text-red-300 bg-red-500/10 px-2.5 py-1 rounded-lg border border-red-500/20 text-xs flex items-center" title={file.error}>
                                  <AlertTriangle className="w-3.5 h-3.5 mr-1 text-red-400"/> Erreur
                                </span>
                                <button
                                  onClick={() => processFileTranscription(file)}
                                  className="px-2 py-1 bg-white/10 hover:bg-white/20 text-white rounded-lg text-xs"
                                >
                                  Relancer
                                </button>
                              </div>
                            )}

                            {file.status === 'ready' && (
                              <div className="flex items-center gap-2">
                                <button 
                                  onClick={() => openEditor(file)}
                                  className="flex items-center px-3 py-1.5 bg-indigo-500/20 hover:bg-indigo-500/30 text-indigo-200 rounded-xl text-xs font-semibold border border-indigo-500/30 transition-all shadow-sm shadow-indigo-500/20"
                                >
                                  <Edit3 className="w-3.5 h-3.5 mr-1.5" />
                                  Éditer & Corriger
                                </button>
                                <button 
                                  onClick={() => handleBurnSingle(file.id)}
                                  className="flex items-center px-3 py-1.5 bg-purple-500/20 hover:bg-purple-500/30 text-purple-200 rounded-xl text-xs font-semibold border border-purple-500/30 transition-all"
                                  title="Lancer le rendu de cette vidéo"
                                >
                                  <Play className="w-3.5 h-3.5 mr-1.5 fill-current" />
                                  Rendu
                                </button>
                                <button 
                                  onClick={() => handleDownloadSRT(file.lines || [], file.originalName)}
                                  className="p-1.5 bg-white/5 hover:bg-white/10 text-gray-300 rounded-xl border border-white/10 transition-colors"
                                  title="Télécharger .SRT"
                                >
                                  <Download className="w-3.5 h-3.5" />
                                </button>
                              </div>
                            )}

                            <button 
                              onClick={() => handleRemoveBatchFile(file.id)}
                              className="p-1.5 text-gray-500 hover:text-red-400 hover:bg-red-500/10 rounded-lg transition-colors"
                              title="Retirer de la file"
                            >
                              <Trash2 className="w-4 h-4" />
                            </button>
                          </div>
                        </div>
                      ))
                    )}
                  </div>
                </div>
              </div>
            </motion.div>
          )}

          {activeTab === 'presets' && (
            <motion.div 
              key="presets"
              initial={{ opacity: 0, y: 15 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -15 }}
              className="grid grid-cols-1 lg:grid-cols-12 gap-8"
            >
              <div className="lg:col-span-4 space-y-4">
                <div className="bg-white/[0.02] backdrop-blur-3xl border border-white/5 shadow-2xl rounded-3xl p-6 shadow-2xl space-y-4">
                  <div className="flex justify-between items-center pb-4 border-b border-white/10">
                    <h2 className="text-lg font-semibold flex items-center text-white">
                      <Sliders className="w-5 h-5 mr-2 text-indigo-400" />
                      Studio Presets
                    </h2>
                    <button 
                      onClick={() => {
                        setCurrentPreset({ ...defaultPreset, name: 'Nouveau Preset' });
                        setIsEditingPreset(true);
                      }}
                      className="bg-gradient-to-r from-indigo-600 to-purple-600 hover:from-indigo-500 hover:to-purple-500 shadow-lg shadow-indigo-500/25 text-white px-3 py-1.5 rounded-xl text-xs font-semibold transition-all flex items-center shadow-md shadow-indigo-500/20"
                    >
                      <Plus className="w-3.5 h-3.5 mr-1" />
                      Créer
                    </button>
                  </div>

                  <div className="space-y-2.5">
                    {Object.entries(presets.reduce((acc, p) => {
                      const b = p.brand || 'Général';
                      if (!acc[b]) acc[b] = [];
                      acc[b].push(p);
                      return acc;
                    }, {} as Record<string, Preset[]>)).map(([brandName, brandPresets]) => (
                      <div key={brandName} className="space-y-2">
                        <h3 className="text-[11px] font-bold text-gray-500 uppercase tracking-widest pl-2 mb-1">{brandName}</h3>
                        {(brandPresets as Preset[]).map(p => (
                          <div 
                            key={p.name}
                            onClick={() => {
                              setCurrentPreset({ ...defaultPreset, ...p });
                              setIsEditingPreset(true);
                            }}
                            className={`flex items-center justify-between p-3.5 rounded-2xl border transition-all cursor-pointer group ${
                              currentPreset.name === p.name 
                                ? 'bg-indigo-950/40 border-indigo-500/50 shadow-md shadow-indigo-500/10' 
                                : 'bg-black/30 border-white/5 hover:border-white/15'
                            }`}
                          >
                            <div>
                              <p className="font-semibold text-sm text-gray-200 group-hover:text-white">{p.name}</p>
                              <p className="text-[11px] text-gray-400 font-mono">
                                {p.aspectRatio || '9:16'} • {p.fontFamily} • {p.size}px
                              </p>
                            </div>
                            <div className="flex items-center gap-1 opacity-80 group-hover:opacity-100">
                              <button 
                                onClick={(e) => { e.stopPropagation(); handleDuplicatePreset(p); }}
                                className="p-1.5 text-gray-400 hover:text-indigo-300 hover:bg-white/5 rounded-lg transition-colors"
                                title="Dupliquer"
                              >
                                <Copy className="w-3.5 h-3.5" />
                              </button>
                              {p.name !== 'Défaut' && (
                                <button 
                                  onClick={(e) => { e.stopPropagation(); handleDeletePreset(p.name); }}
                                  className="p-1.5 text-gray-400 hover:text-red-400 hover:bg-red-500/10 rounded-lg transition-colors"
                                  title="Supprimer"
                                >
                                  <Trash2 className="w-3.5 h-3.5" />
                                </button>
                              )}
                            </div>
                          </div>
                        ))}
                      </div>
                    ))}
                  </div>
                </div>
              </div>

              <div className="lg:col-span-8 space-y-6">
                <div className="bg-white/[0.02] backdrop-blur-3xl border border-white/5 shadow-2xl rounded-3xl p-6 sm:p-8 shadow-2xl">
                  <div className="flex flex-col sm:flex-row sm:items-center justify-between pb-6 border-b border-white/10 gap-4 mb-6">
                    <div>
                      <h3 className="text-xl font-bold text-white flex items-center gap-2">
                        <Edit3 className="w-5 h-5 text-indigo-400" />
                        {currentPreset.name}
                      </h3>
                      <p className="text-xs text-gray-400 mt-0.5">Personnalisation typographique et animations ASS</p>
                    </div>

                    <div className="flex items-center gap-2">
                      <button 
                        onClick={handleSavePreset}
                        className="bg-gradient-to-r from-indigo-600 to-purple-600 hover:from-indigo-500 hover:to-purple-500 shadow-lg shadow-indigo-500/25 text-white px-5 py-2 rounded-xl text-sm font-semibold transition-all shadow-lg shadow-indigo-500/25 flex items-center gap-1.5"
                      >
                        <Save className="w-4 h-4" />
                        Enregistrer Preset
                      </button>
                    </div>
                  </div>

                  <div className="grid grid-cols-1 xl:grid-cols-12 gap-8">
                    <div className="xl:col-span-6 space-y-5">
                      <div>
                        <label className="block text-xs font-semibold text-gray-400 uppercase tracking-wider mb-1.5">Nom du preset</label>
                        <input 
                          type="text" 
                          value={currentPreset.name}
                          onChange={e => setCurrentPreset({ ...currentPreset, name: e.target.value })}
                          className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2 text-white text-sm focus:ring-2 focus:ring-indigo-500 focus:outline-none"
                        />
                      </div>

                      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 mb-4">
                        <div className="space-y-1.5">
                          <label className="text-xs font-medium text-gray-400">Marque (Brand)</label>
                          <input 
                            type="text"
                            value={currentPreset.brand || ""}
                            onChange={e => setCurrentPreset({ ...currentPreset, brand: e.target.value })}
                            placeholder="Ex: Client A..."
                            className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2 text-white text-sm focus:ring-2 focus:ring-indigo-500 focus:outline-none"
                          />
                        </div>
                        <div className="space-y-1.5">
                          <label className="text-xs font-medium text-gray-400">Format Vidéo</label>
                          <select 
                            value={currentPreset.aspectRatio || "16:9"}
                            onChange={e => setCurrentPreset({ ...currentPreset, aspectRatio: e.target.value as any })}
                            className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2 text-white text-sm focus:ring-2 focus:ring-indigo-500 focus:outline-none"
                          >
                            <option value="9:16" className="bg-gray-900">Portrait (9:16) - TikTok</option>
                            <option value="16:9" className="bg-gray-900">Paysage (16:9) - YouTube</option>
                            <option value="1:1" className="bg-gray-900">Carré (1:1) - Instagram</option>
                            <option value="4:5" className="bg-gray-900">Vertical (4:5) - Feed</option>
                          </select>
                        </div>
                      </div>

                      
                      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 mb-4">
                        <div className="space-y-1.5">
                          <label className="text-xs font-medium text-gray-400">Post Vidéo (Outro)</label>
                          <div className="flex gap-2">
                            <select 
                              value={currentPreset.outroVideo || ""}
                              onChange={e => setCurrentPreset({ ...currentPreset, outroVideo: e.target.value })}
                              className="flex-1 bg-black/40 border border-white/10 rounded-xl px-3.5 py-2 text-white text-sm focus:ring-2 focus:ring-indigo-500 focus:outline-none"
                            >
                              <option value="">Aucune outro</option>
                              {outros.map(o => <option key={o} value={o}>{o}</option>)}
                            </select>
                            <label className="bg-white/10 hover:bg-white/20 text-white p-2 rounded-xl cursor-pointer transition-colors flex items-center justify-center" title="Uploader une outro">
                              <input type="file" accept="video/mp4,video/quicktime" className="hidden" onChange={handleUploadOutro} />
                              <Upload className="w-4 h-4" />
                            </label>
                          </div>
                        </div>
                      </div>

                      <div className="grid grid-cols-2 gap-4">
                        <div>
                          <label className="block text-xs font-semibold text-gray-400 uppercase tracking-wider mb-1.5">Police</label>
                          <select 
                            value={currentPreset.fontFamily}
                            onChange={e => setCurrentPreset({ ...currentPreset, fontFamily: e.target.value })}
                            className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2 text-white text-sm focus:ring-2 focus:ring-indigo-500 focus:outline-none"
                          >
                            {fonts.map(f => <option key={f} value={f} className="bg-gray-900">{f}</option>)}
                          </select>
                        </div>

                        <div>
                          <label className="block text-xs font-semibold text-gray-400 uppercase tracking-wider mb-1.5">Animation</label>
                          <select 
                            value={currentPreset.animationStyle}
                            onChange={e => setCurrentPreset({ ...currentPreset, animationStyle: e.target.value as any })}
                            className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2 text-white text-sm focus:ring-2 focus:ring-indigo-500 focus:outline-none"
                          >
                            <option value="pop" className="bg-gray-900">Pop (Mot par mot)</option>
                            <option value="karaoke" className="bg-gray-900">Karaoké (Progressif)</option>
                            <option value="bounce" className="bg-gray-900">Rebond (Bounce)</option>
                            <option value="fade" className="bg-gray-900">Fondu (Fade)</option>
                            <option value="slide-up" className="bg-gray-900">Glissement (Slide-Up)</option>
                            <option value="none" className="bg-gray-900">Statique (Aucune)</option>
                          </select>
                        </div>
                      </div>

                      <div className="grid grid-cols-2 gap-4 mb-4">

                        <div className="space-y-1.5">

                          <label className="text-xs font-medium text-gray-400">Marque (Brand)</label>
                          <input 
                            type="text"
                            value={currentPreset.brand || ""}
                            onChange={e => setCurrentPreset({ ...currentPreset, brand: e.target.value })}
                            placeholder="Ex: Client A, Ma Chaîne..."
                            className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2 text-white text-sm focus:ring-2 focus:ring-indigo-500 focus:outline-none mb-4"
                          />

                          
                          <label className="text-xs font-medium text-gray-400 mt-4">Marque (Brand)</label>
                          <input 
                            type="text"
                            value={currentPreset.brand || ""}
                            onChange={e => setCurrentPreset({ ...currentPreset, brand: e.target.value })}
                            placeholder="Ex: Client A, Ma Chaîne..."
                            className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2 text-white text-sm focus:ring-2 focus:ring-indigo-500 focus:outline-none mb-4"
                          />

                          <label className="text-xs font-medium text-gray-400">Aspect Ratio (Format)</label>

                          <select 

                            value={currentPreset.aspectRatio || "16:9"}

                            onChange={e => setCurrentPreset({ ...currentPreset, aspectRatio: e.target.value as any })}

                            className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2 text-white text-sm focus:ring-2 focus:ring-indigo-500 focus:outline-none"

                          >

                            <option value="9:16" className="bg-gray-900">Portrait (9:16) - TikTok/Reels</option>

                            <option value="16:9" className="bg-gray-900">Paysage (16:9) - YouTube</option>

                            <option value="1:1" className="bg-gray-900">Carré (1:1) - Instagram</option>

                            <option value="4:5" className="bg-gray-900">Vertical (4:5) - Feed</option>

                          </select>

                        </div>

                      </div>

                      
                      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 mb-4">
                        <div className="space-y-1.5">
                          <label className="text-xs font-medium text-gray-400">Post Vidéo (Outro)</label>
                          <div className="flex gap-2">
                            <select 
                              value={currentPreset.outroVideo || ""}
                              onChange={e => setCurrentPreset({ ...currentPreset, outroVideo: e.target.value })}
                              className="flex-1 bg-black/40 border border-white/10 rounded-xl px-3.5 py-2 text-white text-sm focus:ring-2 focus:ring-indigo-500 focus:outline-none"
                            >
                              <option value="">Aucune outro</option>
                              {outros.map(o => <option key={o} value={o}>{o}</option>)}
                            </select>
                            <label className="bg-white/10 hover:bg-white/20 text-white p-2 rounded-xl cursor-pointer transition-colors flex items-center justify-center" title="Uploader une outro">
                              <input type="file" accept="video/mp4,video/quicktime" className="hidden" onChange={handleUploadOutro} />
                              <Upload className="w-4 h-4" />
                            </label>
                          </div>
                        </div>
                      </div>

                      <div className="grid grid-cols-2 gap-4">
                        <div className="bg-black/30 p-3 rounded-xl border border-white/5 space-y-1.5">
                          <span className="text-xs text-gray-400 font-medium">Texte Principal</span>
                          <div className="flex items-center gap-2">
                            <input 
                              type="color" 
                              value={currentPreset.baseColor || '#ffffff'}
                              onChange={e => setCurrentPreset({ ...currentPreset, baseColor: e.target.value })}
                              className="w-8 h-8 rounded-lg cursor-pointer bg-transparent border-0"
                            />
                            <input 
                              type="text" 
                              value={currentPreset.baseColor}
                              onChange={e => setCurrentPreset({ ...currentPreset, baseColor: e.target.value })}
                              className="w-20 bg-black/50 border border-white/10 rounded px-2 py-1 text-xs font-mono uppercase text-gray-200"
                            />
                          </div>
                        </div>

                        <div className="bg-black/30 p-3 rounded-xl border border-white/5 space-y-1.5">
                          <span className="text-xs text-gray-400 font-medium">Highlight / Mot actif</span>
                          <div className="flex items-center gap-2">
                            <input 
                              type="color" 
                              value={currentPreset.highlightColor || '#00d2ff'}
                              onChange={e => setCurrentPreset({ ...currentPreset, highlightColor: e.target.value })}
                              className="w-8 h-8 rounded-lg cursor-pointer bg-transparent border-0"
                            />
                            <input 
                              type="text" 
                              value={currentPreset.highlightColor}
                              onChange={e => setCurrentPreset({ ...currentPreset, highlightColor: e.target.value })}
                              className="w-20 bg-black/50 border border-white/10 rounded px-2 py-1 text-xs font-mono uppercase text-gray-200"
                            />
                          </div>
                        </div>
                      </div>

                      <div className="grid grid-cols-3 gap-2 bg-black/30 p-3 rounded-xl border border-white/5">
                        <label className="flex items-center gap-2 text-xs font-medium text-gray-300 cursor-pointer">
                          <input 
                            type="checkbox" 
                            checked={currentPreset.bold || false}
                            onChange={e => setCurrentPreset({ ...currentPreset, bold: e.target.checked })}
                            className="rounded text-indigo-500 bg-black/50 border-white/20"
                          />
                          <span>Gras</span>
                        </label>

                        <label className="flex items-center gap-2 text-xs font-medium text-gray-300 cursor-pointer">
                          <input 
                            type="checkbox" 
                            checked={currentPreset.italic || false}
                            onChange={e => setCurrentPreset({ ...currentPreset, italic: e.target.checked })}
                            className="rounded text-indigo-500 bg-black/50 border-white/20"
                          />
                          <span>Italique</span>
                        </label>

                        <label className="flex items-center gap-2 text-xs font-medium text-gray-300 cursor-pointer">
                          <input 
                            type="checkbox" 
                            checked={currentPreset.uppercase}
                            onChange={e => setCurrentPreset({ ...currentPreset, uppercase: e.target.checked })}
                            className="rounded text-indigo-500 bg-black/50 border-white/20"
                          />
                          <span>Majuscules</span>
                        </label>
                      </div>

                      <div className="space-y-3 bg-black/30 p-4 rounded-xl border border-white/5">
                        <div>
                          <div className="flex justify-between text-xs text-gray-400 font-medium mb-1">
                            <span>Taille de Police</span>
                            <span className="text-indigo-400 font-mono">{currentPreset.size}px</span>
                          </div>
                          <input 
                            type="range" 
                            min="14" max="70" 
                            value={currentPreset.size}
                            onChange={e => setCurrentPreset({ ...currentPreset, size: Number(e.target.value) })}
                            className="w-full accent-indigo-500"
                          />
                        </div>

                        <div>
                          <div className="flex justify-between text-xs text-gray-400 font-medium mb-1">
                            <span>Épaisseur Contour</span>
                            <span className="text-indigo-400 font-mono">{currentPreset.outlineThickness}px</span>
                          </div>
                          <input 
                            type="range" 
                            min="0" max="8" step="0.5"
                            value={currentPreset.outlineThickness}
                            onChange={e => setCurrentPreset({ ...currentPreset, outlineThickness: Number(e.target.value) })}
                            className="w-full accent-indigo-500"
                          />
                        </div>

                        <div>
                          <div className="flex justify-between text-xs text-gray-400 font-medium mb-1">
                            <span>Position Verticale (Y)</span>
                            <span className="text-indigo-400 font-mono">{currentPreset.positionY}%</span>
                          </div>
                          <input 
                            type="range" 
                            min="10" max="95" 
                            value={currentPreset.positionY}
                            onChange={e => setCurrentPreset({ ...currentPreset, positionY: Number(e.target.value) })}
                            className="w-full accent-indigo-500"
                          />
                        </div>
                      </div>

                      <div>
                        <label className="block text-xs font-semibold text-gray-400 uppercase tracking-wider mb-1.5">
                          Mots-clés Watchdog (détection auto)
                        </label>
                        <input 
                          type="text" 
                          value={currentPreset.matchKeywords || ''}
                          onChange={e => setCurrentPreset({ ...currentPreset, matchKeywords: e.target.value })}
                          placeholder="ex: reel, tiktok, shorts, CF"
                          className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2 text-white text-xs font-mono focus:ring-2 focus:ring-indigo-500 focus:outline-none"
                        />
                      </div>
                    </div>

                    <div className="xl:col-span-6 flex flex-col items-center justify-center">
                      <div className="w-full flex items-center justify-between mb-3">
                        <span className="text-xs font-semibold text-gray-400 uppercase tracking-wider flex items-center gap-1.5">
                          <Smartphone className="w-4 h-4 text-indigo-400" />
                          Aperçu Format Reels (9:16)
                        </span>
                        <input 
                          type="text" 
                          value={previewSampleText} 
                          onChange={e => setPreviewSampleText(e.target.value)}
                          className="bg-black/50 border border-white/10 text-xs px-2.5 py-1 rounded-lg text-gray-300 w-40 text-center"
                          placeholder="Texte de test"
                        />
                      </div>

                      <div 
                        ref={previewBoxRef}
                        className="relative w-[260px] h-[460px] bg-[#0c0c10] border-2 border-white/20 rounded-3xl overflow-hidden shadow-2xl flex flex-col items-center justify-center select-none"
                      >
                        <div className="absolute inset-0 bg-gradient-to-b from-indigo-950/20 via-black to-purple-950/30" />
                        <div className="absolute inset-x-4 top-4 flex justify-between items-center text-[10px] text-gray-500 font-mono">
                          <span>1080 × 1920</span>
                          <span>9:16</span>
                        </div>

                        <div 
                          className="absolute w-[90%] text-center cursor-move transition-transform"
                          style={{
                            left: '50%',
                            top: `${currentPreset.positionY}%`,
                            transform: 'translate(-50%, -50%)',
                            fontFamily: `"${currentPreset.fontFamily}", sans-serif`,
                            fontSize: `${(currentPreset.size / 24) * 16}px`,
                            color: currentPreset.baseColor,
                            fontWeight: currentPreset.bold ? 'bold' : 'normal',
                            fontStyle: currentPreset.italic ? 'italic' : 'normal',
                            textTransform: currentPreset.uppercase ? 'uppercase' : 'none',
                            WebkitTextStroke: currentPreset.borderStyle === 1 ? `${currentPreset.outlineThickness}px ${currentPreset.outlineColor}` : 'none',
                            textShadow: currentPreset.borderStyle === 1 && currentPreset.shadowThickness ? 
                              `${currentPreset.shadowThickness}px ${currentPreset.shadowThickness}px 0px ${currentPreset.shadowColor}` : 'none',
                            backgroundColor: currentPreset.borderStyle === 3 ? (currentPreset.shadowColor || '#000000cc') : 'transparent',
                            padding: currentPreset.borderStyle === 3 ? '4px 10px' : '0',
                            borderRadius: currentPreset.borderStyle === 3 ? '8px' : '0'
                          }}
                        >
                          {previewSampleText.split(' ').map((word, idx) => (
                            <span 
                              key={idx} 
                              style={{ 
                                color: idx === 1 ? currentPreset.highlightColor : currentPreset.baseColor,
                                display: 'inline-block',
                                transform: idx === 1 && currentPreset.animationStyle === 'pop' ? 'scale(1.12)' : 'none',
                                margin: '0 3px'
                              }}
                            >
                              {word}
                            </span>
                          ))}
                        </div>

                        <div className="absolute inset-x-0 top-[20%] border-t border-dashed border-white/10 pointer-events-none" />
                        <div className="absolute inset-x-0 bottom-[20%] border-b border-dashed border-white/10 pointer-events-none" />
                      </div>
                    </div>
                  </div>
                </div>
              </div>
            </motion.div>
          )}

          
          {activeTab === 'workflows' && (
            <motion.div 
              initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }}
              className="max-w-4xl mx-auto space-y-6"
            >
              <div className="flex items-center justify-between">
                <div>
                  <h2 className="text-2xl font-bold tracking-tight">Workflows Automatiques</h2>
                  <p className="text-sm text-gray-400 mt-1">Dossiers surveillés en tâche de fond (Watchdog)</p>
                </div>
                <button 
                  onClick={() => {
                    const newW = { id: Date.now().toString(), name: 'Nouveau Workflow', watch_dir: '', output_dir: '', archives_dir: '', preset_name: 'Défaut', enabled: false };
                    setWorkflows([...workflows, newW]);
                  }}
                  className="flex items-center gap-2 px-4 py-2 bg-white text-black rounded-xl text-sm font-semibold hover:bg-gray-200 transition-colors"
                >
                  <Plus className="w-4 h-4" />
                  Nouveau Workflow
                </button>
              </div>

              <div className="space-y-4">
                
                {workflows.length === 0 && (
                  <div className="flex flex-col items-center justify-center p-12 bg-white/[0.02] border border-white/5 rounded-2xl border-dashed">
                    <Folder className="w-12 h-12 text-gray-600 mb-4 stroke-[1.5]" />
                    <h3 className="text-lg font-semibold text-white">Aucun workflow configuré</h3>
                    <p className="text-sm text-gray-400 mt-2 text-center max-w-md">Les workflows permettent de surveiller automatiquement un dossier pour générer les sous-titres dès qu'une vidéo y est déposée.</p>
                  </div>
                )}
                {workflows.map((wf, idx) => (
                  <div key={wf.id} className="bg-[#111111] border border-white/10 rounded-2xl p-5 space-y-4">
                    <div className="flex items-center justify-between">
                      <input 
                        type="text" 
                        value={wf.name}
                        onChange={(e) => {
                          const wfs = [...workflows]; wfs[idx].name = e.target.value; setWorkflows(wfs);
                        }}
                        className="bg-transparent text-lg font-bold text-white border-b border-white/20 focus:border-indigo-500 focus:outline-none"
                      />
                      <div className="flex items-center gap-4">
                        <label className="flex items-center gap-2 text-sm text-gray-300">
                          <input type="checkbox" checked={wf.enabled} onChange={(e) => {
                            const wfs = [...workflows]; wfs[idx].enabled = e.target.checked; setWorkflows(wfs);
                          }} className="w-4 h-4 rounded bg-black/50 border-white/20 text-indigo-500 focus:ring-indigo-500" />
                          Actif
                        </label>
                        <button onClick={async () => {
                          await fetch(`/api/workflows/${wf.id}`, { method: 'DELETE' });
                          fetchWorkflows();
                        }} className="p-2 text-gray-400 hover:text-red-400 hover:bg-red-500/10 rounded-lg transition-colors">
                          <Trash2 className="w-4 h-4" />
                        </button>
                      </div>
                    </div>

                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                      <div className="space-y-1.5">
                        <label className="text-xs font-medium text-gray-400">Dossier à surveiller (Watch)</label>
                        <div className="flex gap-2">
                          <input type="text" value={wf.watch_dir} onChange={(e) => {
                            const wfs = [...workflows]; wfs[idx].watch_dir = e.target.value; setWorkflows(wfs);
                          }} className="flex-1 bg-black/40 border border-white/10 rounded-xl px-3.5 py-2 text-white text-sm" placeholder="/chemin/vers/watch" />
                          <button onClick={() => openBrowser(wf.id, 'watch_dir', wf.watch_dir)} className="px-3 py-2 bg-white/10 hover:bg-white/20 text-white rounded-xl text-sm font-medium transition-colors"><Folder className="w-4 h-4" /></button>
                        </div>
                      </div>
                      <div className="space-y-1.5">
                        <label className="text-xs font-medium text-gray-400">Dossier de sortie (Output)</label>
                        <div className="flex gap-2">
                          <input type="text" value={wf.output_dir} onChange={(e) => {
                            const wfs = [...workflows]; wfs[idx].output_dir = e.target.value; setWorkflows(wfs);
                          }} className="flex-1 bg-black/40 border border-white/10 rounded-xl px-3.5 py-2 text-white text-sm" placeholder="/chemin/vers/output" />
                          <button onClick={() => openBrowser(wf.id, 'output_dir', wf.output_dir)} className="px-3 py-2 bg-white/10 hover:bg-white/20 text-white rounded-xl text-sm font-medium transition-colors"><Folder className="w-4 h-4" /></button>
                        </div>
                      </div>
                      <div className="space-y-1.5">
                        <label className="text-xs font-medium text-gray-400">Dossier d'archives</label>
                        <input type="text" value={wf.archives_dir} onChange={(e) => {
                          const wfs = [...workflows]; wfs[idx].archives_dir = e.target.value; setWorkflows(wfs);
                        }} className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2 text-white text-sm" placeholder="/chemin/vers/archives" />
                      </div>
                      <div className="space-y-1.5">
                        <label className="text-xs font-medium text-gray-400">Preset appliqué</label>
                        <select value={wf.preset_name} onChange={(e) => {
                          const wfs = [...workflows]; wfs[idx].preset_name = e.target.value; setWorkflows(wfs);
                        }} className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2 text-white text-sm">
                          {presets.map(p => <option key={p.name} value={p.name} className="bg-gray-900">{p.name}</option>)}
                        </select>
                      </div>
                    </div>
                    
                    <div className="flex justify-end pt-2">
                      <button onClick={async () => {
                        await fetch('/api/workflows', { method: 'POST', headers: {'Content-Type': 'application/json'}, body: JSON.stringify(wf) });
                        addToast('success', 'Workflow enregistré');
                      }} className="px-4 py-2 bg-white/10 hover:bg-white/20 text-white rounded-xl text-sm font-medium transition-colors">
                        Sauvegarder
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            </motion.div>
          )}

          {activeTab === 'settings' && (
            <motion.div 
              key="settings"
              initial={{ opacity: 0, y: 15 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -15 }}
              className="max-w-3xl mx-auto space-y-6"
            >
              <div className="bg-white/[0.02] backdrop-blur-3xl border border-white/5 shadow-2xl rounded-3xl p-6 sm:p-8 shadow-2xl space-y-6">
                <div className="flex items-center justify-between pb-4 border-b border-white/10">
                  <h2 className="text-xl font-bold flex items-center text-white">
                    <Settings className="w-5 h-5 mr-2 text-indigo-400" />
                    Configuration Serveur & IA
                  </h2>
                </div>

                <div className="flex space-x-2 bg-black/40 p-1.5 rounded-2xl border border-white/10 overflow-x-auto">
                  <button 
                    onClick={() => setActiveSettingsTab('external')}
                    className={`px-4 py-2 rounded-xl text-xs font-semibold transition-all whitespace-nowrap ${
                      activeSettingsTab === 'external' ? 'bg-indigo-600 text-white' : 'text-gray-400 hover:text-white'
                    }`}
                  >
                    Transcription Externe
                  </button>
                  <button 
                    onClick={() => setActiveSettingsTab('local')}
                    className={`px-4 py-2 rounded-xl text-xs font-semibold transition-all whitespace-nowrap ${
                      activeSettingsTab === 'local' ? 'bg-indigo-600 text-white' : 'text-gray-400 hover:text-white'
                    }`}
                  >
                    Transcription Locale & Fallback
                  </button>
                  <button 
                    onClick={() => setActiveSettingsTab('llm')}
                    className={`px-4 py-2 rounded-xl text-xs font-semibold transition-all whitespace-nowrap ${
                      activeSettingsTab === 'llm' ? 'bg-indigo-600 text-white' : 'text-gray-400 hover:text-white'
                    }`}
                  >
                    Auto-Correction LLM
                  </button>
                  <button 
                    onClick={() => setActiveSettingsTab('performance')}
                    className={`px-4 py-2 rounded-xl text-xs font-semibold transition-all whitespace-nowrap ${
                      activeSettingsTab === 'performance' ? 'bg-indigo-600 text-white' : 'text-gray-400 hover:text-white'
                    }`}
                  >
                    Performance & Encodage
                  </button>
                </div>

                {activeSettingsTab === 'external' && (
                  <div className="space-y-4 pt-2">
                    <div>
                      <label className="block text-xs font-semibold text-gray-400 uppercase tracking-wider mb-1.5">URL Endpoint (Speaches / OpenAI)</label>
                      <input 
                        type="text" 
                        value={settings.transcriptionUrl}
                        onChange={e => setSettings({ ...settings, transcriptionUrl: e.target.value })}
                        placeholder="http://speaches:8000/v1/audio/transcriptions"
                        className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2.5 text-white text-sm focus:ring-2 focus:ring-indigo-500 focus:outline-none"
                      />
                    </div>
                    <div>
                      <label className="block text-xs font-semibold text-gray-400 uppercase tracking-wider mb-1.5">Clé API (Optionnelle)</label>
                      <input 
                        type="password" 
                        value={settings.transcriptionApiKey}
                        onChange={e => setSettings({ ...settings, transcriptionApiKey: e.target.value })}
                        placeholder="sk-..."
                        className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2.5 text-white text-sm focus:ring-2 focus:ring-indigo-500 focus:outline-none"
                      />
                    </div>
                    <div className="flex items-end gap-3">
                      <div className="flex-1">
                        <label className="block text-xs font-semibold text-gray-400 uppercase tracking-wider mb-1.5">Modèle Whisper</label>
                        <input 
                          type="text" 
                          list="ext-models"
                          value={settings.transcriptionModel}
                          onChange={e => setSettings({ ...settings, transcriptionModel: e.target.value })}
                          placeholder="speaches-ai/faster-whisper-large-v3"
                          className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2.5 text-white text-sm focus:ring-2 focus:ring-indigo-500 focus:outline-none"
                        />
                        <datalist id="ext-models">
                          {availableModels.map(m => <option key={m} value={m} />)}
                        </datalist>
                      </div>
                      <button 
                        onClick={() => fetchAvailableModels(settings.transcriptionUrl, settings.transcriptionApiKey, true, false)}
                        className="bg-white/10 hover:bg-white/20 px-4 py-2.5 rounded-xl text-xs font-medium text-white border border-white/10 transition-colors"
                      >
                        Tester / Détecter
                      </button>
                    </div>
                  </div>
                )}

                {activeSettingsTab === 'local' && (
                  <div className="space-y-4 pt-2">
                    <div className="bg-black/30 p-4 rounded-2xl border border-white/5 space-y-3">
                      <label className="flex items-center gap-3 text-sm font-medium text-white cursor-pointer select-none">
                        <input 
                          type="checkbox" 
                          checked={settings.localTranscriptionEnabled}
                          onChange={e => setSettings({ ...settings, localTranscriptionEnabled: e.target.checked })}
                          className="rounded text-indigo-500 bg-black/50 border-white/20 w-4 h-4"
                        />
                        <span>Activer la transcription locale prioritaire</span>
                      </label>
                      <label className="flex items-center gap-3 text-sm font-medium text-gray-300 cursor-pointer select-none">
                        <input 
                          type="checkbox" 
                          checked={settings.localFallbackEnabled}
                          onChange={e => setSettings({ ...settings, localFallbackEnabled: e.target.checked })}
                          className="rounded text-indigo-500 bg-black/50 border-white/20 w-4 h-4"
                        />
                        <span>Fallback automatique sur l'API externe en cas d'échec</span>
                      </label>
                    </div>

                    <div className={!settings.localTranscriptionEnabled ? 'opacity-40 pointer-events-none' : 'space-y-4'}>
                      <div>
                        <label className="block text-xs font-semibold text-gray-400 uppercase tracking-wider mb-1.5">URL API Locale</label>
                        <input 
                          type="text" 
                          value={settings.localTranscriptionUrl}
                          onChange={e => setSettings({ ...settings, localTranscriptionUrl: e.target.value })}
                          placeholder="http://192.168.1.50:8005/v1/audio/transcriptions"
                          className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2.5 text-white text-sm focus:ring-2 focus:ring-indigo-500 focus:outline-none"
                        />
                      </div>
                      <div>
                        <label className="block text-xs font-semibold text-gray-400 uppercase tracking-wider mb-1.5">Modèle Local</label>
                        <input 
                          type="text" 
                          value={settings.localTranscriptionModel}
                          onChange={e => setSettings({ ...settings, localTranscriptionModel: e.target.value })}
                          placeholder="bofenghuang-distil-fr"
                          className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2.5 text-white text-sm focus:ring-2 focus:ring-indigo-500 focus:outline-none"
                        />
                      </div>
                    </div>
                  </div>
                )}

                {activeSettingsTab === 'llm' && (
                  <div className="space-y-4 pt-2">
                    <label className="flex items-center gap-3 text-sm font-medium text-white cursor-pointer select-none bg-black/30 p-4 rounded-2xl border border-white/5">
                      <input 
                        type="checkbox" 
                        checked={settings.llmEnabled}
                        onChange={e => setSettings({ ...settings, llmEnabled: e.target.checked })}
                        className="rounded text-indigo-500 bg-black/50 border-white/20 w-4 h-4"
                      />
                      <span>Activer l'auto-correction intelligente par IA (Orthographe / Ponctuation)</span>
                    </label>

                    <div className={!settings.llmEnabled ? 'opacity-40 pointer-events-none' : 'space-y-4'}>
                      <div>
                        <label className="block text-xs font-semibold text-gray-400 uppercase tracking-wider mb-1.5">Endpoint LLM (compatible OpenAI)</label>
                        <input 
                          type="text" 
                          value={settings.llmEndpoint}
                          onChange={e => setSettings({ ...settings, llmEndpoint: e.target.value })}
                          placeholder="https://api.openai.com/v1/chat/completions"
                          className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2.5 text-white text-sm focus:ring-2 focus:ring-indigo-500 focus:outline-none"
                        />
                      </div>
                      <div>
                        <label className="block text-xs font-semibold text-gray-400 uppercase tracking-wider mb-1.5">Clé API LLM</label>
                        <input 
                          type="password" 
                          value={settings.llmApiKey}
                          onChange={e => setSettings({ ...settings, llmApiKey: e.target.value })}
                          placeholder="sk-..."
                          className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2.5 text-white text-sm focus:ring-2 focus:ring-indigo-500 focus:outline-none"
                        />
                      </div>
                      <div>
                        <label className="block text-xs font-semibold text-gray-400 uppercase tracking-wider mb-1.5">Modèle LLM</label>
                        <input 
                          type="text" 
                          value={settings.llmModel}
                          onChange={e => setSettings({ ...settings, llmModel: e.target.value })}
                          placeholder="gpt-4o-mini"
                          className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2.5 text-white text-sm focus:ring-2 focus:ring-indigo-500 focus:outline-none"
                        />
                      </div>
                    </div>
                  </div>
                )}

                {activeSettingsTab === 'performance' && (
                  <div className="space-y-4 pt-2">
                    <div>
                      <label className="block text-xs font-semibold text-gray-400 uppercase tracking-wider mb-1.5 flex items-center gap-1.5">
                        <Cpu className="w-4 h-4 text-indigo-400" />
                        Accélération Matérielle Encodage Vidéo
                      </label>
                      <select 
                        value={settings.hardwareAccel || 'auto'}
                        onChange={e => setSettings({ ...settings, hardwareAccel: e.target.value })}
                        className="w-full bg-black/40 border border-white/10 rounded-xl px-3.5 py-2.5 text-white text-sm focus:ring-2 focus:ring-indigo-500 focus:outline-none cursor-pointer"
                      >
                        <option value="auto" className="bg-gray-900">Automatique (Détection)</option>
                        <option value="nvenc" className="bg-gray-900">NVIDIA NVENC (GPU matériel)</option>
                        <option value="cpu" className="bg-gray-900">CPU libx264 (Compatibilité standard)</option>
                      </select>
                    </div>
                  </div>
                )}

                <div className="pt-4 border-t border-white/10 flex justify-end">
                  <button 
                    onClick={handleSaveSettings}
                    className="bg-gradient-to-r from-indigo-600 to-purple-600 hover:from-indigo-500 hover:to-purple-500 shadow-lg shadow-indigo-500/25 text-white px-6 py-2.5 rounded-xl font-semibold text-sm transition-all shadow-lg shadow-indigo-500/25 flex items-center gap-2"
                  >
                    <Save className="w-4 h-4" />
                    Sauvegarder les Paramètres
                  </button>
                </div>
              </div>
            </motion.div>
          )}
        </AnimatePresence>

      {browserOpen && (
        <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/80 backdrop-blur-sm p-4">
          <div className="bg-[#0b0c10] border border-white/10 rounded-2xl shadow-2xl w-full max-w-2xl max-h-[80vh] flex flex-col overflow-hidden">
            <div className="p-4 border-b border-white/10 flex items-center justify-between bg-white/[0.02]">
              <h3 className="text-lg font-bold text-white">Sélectionner un dossier</h3>
              <button onClick={() => setBrowserOpen(false)} className="text-gray-400 hover:text-white"><X className="w-5 h-5" /></button>
            </div>
            <div className="p-3 bg-black/40 border-b border-white/5 text-sm font-mono text-gray-300 break-all">
              {browserPath}
            </div>
            <div className="flex-1 overflow-y-auto p-2">
              {browserPath !== '/' && (
                <div 
                  onClick={() => fetchBrowser(browserPath.split('/').slice(0, -1).join('/') || '/')}
                  className="flex items-center gap-3 p-3 hover:bg-white/5 rounded-xl cursor-pointer text-gray-400"
                >
                  <Folder className="w-5 h-5" /> .. (Retour)
                </div>
              )}
              {browserEntries.filter(e => e.is_dir).map(e => (
                <div 
                  key={e.path}
                  onClick={() => fetchBrowser(e.path)}
                  className="flex items-center gap-3 p-3 hover:bg-white/5 rounded-xl cursor-pointer text-gray-300"
                >
                  <Folder className="w-5 h-5 text-indigo-400" /> {e.name}
                </div>
              ))}
            </div>
            <div className="p-4 border-t border-white/10 flex justify-end gap-3 bg-white/[0.02]">
              <button onClick={() => setBrowserOpen(false)} className="px-4 py-2 text-sm text-gray-400 hover:text-white transition-colors">Annuler</button>
              <button 
                onClick={() => {
                  if (browserTarget) {
                    setWorkflows(wfs => wfs.map(w => w.id === browserTarget.wfId ? { ...w, [browserTarget.field]: browserPath } : w));
                  }
                  setBrowserOpen(false);
                }} 
                className="px-6 py-2 bg-indigo-600 hover:bg-indigo-500 text-white text-sm font-semibold rounded-xl shadow-lg shadow-indigo-500/20"
              >
                Sélectionner ce dossier
              </button>
            </div>
          </div>
        </div>
      )}

      </div>

      <AnimatePresence>
        {editingFileId && (
          <motion.div 
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 z-50 flex items-center justify-center p-3 sm:p-6 bg-black/80 backdrop-blur-md"
          >
            <motion.div 
              initial={{ scale: 0.96, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              exit={{ scale: 0.96, opacity: 0 }}
              className="bg-[#0b0c10] border border-white/15 rounded-3xl shadow-2xl w-full max-w-6xl max-h-[92vh] flex flex-col overflow-hidden"
            >
              <div className="p-4 sm:p-5 border-b border-white/10 flex flex-col md:flex-row justify-between items-start md:items-center gap-4 bg-white/[0.02]">
                <div>
                  <h3 className="text-lg font-bold text-white flex items-center gap-2">
                    <Edit3 className="w-5 h-5 text-indigo-400" />
                    Éditeur & Synchronisation: <span className="text-gray-400 font-mono text-sm">{activeEditingFile?.originalName}</span>
                  </h3>
                  <div className="flex flex-wrap gap-2 text-[11px] text-gray-400 mt-1 font-mono">
                    <span className="bg-white/5 px-2 py-0.5 rounded border border-white/10">{editingLines.length} blocs</span>
                    <span className="bg-emerald-500/10 text-emerald-300 px-2 py-0.5 rounded border border-emerald-500/20">0 overlap garanti</span>
                  </div>
                </div>

                <div className="flex flex-wrap items-center gap-2">
                  <button 
                    onClick={handleAutoFixOverlaps}
                    className="bg-emerald-600/20 hover:bg-emerald-600/30 text-emerald-300 border border-emerald-500/30 px-3 py-1.5 rounded-xl text-xs font-semibold transition-all flex items-center gap-1.5"
                    title="Vérifier et éliminer automatiquement tous les chevauchements"
                  >
                    <Check className="w-3.5 h-3.5" />
                    Auto-Fix Overlaps
                  </button>

                  <button 
                    onClick={() => setShowSearchReplace(!showSearchReplace)}
                    className="bg-white/5 hover:bg-white/10 text-gray-300 border border-white/10 px-3 py-1.5 rounded-xl text-xs font-medium transition-all flex items-center gap-1.5"
                  >
                    <Search className="w-3.5 h-3.5" />
                    Rechercher
                  </button>

                  <button 
                    onClick={() => setEditingFileId(null)}
                    className="p-1.5 text-gray-400 hover:text-white rounded-xl transition-colors"
                  >
                    <X className="w-6 h-6" />
                  </button>
                </div>
              </div>

              {showSearchReplace && (
                <div className="bg-indigo-950/40 border-b border-indigo-500/20 p-3 flex flex-wrap items-center gap-3 text-xs">
                  <input 
                    type="text" 
                    placeholder="Texte recherché..." 
                    value={searchQuery}
                    onChange={e => setSearchQuery(e.target.value)}
                    className="bg-black/50 border border-white/10 rounded-lg px-3 py-1.5 text-white"
                  />
                  <input 
                    type="text" 
                    placeholder="Remplacer par..." 
                    value={replaceQuery}
                    onChange={e => setReplaceQuery(e.target.value)}
                    className="bg-black/50 border border-white/10 rounded-lg px-3 py-1.5 text-white"
                  />
                  <button 
                    onClick={handleSearchReplace}
                    className="bg-gradient-to-r from-indigo-600 to-purple-600 hover:from-indigo-500 hover:to-purple-500 shadow-lg shadow-indigo-500/25 text-white px-3 py-1.5 rounded-lg font-semibold"
                  >
                    Remplacer Tout
                  </button>
                </div>
              )}

              <div className="flex-1 grid grid-cols-1 lg:grid-cols-12 overflow-hidden">
                <div className="lg:col-span-5 border-r border-white/10 p-4 flex flex-col items-center justify-center bg-black/40 space-y-4">
                  <div className="relative w-full bg-black rounded-2xl overflow-hidden border border-white/15 shadow-2xl flex items-center justify-center transition-all duration-500"
                    style={getAspectRatioStyle(currentPreset.aspectRatio)}>
                    
                    {currentPreset.aspectRatio === '9:16' && (
                      <div className="absolute inset-0 pointer-events-none z-10 border-[2px] border-dashed border-white/10">
                        {/* TikTok Right sidebar safe zone */}
                        <div className="absolute right-0 bottom-[20%] w-[60px] h-[30%] bg-red-500/20 border-l border-red-500/50 flex items-center justify-center">
                          <span className="text-[8px] text-red-200 rotate-90 whitespace-nowrap">UI TikTok</span>
                        </div>
                        {/* TikTok Bottom safe zone */}
                        <div className="absolute bottom-0 inset-x-0 h-[15%] bg-red-500/20 border-t border-red-500/50 flex items-center justify-center">
                          <span className="text-[10px] text-red-200">Zone de Description</span>
                        </div>
                      </div>
                    )}

                    {activeEditingFile && (
                      <video 
                        ref={videoPlayerRef}
                        src={activeEditingFile.videoUrl || `/api/video-stream/${activeEditingFile.id}`}
                        className="w-full h-full object-cover"
                        playsInline
                        onClick={() => {
                          if (videoPlayerRef.current) {
                            videoPlayerRef.current.paused ? videoPlayerRef.current.play() : videoPlayerRef.current.pause();
                          }
                        }}
                      />
                    )}

                    {currentActiveLine && (
                      <div 
                        className="absolute w-[90%] text-center pointer-events-none transition-all"
                        style={{
                          left: '50%',
                          top: `${currentPreset.positionY}%`,
                          transform: 'translate(-50%, -50%)',
                          fontFamily: `"${currentPreset.fontFamily}", sans-serif`,
                          fontSize: `${(currentPreset.size / 24) * 14}px`,
                          color: currentPreset.baseColor,
                          fontWeight: currentPreset.bold ? 'bold' : 'normal',
                          fontStyle: currentPreset.italic ? 'italic' : 'normal',
                          textTransform: currentPreset.uppercase ? 'uppercase' : 'none',
                          WebkitTextStroke: currentPreset.borderStyle === 1 ? `${currentPreset.outlineThickness}px ${currentPreset.outlineColor}` : 'none',
                          textShadow: currentPreset.borderStyle === 1 && currentPreset.shadowThickness ? 
                            `${currentPreset.shadowThickness}px ${currentPreset.shadowThickness}px 0px ${currentPreset.shadowColor}` : 'none',
                          backgroundColor: currentPreset.borderStyle === 3 ? (currentPreset.shadowColor || '#000000cc') : 'transparent',
                          padding: currentPreset.borderStyle === 3 ? '4px 8px' : '0',
                          borderRadius: currentPreset.borderStyle === 3 ? '6px' : '0'
                        }}
                      >
                        {currentActiveLine.text.split('\n').map((visualLine, vIdx) => (
                          <div key={vIdx} className="leading-tight">
                            {visualLine.split(' ').map((word, wIdx) => {
                              const activeWord = currentActiveLine.words?.find(w => currentTime >= w.start && currentTime <= w.end);
                              const isHighlighted = activeWord?.word.toLowerCase().replace(/[^a-z0-9]/gi, '') === word.toLowerCase().replace(/[^a-z0-9]/gi, '');
                              return (
                                <span 
                                  key={wIdx}
                                  style={{
                                    color: isHighlighted ? currentPreset.highlightColor : currentPreset.baseColor,
                                    display: 'inline-block',
                                    transform: isHighlighted && currentPreset.animationStyle === 'pop' ? 'scale(1.15)' : 'none',
                                    margin: '0 2px'
                                  }}
                                >
                                  {word}
                                </span>
                              );
                            })}
                          </div>
                        ))}
                      </div>
                    )}
                  </div>

                  <div className="w-full max-w-[280px] flex items-center justify-between text-xs text-gray-400 font-mono">
                    <span>{currentTime.toFixed(1)}s</span>
                    <div className="flex items-center gap-2">
                      <button 
                        onClick={() => {
                          if (videoPlayerRef.current) {
                            videoPlayerRef.current.currentTime = Math.max(0, videoPlayerRef.current.currentTime - 1);
                          }
                        }}
                        className="p-1 hover:text-white"
                      >
                        <Rewind className="w-4 h-4" />
                      </button>
                      <button 
                        onClick={() => {
                          if (videoPlayerRef.current) {
                            videoPlayerRef.current.paused ? videoPlayerRef.current.play() : videoPlayerRef.current.pause();
                          }
                        }}
                        className="p-2 bg-gradient-to-r from-indigo-600 to-purple-600 hover:from-indigo-500 hover:to-purple-500 shadow-lg shadow-indigo-500/25 text-white rounded-full"
                      >
                        {isPlaying ? <Pause className="w-4 h-4" /> : <Play className="w-4 h-4 fill-current" />}
                      </button>
                      <button 
                        onClick={() => {
                          if (videoPlayerRef.current) {
                            videoPlayerRef.current.currentTime += 1;
                          }
                        }}
                        className="p-1 hover:text-white"
                      >
                        <FastForward className="w-4 h-4" />
                      </button>
                    </div>
                    <button 
                      onClick={() => handleAddLine()}
                      className="text-xs text-indigo-400 hover:text-indigo-300 font-medium flex items-center gap-1"
                    >
                      <Plus className="w-3.5 h-3.5" /> Ligne
                    </button>
                  </div>
                </div>

                <div className="lg:col-span-7 flex flex-col overflow-hidden">
                  <div className="p-3 border-b border-white/10 bg-black/20 flex flex-wrap items-center justify-between gap-3 text-xs">
                    <div className="flex items-center gap-2">
                      <input 
                        type="number" 
                        value={shiftMs}
                        onChange={e => setShiftMs(Number(e.target.value))}
                        placeholder="ms"
                        className="w-16 bg-black/50 border border-white/10 rounded px-2 py-1 text-white font-mono"
                      />
                      <button 
                        onClick={handleApplyShift}
                        className="px-2.5 py-1 bg-white/10 hover:bg-white/20 text-white rounded font-medium"
                      >
                        Décaler Tout
                      </button>
                    </div>

                    <div className="flex items-center gap-2">
                      <button 
                        onClick={handleRegroup}
                        className="px-2.5 py-1 bg-purple-600/20 hover:bg-purple-600/30 text-purple-300 border border-purple-500/30 rounded font-medium flex items-center gap-1"
                      >
                        <Wand2 className="w-3.5 h-3.5" />
                        Regrouper ({editorMaxChars}c/{editorMaxLines}l)
                      </button>
                    </div>
                  </div>

                  <div className="flex-1 overflow-y-auto p-4 space-y-2.5">
                    {editingLines.map((line, index) => {
                      const isActive = currentTime >= line.start && currentTime <= line.end;
                      const hasOverlap = index < editingLines.length - 1 && line.end > editingLines[index + 1].start;

                      return (
                        <div 
                          key={line.id || index}
                          onClick={() => {
                            if (videoPlayerRef.current) videoPlayerRef.current.currentTime = line.start;
                          }}
                          className={`p-3 rounded-2xl border transition-all flex flex-col sm:flex-row items-start sm:items-center gap-3 cursor-pointer ${
                            isActive ? 'bg-indigo-950/40 border-indigo-500/50 shadow-md shadow-indigo-500/10 ring-1 ring-indigo-500/30' :
                            hasOverlap ? 'bg-red-950/20 border-red-500/40' :
                            'bg-black/30 border-white/5 hover:border-white/15'
                          }`}
                        >
                          <div className="flex sm:flex-col items-center sm:items-start gap-1 font-mono text-xs text-gray-400 shrink-0 w-full sm:w-28">
                            <div className="flex items-center gap-1">
                              <input 
                                type="number" 
                                step="0.1"
                                value={parseFloat(line.start.toFixed(2))}
                                onChange={e => {
                                  const newStart = Math.max(0, parseFloat(e.target.value) || 0);
                                  const updated = [...editingLines];
                                  updated[index] = { ...updated[index], start: newStart };
                                  setEditingLines(fixOverlaps(updated));
                                }}
                                onClick={e => e.stopPropagation()}
                                className="w-14 bg-black/60 border border-white/10 rounded px-1.5 py-0.5 text-xs text-white"
                              />
                              <span>→</span>
                              <input 
                                type="number" 
                                step="0.1"
                                value={parseFloat(line.end.toFixed(2))}
                                onChange={e => {
                                  const newEnd = Math.max(line.start + 0.05, parseFloat(e.target.value) || 0);
                                  const updated = [...editingLines];
                                  updated[index] = { ...updated[index], end: newEnd };
                                  setEditingLines(fixOverlaps(updated));
                                }}
                                onClick={e => e.stopPropagation()}
                                className="w-14 bg-black/60 border border-white/10 rounded px-1.5 py-0.5 text-xs text-white"
                              />
                            </div>
                            <span className="text-[10px] text-gray-500">{(line.end - line.start).toFixed(1)}s</span>
                          </div>

                          <div className="flex-1 w-full" onClick={e => e.stopPropagation()}>
                            <textarea 
                              value={line.text}
                              onChange={e => {
                                const newText = e.target.value;
                                const updated = [...editingLines];
                                updated[index] = { ...updated[index], text: newText };
                                setEditingLines(fixOverlaps(updated));
                              }}
                              className="w-full bg-transparent border-0 border-b border-white/10 focus:border-indigo-500 focus:ring-0 text-sm text-white resize-none py-1"
                              rows={line.text.split('\n').length || 1}
                            />
                          </div>

                          <div className="flex items-center gap-1 shrink-0 ml-auto" onClick={e => e.stopPropagation()}>
                            <button 
                              onClick={() => handleAddLine(index)}
                              className="p-1.5 text-gray-400 hover:text-white rounded-lg hover:bg-white/5"
                              title="Insérer une ligne après"
                            >
                              <Plus className="w-3.5 h-3.5" />
                            </button>
                            <button 
                              onClick={() => handleDeleteLine(index)}
                              className="p-1.5 text-gray-400 hover:text-red-400 rounded-lg hover:bg-red-500/10"
                              title="Supprimer la ligne"
                            >
                              <Trash2 className="w-3.5 h-3.5" />
                            </button>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </div>
              </div>

              <div className="p-4 border-t border-white/10 bg-white/[0.02] flex justify-between items-center">
                <div className="text-xs text-gray-400">
                  <span>Astuce: Les chevauchements et mots sont automatiquement ajustés en temps réel.</span>
                </div>

                <div className="flex items-center gap-3">
                  <button 
                    onClick={() => setEditingFileId(null)}
                    className="px-4 py-2 rounded-xl text-xs font-semibold text-gray-400 hover:text-white hover:bg-white/5 transition-all"
                  >
                    Annuler
                  </button>
                  <button 
                    onClick={saveEditedLines}
                    className="bg-gradient-to-r from-indigo-600 to-purple-600 hover:from-indigo-500 hover:to-purple-500 shadow-lg shadow-indigo-500/25 text-white px-5 py-2 rounded-xl text-xs font-bold transition-all shadow-lg shadow-indigo-500/25 flex items-center gap-1.5"
                  >
                    <Save className="w-4 h-4" />
                    Enregistrer les Corrections
                  </button>
                </div>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

      {browserOpen && (
        <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/80 backdrop-blur-sm p-4">
          <div className="bg-[#0b0c10] border border-white/10 rounded-2xl shadow-2xl w-full max-w-2xl max-h-[80vh] flex flex-col overflow-hidden">
            <div className="p-4 border-b border-white/10 flex items-center justify-between bg-white/[0.02]">
              <h3 className="text-lg font-bold text-white">Sélectionner un dossier</h3>
              <button onClick={() => setBrowserOpen(false)} className="text-gray-400 hover:text-white"><X className="w-5 h-5" /></button>
            </div>
            <div className="p-3 bg-black/40 border-b border-white/5 text-sm font-mono text-gray-300 break-all">
              {browserPath}
            </div>
            <div className="flex-1 overflow-y-auto p-2">
              {browserPath !== '/' && (
                <div 
                  onClick={() => fetchBrowser(browserPath.split('/').slice(0, -1).join('/') || '/')}
                  className="flex items-center gap-3 p-3 hover:bg-white/5 rounded-xl cursor-pointer text-gray-400"
                >
                  <Folder className="w-5 h-5" /> .. (Retour)
                </div>
              )}
              {browserEntries.filter(e => e.is_dir).map(e => (
                <div 
                  key={e.path}
                  onClick={() => fetchBrowser(e.path)}
                  className="flex items-center gap-3 p-3 hover:bg-white/5 rounded-xl cursor-pointer text-gray-300"
                >
                  <Folder className="w-5 h-5 text-indigo-400" /> {e.name}
                </div>
              ))}
            </div>
            <div className="p-4 border-t border-white/10 flex justify-end gap-3 bg-white/[0.02]">
              <button onClick={() => setBrowserOpen(false)} className="px-4 py-2 text-sm text-gray-400 hover:text-white transition-colors">Annuler</button>
              <button 
                onClick={() => {
                  if (browserTarget) {
                    setWorkflows(wfs => wfs.map(w => w.id === browserTarget.wfId ? { ...w, [browserTarget.field]: browserPath } : w));
                  }
                  setBrowserOpen(false);
                }} 
                className="px-6 py-2 bg-indigo-600 hover:bg-indigo-500 text-white text-sm font-semibold rounded-xl shadow-lg shadow-indigo-500/20"
              >
                Sélectionner ce dossier
              </button>
            </div>
          </div>
        </div>
      )}

    </div>
  );
}
