export type Locale = 'en' | 'fr';
export type JobStatus = 'pending'|'uploading'|'probing'|'transcribing'|'correcting'|'ready'|'rendering'|'done'|'error'|'cancelled'|'interrupted';
export type RenderProfile = 'auto'|'fast'|'quality'|'compact';
export type FormatKey = 'source'|'portrait916'|'landscape169'|'square11'|'portrait45'|'custom';
export type FitMode = 'preserve'|'contain'|'cover'|'stretch';
export interface FormatProfile { key: FormatKey; fit: FitMode; width?: number; height?: number }
export type TimingQuality = 'exact'|'inferred'|'aligned';
export interface SubtitleWord { word:string; start:number; end:number }
export type FontSource = 'app'|'system';
export interface FontFace { id:string; family:string; fullName:string; style:string; weight:number; italic:boolean; fileName:string; source:FontSource }
export interface SubtitleLine { id:number; start:number; end:number; text:string; words?:SubtitleWord[] }
export type AnimationStyle = 'pop'|'highlight'|'karaoke'|'word-by-word'|'fade'|'slide-up'|'bounce'|'none';
export interface Preset {
  id:string; name:string; brandId?:string; format:FormatProfile; animationStyle:AnimationStyle; size:number; positionX:number; positionY:number;
  baseColor:string; outlineColor:string; highlightColor:string; fontFamily:string; uppercase:boolean; outlineThickness:number; shadowThickness?:number;
  shadowOffsetX?:number; shadowOffsetY?:number; shadowBlur?:number; shadowOpacity?:number; shadowColor?:string; borderStyle:number; floating:boolean; maxChars:number; maxLines:number; wobbleSpeed:number; bold:boolean; italic:boolean;
  matchKeywords?:string; lineSpacing:number; outroVideo?:string;
}
export interface BrandAssets { defaultOutro?:string; logo?:string }
export interface Brand { id:string; name:string; description:string; assets:BrandAssets; presetIds:string[]; defaultPresetByFormat:Partial<Record<FormatKey,string>>; matchKeywords?:string; highlightColor?:string }
export type WorkflowOutput = 'video-only'|'video-srt';
export interface Workflow { id:string; name:string; watchDir:string; outputDir:string; archiveDir:string; outputMode:WorkflowOutput; brandId?:string; format:FormatProfile; presetId?:string; enabled:boolean }
export type JobOutro = {mode:'inherit'} | {mode:'none'} | {mode:'asset';assetId:string};
export interface Job { id:string; originalName:string; status:JobStatus; progress?:number; lines?:SubtitleLine[]; error?:string; inputPath?:string; outputPath?:string; presetId?:string; effectivePreset?:Preset; resolvedBrandId?:string; timingQuality?:TimingQuality; timingFallback?:string; format:FormatProfile; renderProfile:RenderProfile; lastRenderEncoder?:EncoderKind; lastRenderElapsedMs?:number; outro:JobOutro; workflowId?:string; archiveAfterSuccess:boolean; attachedSidecar?:string; createdAtMs:number; updatedAtMs:number }
export type EncoderKind = 'auto'|'libx264'|'libx265'|'nvenc_h264'|'nvenc_hevc'|'qsv_h264'|'vaapi_h264'|'vulkan_h264'|'amf_h264';
export type EstimateBasis = 'initial'|'history';
export interface RenderEstimate { minSeconds:number; maxSeconds:number; basis:EstimateBasis; sampleCount:number }
export interface RenderProfileOption { profile:RenderProfile; encoder:EncoderKind; estimate:RenderEstimate }
export interface RenderOptions { options:RenderProfileOption[]; actualEncoder?:EncoderKind; lastElapsedMs?:number }
export interface Encoder { kind:EncoderKind; quality:number; preset:string }
export interface SettingsView {
  transcriptionUrl:string; transcriptionModel:string; transcriptionApiKeySet:boolean; language:string;
  localTranscriptionEnabled:boolean; localFallbackEnabled:boolean; localTranscriptionUrl:string; localTranscriptionModel:string; localTranscriptionApiKeySet:boolean;
  alignmentEnabled:boolean; alignmentUrl:string; alignmentModel:string; alignmentApiKeySet:boolean;
  llmEnabled:boolean; llmEndpoint:string; llmModel:string; llmPrompt:string; llmApiKeySet:boolean; encoder:Encoder;
}
export interface Asset { id:string; name:string; storedFile:string; mime:string; size:number; createdAtMs:number }
export interface BrowseEntry { name:string; path:string; isDir:boolean; size?:number; modifiedMs?:number; selectable:boolean }
export interface BrowseResponse { currentPath:string; parentPath?:string; entries:BrowseEntry[]; roots:string[]; favorites:string[] }
export interface Capabilities { ffmpeg:boolean; h264Nvenc:boolean; hevcNvenc:boolean; h264Qsv:boolean; h264Vaapi:boolean; h264Vulkan:boolean; h264Amf:boolean; vaapiDevice?:string; vulkanDevice?:string; h264BenchmarksMs:Record<string,number>; autoEncoderOrder:EncoderKind[]; libass:boolean }
