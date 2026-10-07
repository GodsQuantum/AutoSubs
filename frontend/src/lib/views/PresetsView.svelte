<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { dictionary } from '$lib/i18n';
  import type { Asset, Brand, FontFace, FormatKey, Preset } from '$lib/types';
  import FormatPreview from '$lib/components/FormatPreview.svelte';
  import NumberSlider from '$lib/components/NumberSlider.svelte';
  import { customFontMatch, verifyCustomFont } from '$lib/preview.js';

  export let presets:Preset[]=[];
  export let brands:Brand[]=[];
  export let fonts:FontFace[]=[];
  export let assets:Asset[]=[];
  export let refresh:()=>Promise<void>=async()=>{};
  export let notify:(type:'error'|'success'|'info',message:string)=>void=()=>{};

  type SafeZoneKey = 'off'|'generic'|'tiktok'|'reels'|'shorts';
  let selected='';
  let draft:Preset=makePreset();
  let previousFormatKey:FormatKey=draft.format.key;
  let safeZone:SafeZoneKey='generic';
  let fontStylesheetRevision=0;
  let fontCheckRevision=0;
  let fontStatus:{status:string;fileName:string}|null=null;
  let fontInput:HTMLInputElement|undefined;

  $: current=presets.find(p=>p.id===selected);
  $: selectedCustomFont=customFontMatch(fonts,draft.fontFamily,draft.bold?700:400,draft.italic);
  $: videoAssets=assets.filter(asset=>asset.mime.startsWith('video/'));
  $: if(draft.fontFamily && fontStylesheetRevision>=0) checkSelectedFont(draft.fontFamily,selectedCustomFont,fontStylesheetRevision);
  $: if(current && draft.id!==current.id) draft=clone(current);
  $: if(draft.format.key!==previousFormatKey){
    previousFormatKey=draft.format.key;
    if(draft.format.key==='source') draft.format.fit='preserve';
    else if(draft.format.fit==='preserve') draft.format.fit='cover';
  }

  function makePreset():Preset{return {
    id:'',name:'New preset',format:{key:'source',fit:'preserve'},animationStyle:'pop',
    size:28,positionX:50,positionY:68,baseColor:'#ffffff',outlineColor:'#000000',
    highlightColor:'#3dd7cf',fontFamily:'Inter',uppercase:false,outlineThickness:2.5,
    shadowOffsetX:1.5,shadowOffsetY:1.5,shadowBlur:0,shadowOpacity:100,shadowColor:'#000000',
    borderStyle:1,floating:false,maxChars:25,maxLines:2,wobbleSpeed:1,bold:true,italic:false,lineSpacing:0
  };}
  const clone=(p:Preset):Preset=>JSON.parse(JSON.stringify(p));
  function create(){selected='';draft=makePreset();previousFormatKey=draft.format.key}
  function selectFont(event:Event){
    const font=fonts.find(candidate=>candidate.id===(event.currentTarget as HTMLSelectElement).value);
    if(font)draft={...draft,fontFamily:font.fullName||font.family,bold:font.weight>=600,italic:font.italic};
  }
  function duplicate(){draft={...clone(draft),id:'',name:`${draft.name} copy`};selected='';previousFormatKey=draft.format.key}
  async function save(){
    if(draft.format.key==='custom' && (!Number.isInteger(Number(draft.format.width))||!Number.isInteger(Number(draft.format.height))||Number(draft.format.width)<16||Number(draft.format.height)<16||Number(draft.format.width)>16384||Number(draft.format.height)>16384||Number(draft.format.width)%2!==0||Number(draft.format.height)%2!==0)){notify('error',`${$dictionary.custom}: ${$dictionary.width} × ${$dictionary.height}`);return;}
    try{const saved=await api.savePreset({...draft,shadowThickness:undefined,outroVideo:draft.outroVideo||undefined});selected=saved.id;draft=clone(saved);await refresh();notify('success',$dictionary.saved)}catch(e){notify('error',e instanceof Error?e.message:String(e))}
  }
  async function remove(){if(!draft.id||!confirm($dictionary.confirmDelete))return;try{await api.deletePreset(draft.id);selected='';draft=makePreset();await refresh()}catch(e){notify('error',e instanceof Error?e.message:String(e))}}
  async function uploadFont(file:File){
    try{
      const imported=await api.uploadFont(file);
      draft={...draft,fontFamily:imported.fullName||imported.family,bold:imported.weight>=600,italic:imported.italic};
      const stylesheet=document.querySelector<HTMLLinkElement>('link[href^="/api/v1/fonts/css"]');
      if(stylesheet)stylesheet.href=`/api/v1/fonts/css?rev=${Date.now()}`;
      await refresh();
      fontStylesheetRevision+=1;
      notify('success',`Font importée · ${imported.fullName||imported.family}`);
    }catch(e){notify('error',e instanceof Error?e.message:String(e))}
  }
  async function checkSelectedFont(family:string,font:FontFace|null,_stylesheetRevision:number){
    const revision=++fontCheckRevision;
    if(!font || font.source!=='app'){fontStatus=null;return;}
    if(typeof document==='undefined')return;
    fontStatus=null;
    const result=await verifyCustomFont(document.fonts,font);
    if(revision===fontCheckRevision && family===draft.fontFamily)fontStatus=result;
  }
  onMount(()=>{
    const stylesheet=document.querySelector<HTMLLinkElement>('link[href^="/api/v1/fonts/css"]');
    const loaded=()=>fontStylesheetRevision+=1;
    stylesheet?.addEventListener('load',loaded);
    document.fonts?.addEventListener('loadingdone',loaded);
    document.fonts?.ready.then(loaded);
    return ()=>{stylesheet?.removeEventListener('load',loaded);document.fonts?.removeEventListener('loadingdone',loaded);};
  });
