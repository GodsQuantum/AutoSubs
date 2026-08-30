<script lang="ts">
  import type { FormatProfile, Preset } from '$lib/types';
  export let format: FormatProfile;
  export let preset: Preset | undefined = undefined;
  export let text = 'THIS IS A PREVIEW';
  export let sourceRatio = 16 / 9;
  $: ratio = format.key === 'portrait916' ? 9/16 : format.key === 'landscape169' ? 16/9 : format.key === 'square11' ? 1 : format.key === 'portrait45' ? 4/5 : format.key === 'custom' && format.width && format.height ? format.width/format.height : sourceRatio;
  $: safeRatio = Math.max(.38, Math.min(2.2, ratio));
  $: style = `aspect-ratio:${safeRatio};`;
  $: p = preset;
  $: subtitleStyle = p ? `top:${p.positionY}%;left:${Math.max(2,p.positionX-46)}%;right:${Math.max(2,54-p.positionX)}%;font-size:${Math.max(13,Math.min(34,p.size*.72))}px;color:${p.baseColor};font-family:${p.fontFamily},sans-serif;font-weight:${p.bold?800:500};font-style:${p.italic?'italic':'normal'};text-transform:${p.uppercase?'uppercase':'none'};-webkit-text-stroke:${Math.max(0,p.outlineThickness*.45)}px ${p.outlineColor};` : '';
</script>
<div class="format-preview" style={style}>
  <div class="preview-subtitle" style={subtitleStyle}>{text}</div>
</div>
