<script lang="ts">
  import type { FormatProfile, Preset } from '$lib/types';
  import { clampPreviewPosition, formatRatio, previewWidthForRatio, safeZoneGuide, videoObjectFit } from '$lib/preview.js';

  type SafeZoneKey = 'off'|'generic'|'tiktok'|'reels'|'shorts';

  export let format: FormatProfile;
  export let preset: Preset | undefined = undefined;
  export let text = 'THIS IS A PREVIEW';
  export let sourceRatio = 16 / 9;
  export let videoSrc = '';
  export let controls = false;
  export let videoId = '';
  export let captionsSrc = '';
  export let safeZone: SafeZoneKey = 'off';
  export let onVideoTimeUpdate: (time:number)=>void = ()=>{};
  export let editable = false;
  export let onPositionChange: (x:number,y:number)=>void = ()=>{};

  let naturalWidth = 0;
  let naturalHeight = 0;

  $: effectiveSourceRatio = naturalWidth > 0 && naturalHeight > 0 ? naturalWidth / naturalHeight : sourceRatio;
  $: ratio = formatRatio(format, effectiveSourceRatio);
  $: previewWidth = previewWidthForRatio(ratio);
  $: objectFit = videoObjectFit(format);
  $: guide = safeZoneGuide(safeZone);
  $: outputHeight = format.key === 'portrait916' ? 1920 : format.key === 'landscape169' ? 1080 : format.key === 'square11' ? 1080 : format.key === 'portrait45' ? 1350 : format.key === 'custom' && format.height ? Number(format.height) : naturalHeight || 1080;
  $: displayHeight = previewWidth / ratio;
  $: p = preset;
  $: fontSize = p ? Math.max(10, Math.min(52, p.size * displayHeight / Math.max(1, outputHeight))) : 18;
  $: subtitleStyle = p
    ? `top:${p.positionY}%;left:${p.positionX}%;font-size:${fontSize}px;color:${p.baseColor};font-family:${p.fontFamily},sans-serif;font-weight:${p.bold?800:500};font-style:${p.italic?'italic':'normal'};text-transform:${p.uppercase?'uppercase':'none'};-webkit-text-stroke:${Math.max(0,p.outlineThickness*displayHeight/Math.max(1,outputHeight))}px ${p.outlineColor};text-shadow:0 ${Math.max(1,(p.shadowThickness??1)*displayHeight/Math.max(1,outputHeight))}px ${Math.max(2,(p.shadowThickness??1)*2*displayHeight/Math.max(1,outputHeight))}px ${p.shadowColor??'#000000'};`
    : `top:68%;left:50%;font-size:${fontSize}px;color:#fff;font-weight:800;`;
  $: formatLabel = format.key === 'source' ? 'Source' : format.key === 'portrait916' ? '9:16' : format.key === 'landscape169' ? '16:9' : format.key === 'square11' ? '1:1' : format.key === 'portrait45' ? '4:5' : `${format.width || '?'}×${format.height || '?'}`;
  $: resolutionLabel = format.key === 'portrait916' ? '1080×1920' : format.key === 'landscape169' ? '1920×1080' : format.key === 'square11' ? '1080×1080' : format.key === 'portrait45' ? '1080×1350' : format.key === 'custom' ? `${format.width || '?'}×${format.height || '?'}` : naturalWidth && naturalHeight ? `${naturalWidth}×${naturalHeight}` : '';

  function updatePosition(event: PointerEvent) {
    if (!editable) return;
    const target = event.currentTarget as HTMLElement;
    const frame = target.closest('.format-preview') as HTMLElement | null;
    if (!frame) return;
    const rect = frame.getBoundingClientRect();
    const position = clampPreviewPosition(
      ((event.clientX - rect.left) / Math.max(1, rect.width)) * 100,
      ((event.clientY - rect.top) / Math.max(1, rect.height)) * 100
    );
    onPositionChange(position.x, position.y);
  }

  function startDrag(event: PointerEvent) {
    if (!editable) return;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    updatePosition(event);
  }

  function drag(event: PointerEvent) {
    const target = event.currentTarget as HTMLElement;
    if (editable && target.hasPointerCapture(event.pointerId)) updatePosition(event);
  }

  function stopDrag(event: PointerEvent) {
    const target = event.currentTarget as HTMLElement;
    if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId);
  }

  function nudgePosition(event: KeyboardEvent) {
    if (!editable || !p) return;
    const step = event.shiftKey ? 5 : 1;
    let x = p.positionX;
    let y = p.positionY;

    if (event.key === 'ArrowLeft') x -= step;
    else if (event.key === 'ArrowRight') x += step;
    else if (event.key === 'ArrowUp') y -= step;
    else if (event.key === 'ArrowDown') y += step;
    else return;

    event.preventDefault();
    const position = clampPreviewPosition(x, y);
    onPositionChange(position.x, position.y);
  }
</script>