</script>

<div class="page">
  <div class="page-head">
    <div><h1 class="page-title">{$dictionary.presets}</h1><p class="page-kicker">{$dictionary.sourcePreserveHint}</p></div>
    <div class="page-actions"><button class="btn" on:click={create}>＋ {$dictionary.newPreset}</button><button class="btn" on:click={duplicate}>{$dictionary.duplicate}</button><button class="btn primary" on:click={save}>{$dictionary.save}</button></div>
  </div>

  <div class="split-editor">
    <div class="list-pane">
      {#if presets.length===0}<div class="empty">{$dictionary.noPresets}</div>{/if}
      {#each presets as p}<button class="list-item" class:active={selected===p.id} on:click={()=>selected=p.id}><strong>{p.name}</strong><span>{p.animationStyle} · {p.format.key}</span></button>{/each}
    </div>

    <div class="stack">
      <section class="card">
        <div class="card-header"><strong>{draft.name}</strong>{#if draft.id}<button class="btn danger" on:click={remove}>{$dictionary.delete}</button>{/if}</div>
        <div class="card-body grid two">
          <div class="field"><label for="presets-field-1">{$dictionary.presetName}</label><input id="presets-field-1" class="input" bind:value={draft.name}/></div>
          <div class="field"><label for="presets-field-2">{$dictionary.brand}</label><select id="presets-field-2" class="select" bind:value={draft.brandId}><option value="">{$dictionary.noBrand}</option>{#each brands as b}<option value={b.id}>{b.name}</option>{/each}</select></div>
          <div class="field"><label for="presets-field-3">{$dictionary.format}</label><select id="presets-field-3" class="select" bind:value={draft.format.key}><option value="source">{$dictionary.sourceFormat}</option><option value="portrait916">9:16</option><option value="landscape169">16:9</option><option value="square11">1:1</option><option value="portrait45">4:5</option><option value="custom">{$dictionary.custom}</option></select></div>
          <div class="field"><label for="presets-field-4">{$dictionary.fit}</label><select id="presets-field-4" class="select" bind:value={draft.format.fit} disabled={draft.format.key==='source'}><option value="preserve">{$dictionary.preserve}</option><option value="contain">{$dictionary.contain}</option><option value="cover">{$dictionary.cover}</option><option value="stretch">{$dictionary.stretch}</option></select></div>
          {#if draft.format.key==='custom'}<div class="field"><label for="presets-field-5">{$dictionary.width}</label><input id="presets-field-5" class="input" type="number" min="16" max="16384" step="2" bind:value={draft.format.width}/></div><div class="field"><label for="presets-field-6">{$dictionary.height}</label><input id="presets-field-6" class="input" type="number" min="16" max="16384" step="2" bind:value={draft.format.height}/></div>{/if}
          <div class="field"><label for="presets-outro">{$dictionary.outro}</label><select id="presets-outro" class="select" value={draft.outroVideo??''} on:change={(event)=>draft={...draft,outroVideo:(event.currentTarget as HTMLSelectElement).value||undefined}}><option value="">{$dictionary.noOutro}</option>{#each videoAssets as asset}<option value={asset.id}>{asset.name}</option>{/each}</select></div>
        </div>
      </section>

      <div class="grid two">
        <section class="card">
          <div class="card-header"><strong>{$dictionary.styling}</strong></div>
          <div class="card-body stack">
            <div class="grid two">
              <div class="field"><label for="presets-field-7">{$dictionary.animation}</label><select id="presets-field-7" class="select" bind:value={draft.animationStyle}><option value="pop">{$dictionary.pop}</option><option value="highlight">{$dictionary.highlight}</option><option value="karaoke">{$dictionary.karaoke}</option><option value="word-by-word">{$dictionary.wordByWord}</option><option value="fade">{$dictionary.fade}</option><option value="slide-up">{$dictionary.slideUp}</option><option value="bounce">{$dictionary.bounce}</option><option value="none">{$dictionary.animationNone}</option></select></div>
              <div class="field">
                <label for="presets-field-8">{$dictionary.font}</label>
                <div class="row"><select id="presets-field-8" class="select" value={selectedCustomFont?.id ?? ""} on:change={selectFont}>{#if !selectedCustomFont}<option value="">{draft.fontFamily}</option>{/if}{#each fonts as font}<option value={font.id}>{font.source==='app'?'App':'Système'} · {font.fullName || font.family} · {font.style}</option>{/each}</select><button class="btn" type="button" on:click={()=>fontInput?.click()}>＋ Font</button></div>
                <input hidden bind:this={fontInput} type="file" accept=".ttf,.otf,.ttc,.otc" on:change={(event)=>{const input=event.currentTarget as HTMLInputElement;const file=input.files?.[0];if(file)uploadFont(file);input.value='';}}/>
                {#if selectedCustomFont}<span class="help">{selectedCustomFont.source==='app'?'Font app persistante':'Font système serveur'} · {selectedCustomFont.fileName}</span>{/if}
                {#if selectedCustomFont?.source==='app' && fontStatus}<span class="help" class:font-loaded={fontStatus.status === "loaded"}>{fontStatus.status === "loaded" ? "✓ Font chargée" : "⚠ Preview navigateur en fallback"} · {fontStatus.fileName}</span>{/if}
              </div>
            </div>
            <div class="grid three"><div class="field"><label for="presets-field-9">{$dictionary.baseColor}</label><input id="presets-field-9" class="input" type="color" bind:value={draft.baseColor}/></div><div class="field"><label for="presets-field-10">{$dictionary.highlightColor}</label><input id="presets-field-10" class="input" type="color" bind:value={draft.highlightColor}/></div><div class="field"><label for="presets-field-11">{$dictionary.outlineColor}</label><input id="presets-field-11" class="input" type="color" bind:value={draft.outlineColor}/></div></div>
            <div class="grid two"><NumberSlider label={$dictionary.size} value={draft.size} min={8} max={160} step={1} onChange={(value)=>draft={...draft,size:value}}/><NumberSlider label={$dictionary.outline} value={draft.outlineThickness} min={0} max={12} step={0.1} onChange={(value)=>draft={...draft,outlineThickness:value}}/></div>
            <div class="row wrap"><label class="check"><input type="checkbox" bind:checked={draft.bold}/>{$dictionary.bold}</label><label class="check"><input type="checkbox" bind:checked={draft.italic}/>{$dictionary.italic}</label><label class="check"><input type="checkbox" bind:checked={draft.uppercase}/>{$dictionary.uppercase}</label><label class="check"><input type="checkbox" bind:checked={draft.floating}/>{$dictionary.floating}</label></div>

            <div class="divider"></div>
            <strong class="small">Ombre portée</strong>
            <div class="grid two"><NumberSlider label="Distance X" value={draft.shadowOffsetX??0} min={-20} max={20} step={0.25} unit="px" onChange={(value)=>draft={...draft,shadowOffsetX:value}}/><NumberSlider label="Distance Y" value={draft.shadowOffsetY??0} min={-20} max={20} step={0.25} unit="px" onChange={(value)=>draft={...draft,shadowOffsetY:value}}/></div>
            <div class="grid two"><NumberSlider label="Flou" value={draft.shadowBlur??0} min={0} max={20} step={0.25} unit="px" onChange={(value)=>draft={...draft,shadowBlur:value}}/><NumberSlider label="Opacité" value={draft.shadowOpacity??100} min={0} max={100} step={1} unit="%" onChange={(value)=>draft={...draft,shadowOpacity:value}}/></div>
            <div class="field"><label for="shadow-color">Couleur de l’ombre</label><input id="shadow-color" class="input" type="color" bind:value={draft.shadowColor}/></div>
          </div>
        </section>

        <section class="card">
          <div class="card-header"><strong>{$dictionary.placement}</strong></div>
          <div class="card-body stack">
            <div class="grid two"><NumberSlider label={$dictionary.positionX} value={draft.positionX} min={0} max={100} step={0.5} unit="%" onChange={(value)=>draft={...draft,positionX:value}}/><NumberSlider label={$dictionary.positionY} value={draft.positionY} min={0} max={100} step={0.5} unit="%" onChange={(value)=>draft={...draft,positionY:value}}/></div>
            <div class="grid two"><div class="field"><label for="presets-field-16">{$dictionary.maxChars}</label><input id="presets-field-16" class="input" type="number" min="5" max="42" bind:value={draft.maxChars}/></div><div class="field"><label for="presets-field-17">{$dictionary.maxLines}</label><input id="presets-field-17" class="input" type="number" min="1" max="2" bind:value={draft.maxLines}/></div></div>
            <div class="field"><label for="presets-field-18">{$dictionary.keywords}</label><input id="presets-field-18" class="input" bind:value={draft.matchKeywords} placeholder="shorts, reels, interview"/></div>
            <div class="grid two"><NumberSlider label={$dictionary.lineSpacing} value={draft.lineSpacing} min={-20} max={40} step={0.5} unit="px" onChange={(value)=>draft={...draft,lineSpacing:value}}/><NumberSlider label={$dictionary.wobbleSpeed} value={draft.wobbleSpeed} min={0.1} max={5} step={0.1} onChange={(value)=>draft={...draft,wobbleSpeed:value}}/></div>
          </div>
        </section>
      </div>

      <section class="card">
        <div class="card-header"><strong>{$dictionary.preview} · libass</strong><div class="segmented"><button class:active={safeZone==='off'} on:click={()=>safeZone='off'}>{$dictionary.none}</button><button class:active={safeZone==='generic'} on:click={()=>safeZone='generic'}>Generic</button><button class:active={safeZone==='tiktok'} on:click={()=>safeZone='tiktok'}>TikTok</button><button class:active={safeZone==='reels'} on:click={()=>safeZone='reels'}>Reels</button><button class:active={safeZone==='shorts'} on:click={()=>safeZone='shorts'}>Shorts</button></div></div>
        <div class="card-body"><div class="preview-shell"><FormatPreview format={draft.format} preset={draft} {fonts} text={$dictionary.sampleText} {safeZone} editable={true} onPositionChange={(x,y)=>{draft.positionX=Math.round(x*2)/2;draft.positionY=Math.round(y*2)/2;draft={...draft};}}/></div></div>
      </section>
    </div>
  </div>
</div>
