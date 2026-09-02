<script lang="ts">
  import { onMount } from 'svelte';
  import type { FontFace, FormatProfile, Preset, SubtitleWord } from '$lib/types';
  import { activeWordIndex, customFontMatch, clampPreviewPosition, demoSubtitleWords, formatRatio, karaokeProgress, loopedPreviewTime, previewSubtitleTokens, previewWidthForRatio, safeZoneGuide, subtitlePositionBounds, scalePreviewMetric, videoObjectFit } from '$lib/preview.js';

  type SafeZoneKey = 'off'|'generic'|'tiktok'|'reels'|'shorts';

  export let format: FormatProfile;
  export let preset: Preset | undefined = undefined;
  export let fonts: FontFace[] = [];
  export let text = 'THIS IS A PREVIEW';
  export let words: SubtitleWord[] | undefined = undefined;
  export let currentTime = 0;
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
  let demoPlaying = true;
  let demoElapsed = 0;
  let lastFrame:number|undefined;
  let videoElement:HTMLVideoElement|undefined;
  let frameElement:HTMLDivElement|undefined;
  let measuredWidth=0;
  let measuredHeight=0;
  const demoDuration = 3;

  onMount(()=>{
    let resizeObserver:ResizeObserver|undefined;
    const measure=()=>{if(frameElement){measuredWidth=frameElement.clientWidth;measuredHeight=frameElement.clientHeight;}};
    measure();
    if(typeof ResizeObserver!=="undefined"){resizeObserver=new ResizeObserver(measure);if(frameElement)resizeObserver.observe(frameElement);}
    else window.addEventListener("resize",measure);
    let frame=0;
    const tick=(now:number)=>{
      if(demoPlaying && !videoSrc){
        if(lastFrame!==undefined) demoElapsed+=Math.max(0,now-lastFrame)/1000;
        lastFrame=now;
      } else {
        lastFrame=undefined;
        if(videoSrc && videoElement && !videoElement.paused) onVideoTimeUpdate(videoElement.currentTime);
      }
      frame=requestAnimationFrame(tick);
    };
    frame=requestAnimationFrame(tick);
    return ()=>{cancelAnimationFrame(frame);resizeObserver?.disconnect();window.removeEventListener("resize",measure);};
  });

  $: effectiveSourceRatio = naturalWidth > 0 && naturalHeight > 0 ? naturalWidth / naturalHeight : sourceRatio;
  $: ratio = formatRatio(format, effectiveSourceRatio);
  $: previewWidth = previewWidthForRatio(ratio);
  $: objectFit = videoObjectFit(format);
  $: guide = safeZoneGuide(safeZone);
  $: outputHeight = format.key === 'portrait916' ? 1920 : format.key === 'landscape169' ? 1080 : format.key === 'square11' ? 1080 : format.key === 'portrait45' ? 1350 : format.key === 'custom' && format.height ? Number(format.height) : naturalHeight || 1080;
  $: displayWidth = measuredWidth || previewWidth;
  $: displayHeight = measuredHeight || previewWidth / ratio;
  $: p = preset;
  $: requestedWeight = p?.bold ? 700 : 400;
  $: selectedFace = p ? customFontMatch(fonts,p.fontFamily,requestedWeight,Boolean(p.italic)) : null;
  $: renderedWeight = selectedFace?.weight ?? requestedWeight;
  $: renderedItalic = selectedFace?.italic ?? Boolean(p?.italic);
  $: timedWords = words?.length ? words : demoSubtitleWords(text).map(word=>({...word,start:word.start*demoDuration,end:word.end*demoDuration}));
  $: renderedFamily = selectedFace?.fullName ?? selectedFace?.family ?? p?.fontFamily.split(',')[0].trim() ?? "sans-serif";
  $: previewWords = previewSubtitleTokens(text, timedWords);
  $: previewTime = videoSrc ? currentTime : loopedPreviewTime(demoElapsed,demoDuration);
  $: activeWord = activeWordIndex(previewWords, previewTime);
  $: animation = p?.animationStyle ?? 'none';
  $: eventTime = Math.max(0, previewTime - (previewWords[0]?.start ?? 0));
  $: wobbleDuration = 1 / Math.max(0.05, Number(p?.wobbleSpeed) || 1);
  $: motionStyle = `--preview-time:${previewTime};--event-time:${eventTime};--wobble-duration:${wobbleDuration}s;`;
  $: fontSize = p ? scalePreviewMetric(p.size,displayHeight,outputHeight) : 18;
  $: lineHeight = 1.08;
  $: visualLines = Math.max(1, text.split('\n').filter(line=>line.trim()).length);
  $: longestLine = Math.max(1, ...text.split('\n').map(line=>Array.from(line).length));
  $: estimatedBlockWidth = Math.min(displayWidth * .9, longestLine * fontSize * .56);
  $: estimatedBlockHeight = visualLines * fontSize + Math.max(0,visualLines-1) * scalePreviewMetric(p?.lineSpacing ?? 0,displayHeight,outputHeight);
  $: positionBounds = subtitlePositionBounds(displayWidth, displayHeight, estimatedBlockWidth, estimatedBlockHeight);
  $: outlineSize = p ? scalePreviewMetric(p.outlineThickness,displayHeight,outputHeight) : 0;
  $: shadowSize = p ? scalePreviewMetric(p.shadowThickness ?? 1,displayHeight,outputHeight) : 0;
  $: safePosition = clampPreviewPosition(p?.positionX ?? 50, p?.positionY ?? 68, positionBounds);
  $: subtitleStyle = p
    ? `top:${safePosition.y}%;left:${safePosition.x}%;font-size:${fontSize}px;line-height:${lineHeight};color:${p.baseColor};font-family:"${renderedFamily.replaceAll('"','\\"')}";font-weight:${renderedWeight};font-style:${renderedItalic?'italic':'normal'};font-synthesis:none;text-transform:${p.uppercase?'uppercase':'none'};-webkit-text-stroke:${outlineSize}px ${p.outlineColor};text-shadow:0 ${shadowSize}px ${shadowSize*2}px ${p.shadowColor??'#000000'};`
    : `top:${safePosition.y}%;left:${safePosition.x}%;font-size:${fontSize}px;line-height:${lineHeight};color:#fff;font-weight:800;`;
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
      ((event.clientY - rect.top) / Math.max(1, rect.height)) * 100,
      positionBounds
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
    const position = clampPreviewPosition(x, y, positionBounds);
    onPositionChange(position.x, position.y);
  }

  function toggleDemo(){demoPlaying=!demoPlaying;lastFrame=undefined;}
  function restartDemo(){demoElapsed=0;lastFrame=undefined;demoPlaying=true;}
