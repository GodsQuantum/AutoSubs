<script lang="ts">
  import { api } from '$lib/api';
  import { dictionary } from '$lib/i18n';
  import type { Brand, FontFace, FormatKey, Preset } from '$lib/types';
  import FormatPreview from '$lib/components/FormatPreview.svelte';
  export let presets:Preset[]=[];
  export let brands:Brand[]=[];
  export let fonts:FontFace[]=[];
  export let refresh:()=>Promise<void>=async()=>{};
  export let notify:(type:'error'|'success'|'info',message:string)=>void=()=>{};

  type SafeZoneKey = 'off'|'generic'|'tiktok'|'reels'|'shorts';
  let selected='';
  let draft:Preset=makePreset();
  let previousFormatKey:FormatKey=draft.format.key;
  let safeZone:SafeZoneKey='generic';
  $: current=presets.find(p=>p.id===selected);
  $: fontFamilies=[...new Set([draft.fontFamily, ...fonts.map(font=>font.family)].filter(Boolean))];
  $: if(current && draft.id!==current.id) draft=clone(current);
  $: if(draft.format.key!==previousFormatKey){
    previousFormatKey=draft.format.key;
    if(draft.format.key==='source') draft.format.fit='preserve';
    else if(draft.format.fit==='preserve') draft.format.fit='cover';
  }

  function makePreset():Preset{return {id:'',name:'New preset',format:{key:'source',fit:'preserve'},animationStyle:'pop',size:28,positionX:50,positionY:68,baseColor:'#ffffff',outlineColor:'#000000',highlightColor:'#3dd7cf',fontFamily:'Inter',uppercase:false,outlineThickness:2.5,shadowThickness:1.2,shadowColor:'#000000',borderStyle:1,floating:false,maxChars:25,maxLines:2,wobbleSpeed:1,bold:true,italic:false,lineSpacing:0};}
  const clone=(p:Preset):Preset=>JSON.parse(JSON.stringify(p));
  function create(){selected='';draft=makePreset();previousFormatKey=draft.format.key}
  function duplicate(){draft={...clone(draft),id:'',name:`${draft.name} copy`};selected='';previousFormatKey=draft.format.key}
  async function save(){
    if(draft.format.key==='custom' && (!Number.isInteger(Number(draft.format.width))||!Number.isInteger(Number(draft.format.height))||Number(draft.format.width)<16||Number(draft.format.height)<16||Number(draft.format.width)>16384||Number(draft.format.height)>16384||Number(draft.format.width)%2!==0||Number(draft.format.height)%2!==0)){notify('error',`${$dictionary.custom}: ${$dictionary.width} × ${$dictionary.height}`);return;}
    try{const saved=await api.savePreset(draft);selected=saved.id;draft=clone(saved);await refresh();notify('success',$dictionary.saved)}catch(e){notify('error',e instanceof Error?e.message:String(e))}
  }
  async function remove(){if(!draft.id||!confirm($dictionary.confirmDelete))return;try{await api.deletePreset(draft.id);selected='';draft=makePreset();await refresh()}catch(e){notify('error',e instanceof Error?e.message:String(e))}}