<div class={`format-preview${videoSrc ? ' has-video' : ''}`} style={`width:${previewWidth}px;aspect-ratio:${ratio};`}>
  {#if videoSrc}
    <video
      id={videoId || undefined}
      src={videoSrc}
      {controls}
      preload="metadata"
      playsinline
      style={`object-fit:${objectFit}`}
      on:loadedmetadata={(e)=>{const v=e.currentTarget as HTMLVideoElement;naturalWidth=v.videoWidth;naturalHeight=v.videoHeight;}}
      on:timeupdate={(e)=>onVideoTimeUpdate((e.currentTarget as HTMLVideoElement).currentTime)}
    >
      <track
        kind="captions"
        src={captionsSrc || 'data:text/vtt;charset=utf-8,WEBVTT%0A%0A'}
        srclang="und"
        label="Subtitles"
      />
    </video>
  {:else}
    <div class="preview-art" aria-hidden="true"></div>
  {/if}

  <div class="preview-meta">{formatLabel}{#if resolutionLabel}<span> · {resolutionLabel}</span>{/if}</div>

  {#if guide}
    <div class="safe-risk safe-risk-top" style={`height:${guide.top*100}%`}></div>
    <div class="safe-risk safe-risk-bottom" style={`height:${guide.bottom*100}%`}></div>
    <div class="safe-risk safe-risk-left" style={`width:${guide.left*100}%`}></div>
    <div class="safe-risk safe-risk-right" style={`width:${guide.right*100}%`}></div>
    <div class="safe-zone-box" style={`inset:${guide.top*100}% ${guide.right*100}% ${guide.bottom*100}% ${guide.left*100}%`}><span>{guide.label}</span></div>
  {/if}

  {#if text.trim()}
    {#if editable}
      <button
        type="button"
        class="preview-subtitle editable"
        style={subtitleStyle}
        aria-label="Subtitle position"
        on:pointerdown={startDrag}
        on:pointermove={drag}
        on:pointerup={stopDrag}
        on:pointercancel={stopDrag}
        on:keydown={nudgePosition}
      >{text}</button>
    {:else}
      <div class="preview-subtitle" style={subtitleStyle}>{text}</div>
    {/if}
  {/if}
</div>

<style>
  .format-preview { position:relative; max-width:100%; border-radius:12px; background:#030506; box-shadow:0 18px 54px rgba(0,0,0,.38); overflow:hidden; isolation:isolate; transition:width .18s ease,aspect-ratio .18s ease; }
  .format-preview:not(.has-video) { background:radial-gradient(circle at 24% 8%,#38505a 0%,#182329 42%,#06090a 100%); }
  .preview-art { position:absolute; inset:0; background:linear-gradient(145deg,rgba(255,255,255,.08),transparent 38%),radial-gradient(circle at 70% 72%,rgba(61,215,207,.12),transparent 32%); }
  video { width:100%; height:100%; display:block; background:#000; }
  .preview-meta { position:absolute; z-index:5; top:10px; left:10px; padding:5px 8px; border:1px solid rgba(255,255,255,.13); border-radius:999px; background:rgba(3,6,7,.72); backdrop-filter:blur(8px); color:#d9e4e5; font:700 10px/1.1 Inter,ui-sans-serif,system-ui,sans-serif; letter-spacing:.025em; pointer-events:none; }
  .preview-meta span { color:#839398; font-weight:650; }
  .preview-subtitle { position:absolute; z-index:6; transform:translate(-50%,-50%); width:max-content; max-width:92%; margin:0; padding:0; border:0; appearance:none; background:transparent; text-align:center; line-height:1.08; white-space:pre-line; overflow-wrap:anywhere; pointer-events:none; }
  .preview-subtitle.editable { pointer-events:auto; cursor:grab; touch-action:none; padding:5px 7px; border-radius:6px; outline:1px dashed rgba(61,215,207,.42); }
  .preview-subtitle.editable:active { cursor:grabbing; outline-color:rgba(61,215,207,.9); }
  .safe-risk { position:absolute; z-index:3; pointer-events:none; background:rgba(255,128,125,.08); }
  .safe-risk-top { top:0; left:0; right:0; }
  .safe-risk-bottom { bottom:0; left:0; right:0; }
  .safe-risk-left { top:0; bottom:0; left:0; }
  .safe-risk-right { top:0; bottom:0; right:0; }
  .safe-zone-box { position:absolute; z-index:4; border:1px dashed rgba(113,216,157,.72); border-radius:7px; pointer-events:none; box-shadow:0 0 0 1px rgba(0,0,0,.18); }
  .safe-zone-box span { position:absolute; top:5px; left:7px; color:#9be6b7; font:800 9px/1 Inter,ui-sans-serif,system-ui,sans-serif; text-transform:uppercase; letter-spacing:.07em; text-shadow:0 1px 2px #000; }
  @media (prefers-reduced-motion:reduce) { .format-preview { transition:none; } }
</style>
