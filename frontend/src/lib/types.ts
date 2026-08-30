export type Locale = 'en' | 'fr';
export type JobStatus = 'pending'|'uploading'|'probing'|'transcribing'|'correcting'|'ready'|'rendering'|'done'|'error'|'cancelled'|'interrupted';
export type FormatKey = 'source'|'portrait916'|'landscape169'|'square11'|'portrait45'|'custom';
export type FitMode = 'preserve'|'contain'|'cover'|'stretch';
export interface FormatProfile { key: FormatKey; fit: FitMode; width?: number; height?: number }
export interface SubtitleWord { word:string; start:number; end:number }
export interface SubtitleLine { id:number; start:number; end:number; text:string; words?:SubtitleWord[] }
export type AnimationStyle = 'pop'|'karaoke'|'fade'|'slide-up'|'bounce'|'none';
export interface Preset {
  id:string; name:string; brandId?:string; format:FormatProfile; animationStyle:AnimationStyle; size:number; positionX:number; positionY:number;
  baseColor:string; outlineColor:string; highlightColor:string; fontFamily:string; uppercase:boolean; outlineThickness:number; shadowThickness?:number;
  shadowColor?:string; borderStyle:number; floating:boolean; maxChars:number; maxLines:number; wobbleSpeed:number; bold:boolean; italic:boolean;
  matchKeywords?:string; lineSpacing:number; outroVideo?:string;
}
export interface BrandAssets { defaultOutro?:string; logo?:string }
export interface Brand { id:string; name:string; description:string; assets:BrandAssets; presetIds:string[]; defaultPresetByFormat:Partial<Record<FormatKey,string>> }
export interface Workflow { id:string; name:string; watchDir:string; outputDir:string; archiveDir:string; brandId?:string; format:FormatProfile; presetId?:string; enabled:boolean }
export interface Job { id:string; originalName:string; status:JobStatus; progress?:number; lines?:SubtitleLine[]; error?:string; inputPath?:string; outputPath?:string; presetId?:string; format:FormatProfile; workflowId?:string; archiveAfterSuccess:boolean; attachedSidecar?:string; createdAtMs:number; updatedAtMs:number }
export interface Encoder { kind:'auto'|'libx264'|'libx265'|'nvenc_h264'|'nvenc_hevc'|'qsv_h264'|'vaapi_h264'|'amf_h264'; quality:number; preset:string }
export interface SettingsView {
  transcriptionUrl:string; transcriptionModel:string; transcriptionApiKeySet:boolean; language:string;
  localTranscriptionEnabled:boolean; localFallbackEnabled:boolean; localTranscriptionUrl:string; localTranscriptionModel:string; localTranscriptionApiKeySet:boolean;
  llmEnabled:boolean; llmEndpoint:string; llmModel:string; llmPrompt:string; llmApiKeySet:boolean; encoder:Encoder;
}
export interface Asset { id:string; name:string; storedFile:string; mime:string; size:number; createdAtMs:number }
export interface BrowseEntry { name:string; path:string; isDir:boolean; size?:number; modifiedMs?:number; selectable:boolean }
export interface BrowseResponse { currentPath:string; parentPath?:string; entries:BrowseEntry[]; roots:string[] }
export interface Capabilities { ffmpeg:boolean; h264Nvenc:boolean; hevcNvenc:boolean; h264Qsv:boolean; h264Vaapi:boolean; h264Amf:boolean; libass:boolean }