</script>
<div class="page">
  <div class="page-head"><div><h1 class="page-title">{$dictionary.presets}</h1><p class="page-kicker">{$dictionary.sourcePreserveHint}</p></div><div class="page-actions"><button class="btn" on:click={create}>＋ {$dictionary.newPreset}</button><button class="btn" on:click={duplicate}>{$dictionary.duplicate}</button><button class="btn primary" on:click={save}>{$dictionary.save}</button></div></div>
  <div class="split-editor">
    <div class="list-pane">
      {#if presets.length===0}<div class="empty">{$dictionary.noPresets}</div>{/if}
      {#each presets as p}<button class="list-item" class:active={selected===p.id} on:click={()=>selected=p.id}><strong>{p.name}</strong><span>{p.animationStyle} · {p.format.key}</span></button>{/each}
    </div>
    <div class="stack">
      <section class="card"><div class="card-header"><strong>{draft.name}</strong>{#if draft.id}<button class="btn danger" on:click={remove}>{$dictionary.delete}</button>{/if}</div><div class="card-body grid two">
        <div class="field"><label for="presets-field-1">{$dictionary.presetName}</label><input id="presets-field-1" class="input" bind:value={draft.name}/></div>
        <div class="field"><label for="presets-field-2">{$dictionary.brand}</label><select id="presets-field-2" class="select" bind:value={draft.brandId}><option value="">{$dictionary.noBrand}</option>{#each brands as b}<option value={b.id}>{b.name}</option>{/each}</select></div>
        <div class="field"><label for="presets-field-3">{$dictionary.format}</label><select id="presets-field-3" class="select" bind:value={draft.format.key}><option value="source">{$dictionary.sourceFormat}</option><option value="portrait916">9:16</option><option value="landscape169">16:9</option><option value="square11">1:1</option><option value="portrait45">4:5</option><option value="custom">{$dictionary.custom}</option></select></div>
        <div class="field"><label for="presets-field-4">{$dictionary.fit}</label><select id="presets-field-4" class="select" bind:value={draft.format.fit} disabled={draft.format.key==='source'}><option value="preserve">{$dictionary.preserve}</option><option value="contain">{$dictionary.contain}</option><option value="cover">{$dictionary.cover}</option><option value="stretch">{$dictionary.stretch}</option></select></div>
        {#if draft.format.key==='custom'}<div class="field"><label for="presets-field-5">{$dictionary.width}</label><input id="presets-field-5" class="input" type="number" min="16" max="16384" step="2" bind:value={draft.format.width}/></div><div class="field"><label for="presets-field-6">{$dictionary.height}</label><input id="presets-field-6" class="input" type="number" min="16" max="16384" step="2" bind:value={draft.format.height}/></div>{/if}
      </div></section>

      <div class="grid two">
        <section class="card"><div class="card-header"><strong>{$dictionary.styling}</strong></div><div class="card-body stack">
          <div class="grid two"><div class="field"><label for="presets-field-7">{$dictionary.animation}</label><select id="presets-field-7" class="select" bind:value={draft.animationStyle}><option value="pop">{$dictionary.pop}</option><option value="highlight">{$dictionary.highlight}</option><option value="karaoke">{$dictionary.karaoke}</option><option value="fade">{$dictionary.fade}</option><option value="slide-up">{$dictionary.slideUp}</option><option value="bounce">{$dictionary.bounce}</option><option value="none">{$dictionary.animationNone}</option></select></div><div class="field"><label for="presets-field-8">{$dictionary.font}</label><select id="presets-field-8" class="select" bind:value={draft.fontFamily}>{#each fontFamilies as family}<option value={family}>{family}</option>{/each}</select></div></div>
          <div class="grid three"><div class="field"><label for="presets-field-9">{$dictionary.baseColor}</label><input id="presets-field-9" class="input" type="color" bind:value={draft.baseColor}/></div><div class="field"><label for="presets-field-10">{$dictionary.highlightColor}</label><input id="presets-field-10" class="input" type="color" bind:value={draft.highlightColor}/></div><div class="field"><label for="presets-field-11">{$dictionary.outlineColor}</label><input id="presets-field-11" class="input" type="color" bind:value={draft.outlineColor}/></div></div>
          <div class="grid two"><div class="field"><label for="presets-field-12">{$dictionary.size}</label><input id="presets-field-12" class="input" type="number" min="8" max="160" bind:value={draft.size}/></div><div class="field"><label for="presets-field-13">{$dictionary.outline}</label><input id="presets-field-13" class="input" type="number" step=".1" min="0" bind:value={draft.outlineThickness}/></div></div>
          <div class="row wrap"><label class="check"><input type="checkbox" bind:checked={draft.bold}/>{$dictionary.bold}</label><label class="check"><input type="checkbox" bind:checked={draft.italic}/>{$dictionary.italic}</label><label class="check"><input type="checkbox" bind:checked={draft.uppercase}/>{$dictionary.uppercase}</label><label class="check"><input type="checkbox" bind:checked={draft.floating}/>{$dictionary.floating}</label></div>
        </div></section>
        <section class="card"><div class="card-header"><strong>{$dictionary.placement}</strong></div><div class="card-body stack">
          <div class="grid two"><div class="field"><label for="presets-field-14">{$dictionary.positionX}</label><input id="presets-field-14" class="input" type="range" min="0" max="100" bind:value={draft.positionX}/><span class="help">{draft.positionX}%</span></div><div class="field"><label for="presets-field-15">{$dictionary.positionY}</label><input id="presets-field-15" class="input" type="range" min="0" max="100" bind:value={draft.positionY}/><span class="help">{draft.positionY}%</span></div></div>
          <div class="grid two"><div class="field"><label for="presets-field-16">{$dictionary.maxChars}</label><input id="presets-field-16" class="input" type="number" min="5" bind:value={draft.maxChars}/></div><div class="field"><label for="presets-field-17">{$dictionary.maxLines}</label><input id="presets-field-17" class="input" type="number" min="1" max="4" bind:value={draft.maxLines}/></div></div>
          <div class="field"><label for="presets-field-18">{$dictionary.keywords}</label><input id="presets-field-18" class="input" bind:value={draft.matchKeywords} placeholder="shorts, reels, interview"/></div>
          <div class="grid two"><div class="field"><label for="presets-field-19">{$dictionary.lineSpacing}</label><input id="presets-field-19" class="input" type="number" step=".5" bind:value={draft.lineSpacing}/></div><div class="field"><label for="presets-field-20">{$dictionary.wobbleSpeed}</label><input id="presets-field-20" class="input" type="number" step=".1" bind:value={draft.wobbleSpeed}/></div></div>
        </div></section>
      </div>
      <section class="card">
        <div class="card-header"><strong>{$dictionary.preview}</strong><div class="segmented"><button class:active={safeZone==='off'} on:click={()=>safeZone='off'}>{$dictionary.none}</button><button class:active={safeZone==='generic'} on:click={()=>safeZone='generic'}>Generic</button><button class:active={safeZone==='tiktok'} on:click={()=>safeZone='tiktok'}>TikTok</button><button class:active={safeZone==='reels'} on:click={()=>safeZone='reels'}>Reels</button><button class:active={safeZone==='shorts'} on:click={()=>safeZone='shorts'}>Shorts</button></div></div>
        <div class="card-body"><div class="preview-shell"><FormatPreview format={draft.format} preset={draft} text={$dictionary.sampleText} {safeZone} editable={true} onPositionChange={(x,y)=>{draft.positionX=Math.round(x);draft.positionY=Math.round(y);draft={...draft};}}/></div></div>
      </section>
    </div>
  </div>
</div>