</script>

<div bind:this={frameElement} class={`format-preview${videoSrc ? " has-video" : ""}${Boolean(videoSrc) && format.key === "source" && !naturalWidth ? " awaiting-source" : ""}`} style={`width:${previewWidth}px;aspect-ratio:${ratio};`}>
  {#if videoSrc}
    <video
      bind:this={videoElement}
      id={videoId || undefined}
      src={videoSrc}
      {controls}
      preload="metadata"
      playsinline
      style={`object-fit:${objectFit}`}
      on:loadedmetadata={(e)=>{const v=e.currentTarget as HTMLVideoElement;naturalWidth=v.videoWidth;naturalHeight=v.videoHeight;}}
      on:timeupdate={(e)=>onVideoTimeUpdate((e.currentTarget as HTMLVideoElement).currentTime)}
      on:pause={(e)=>onVideoTimeUpdate((e.currentTarget as HTMLVideoElement).currentTime)}
      on:seeked={(e)=>onVideoTimeUpdate((e.currentTarget as HTMLVideoElement).currentTime)}
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
  {#if !videoSrc}
    <div class="demo-controls">
      <button type="button" on:click={toggleDemo} aria-label={demoPlaying?'Pause preview':'Play preview'} aria-pressed={!demoPlaying}>
        {#if demoPlaying}
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 5v14M15 5v14"/></svg>
        {:else}
          <svg viewBox="0 0 24 24" aria-hidden="true" class="play-icon"><path d="m8 5 11 7-11 7z"/></svg>
        {/if}
      </button>
      <button type="button" on:click={restartDemo} aria-label="Restart preview">
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 11a8 8 0 1 0-2.34 5.66M20 5v6h-6"/></svg>
      </button>
    </div>
  {/if}

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
      ><span class:floating={p?.floating} class="preview-floating" style={motionStyle}><span class={`preview-animation-inner animation-${animation}`} style={motionStyle}>{#each previewWords as word, i}{#if word.separator}<span aria-hidden="true">{word.separator}</span>{/if}<span class:active={i === activeWord} class={`preview-word word-animation-${animation}`} style={`${motionStyle}--word-time:${Math.max(0,previewTime-word.start)};--base-color:${p?.baseColor ?? '#fff'};--highlight-color:${p?.highlightColor ?? '#3dd7cf'};--karaoke-progress:${karaokeProgress(previewTime,word.start,word.end)}%;color:${i === activeWord && (animation === 'pop' || animation === 'highlight' || animation === 'bounce' || animation === 'karaoke') ? (p?.highlightColor ?? '#3dd7cf') : (p?.baseColor ?? '#fff')}`}>{word.word}</span>{/each}</span></span></button>
    {:else}
      <div class="preview-subtitle" style={subtitleStyle}><span class:floating={p?.floating} class="preview-floating" style={motionStyle}><span class={`preview-animation-inner animation-${animation}`} style={motionStyle}>{#each previewWords as word, i}{#if word.separator}<span aria-hidden="true">{word.separator}</span>{/if}<span class:active={i === activeWord} class={`preview-word word-animation-${animation}`} style={`${motionStyle}--word-time:${Math.max(0,previewTime-word.start)};--base-color:${p?.baseColor ?? '#fff'};--highlight-color:${p?.highlightColor ?? '#3dd7cf'};--karaoke-progress:${karaokeProgress(previewTime,word.start,word.end)}%;color:${i === activeWord && (animation === 'pop' || animation === 'highlight' || animation === 'bounce' || animation === 'karaoke') ? (p?.highlightColor ?? '#3dd7cf') : (p?.baseColor ?? '#fff')}`}>{word.word}</span>{/each}</span></span></div>
    {/if}
  {/if}
</div>

<style>
  .format-preview { position:relative; max-width:100%; border-radius:12px; background:#030506; box-shadow:0 18px 54px rgba(0,0,0,.38); overflow:hidden; isolation:isolate; transition:none; }
  .format-preview:not(.has-video) { background:radial-gradient(circle at 24% 8%,#38505a 0%,#182329 42%,#06090a 100%); }
  .format-preview.awaiting-source { visibility:hidden; }
  .preview-art { position:absolute; inset:0; background:linear-gradient(145deg,rgba(255,255,255,.08),transparent 38%),radial-gradient(circle at 70% 72%,rgba(61,215,207,.12),transparent 32%); }
  video { width:100%; height:100%; display:block; background:#000; }
  .preview-meta { position:absolute; z-index:5; top:10px; left:10px; padding:5px 8px; border:1px solid rgba(255,255,255,.13); border-radius:999px; background:rgba(3,6,7,.72); backdrop-filter:blur(8px); color:#d9e4e5; font:700 10px/1.1 Inter,ui-sans-serif,system-ui,sans-serif; letter-spacing:.025em; pointer-events:none; }
  .preview-meta span { color:#839398; font-weight:650; }
  .demo-controls { position:absolute; z-index:8; top:9px; right:9px; display:flex; gap:5px; }
  .demo-controls button { width:44px; height:44px; padding:0; border:1px solid rgba(255,255,255,.16); border-radius:7px; background:rgba(3,6,7,.76); color:#e8f2f2; cursor:pointer; }
  .demo-controls svg { width:18px; height:18px; fill:none; stroke:currentColor; stroke-width:2; stroke-linecap:round; stroke-linejoin:round; }
  .demo-controls .play-icon { fill:currentColor; stroke:none; }
  .preview-subtitle { position:absolute; z-index:6; transform:translate(-50%,-50%); width:max-content; max-width:92%; margin:0; padding:0; border:0; appearance:none; background:transparent; text-align:center; line-height:1.08; white-space:pre-line; overflow-wrap:normal; word-break:normal; pointer-events:none; }
  .preview-subtitle.editable { pointer-events:auto; cursor:grab; touch-action:none; padding:5px 7px; border-radius:6px; outline:1px dashed rgba(61,215,207,.42); }
  .preview-subtitle.editable:active { cursor:grabbing; outline-color:rgba(61,215,207,.9); }
  .preview-floating, .preview-animation-inner, .preview-word { display:inline-block; }
  .preview-floating.floating { animation:preview-floating var(--wobble-duration) ease-in-out infinite; animation-delay:calc(var(--preview-time) * -1s); animation-play-state:paused; }
  .animation-fade { animation:preview-fade .45s ease both; animation-delay:calc(var(--event-time) * -1s); animation-play-state:paused; }
  .animation-slide-up { animation:preview-slide-up .45s ease both; animation-delay:calc(var(--event-time) * -1s); animation-play-state:paused; }
  .word-animation-pop.active { animation:preview-pop .32s ease both; animation-delay:calc(var(--word-time) * -1s); animation-play-state:paused; }
  .word-animation-bounce.active { animation:preview-bounce .38s ease both; animation-delay:calc(var(--word-time) * -1s); animation-play-state:paused; }
  .word-animation-karaoke.active { color:transparent !important; background:linear-gradient(90deg,var(--highlight-color) 0 var(--karaoke-progress),var(--base-color) var(--karaoke-progress)); background-clip:text; -webkit-background-clip:text; }
  .animation-highlight, .animation-karaoke, .animation-none { animation:none; }
  @keyframes preview-fade { from { opacity:0; } to { opacity:1; } }
  @keyframes preview-slide-up { from { opacity:0; transform:translateY(18px); } to { opacity:1; transform:translateY(0); } }
  @keyframes preview-pop { 0% { transform:scale(1); } 43% { transform:scale(1.12); } 100% { transform:scale(1); } }
  @keyframes preview-bounce { 0% { transform:translateY(0) scale(1); } 45% { transform:translateY(-4px) scale(1.05); } 100% { transform:translateY(0) scale(1); } }
  @keyframes preview-floating { 0%,100% { transform:rotate(-1.5deg); } 50% { transform:rotate(1.5deg); } }
  .safe-risk { position:absolute; z-index:3; pointer-events:none; background:rgba(255,128,125,.08); }
  .safe-risk-top { top:0; left:0; right:0; }
  .safe-risk-bottom { bottom:0; left:0; right:0; }
  .safe-risk-left { top:0; bottom:0; left:0; }
  .safe-risk-right { top:0; bottom:0; right:0; }
  .safe-zone-box { position:absolute; z-index:4; border:1px dashed rgba(113,216,157,.72); border-radius:7px; pointer-events:none; box-shadow:0 0 0 1px rgba(0,0,0,.18); }
  .safe-zone-box span { position:absolute; top:5px; left:7px; color:#9be6b7; font:800 9px/1 Inter,ui-sans-serif,system-ui,sans-serif; text-transform:uppercase; letter-spacing:.07em; text-shadow:0 1px 2px #000; }
  @media (prefers-reduced-motion:reduce) { .format-preview { transition:none; } .preview-floating.floating, .preview-animation-inner, .preview-word { animation:none !important; transform:none !important; } }
</style>
