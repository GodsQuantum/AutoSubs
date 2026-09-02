<script lang="ts">
  import { api, assetUrl } from '$lib/api';
  import { dictionary } from '$lib/i18n';
  import type { Asset, Brand, FormatKey, Preset } from '$lib/types';
  import PathPicker from '$lib/components/PathPicker.svelte';
  export let brands:Brand[]=[];
  export let presets:Preset[]=[];
  export let assets:Asset[]=[];
  export let refresh:()=>Promise<void>=async()=>{};
  export let notify:(type:'error'|'success'|'info',message:string)=>void=()=>{};
  let selected=''; let draft:Brand=makeBrand(); let assetInput:HTMLInputElement; let importPicker=false;
  $: current=brands.find(b=>b.id===selected); $: if(current&&draft.id!==current.id)draft=clone(current);
  $: logoAsset=assets.find(a=>a.id===draft.assets.logo); $: outroAsset=assets.find(a=>a.id===draft.assets.defaultOutro);
  const clone=(b:Brand):Brand=>JSON.parse(JSON.stringify(b));
  function makeBrand():Brand{return{id:'',name:'',description:'',assets:{},presetIds:[],defaultPresetByFormat:{}}}
  function create(){selected='';draft=makeBrand()}
  function setDefault(key:FormatKey,value:string){draft.defaultPresetByFormat={...draft.defaultPresetByFormat,[key]:value||undefined};if(!value)delete draft.defaultPresetByFormat[key];draft={...draft};}
  async function save(){try{const b=await api.saveBrand(draft);selected=b.id;draft=clone(b);await refresh();notify('success',$dictionary.saved)}catch(e){notify('error',e instanceof Error?e.message:String(e))}}
  async function remove(){if(!draft.id||!confirm($dictionary.confirmDelete))return;try{await api.deleteBrand(draft.id);selected='';draft=makeBrand();await refresh()}catch(e){notify('error',e instanceof Error?e.message:String(e))}}
  async function uploadAsset(file:File){try{await api.uploadAsset(file);await refresh();notify('success',$dictionary.saved)}catch(e){notify('error',e instanceof Error?e.message:String(e))}}
  async function importAsset(path:string){try{await api.importAsset(path);importPicker=false;await refresh();notify('success',$dictionary.saved)}catch(e){notify('error',e instanceof Error?e.message:String(e))}}
  const formatKeys:FormatKey[]=['source','portrait916','landscape169','square11','portrait45','custom'];
  const formatLabel=(k:FormatKey)=>k==='source'?$dictionary.sourceFormat:k==='portrait916'?'9:16':k==='landscape169'?'16:9':k==='square11'?'1:1':k==='portrait45'?'4:5':$dictionary.custom;
</script>
<div class="page">
  <div class="page-head"><div><h1 class="page-title">{$dictionary.brands}</h1><p class="page-kicker">{$dictionary.brandDefaults}</p></div><div class="page-actions"><button class="btn" on:click={create}>＋ {$dictionary.newBrand}</button><button class="btn primary" on:click={save} disabled={!draft.name.trim()}>{$dictionary.save}</button></div></div>
  <div class="split-editor">
    <div class="list-pane">{#if !brands.length}<div class="empty">{$dictionary.noBrands}</div>{/if}{#each brands as b}<button class="list-item" class:active={selected===b.id} on:click={()=>selected=b.id}><strong>{b.name}</strong><span>{b.presetIds.length} {$dictionary.presets}</span></button>{/each}</div>
    <div class="stack">
      <section class="card"><div class="card-header"><strong>{draft.name||$dictionary.newBrand}</strong>{#if draft.id}<button class="btn danger" on:click={remove}>{$dictionary.delete}</button>{/if}</div><div class="card-body stack"><div class="field"><label for="brands-field-1">{$dictionary.brandName}</label><input id="brands-field-1" class="input" bind:value={draft.name}/></div><div class="field"><label for="brands-field-2">{$dictionary.description}</label><textarea id="brands-field-2" class="textarea" bind:value={draft.description}></textarea></div></div></section>
      <div class="grid two">
        <section class="card"><div class="card-header"><strong>{$dictionary.assets}</strong><div class="row"><button class="btn" on:click={()=>assetInput?.click()}>{$dictionary.uploadAsset}</button><button class="btn" on:click={()=>importPicker=true}>{$dictionary.importAsset}</button></div></div><div class="card-body stack">
          <input hidden bind:this={assetInput} type="file" on:change={(e)=>{const f=(e.currentTarget as HTMLInputElement).files?.[0];if(f)uploadAsset(f);(e.currentTarget as HTMLInputElement).value=''}}/>
          <div class="field"><label for="brands-field-3">{$dictionary.logo}</label><select id="brands-field-3" class="select" bind:value={draft.assets.logo}><option value="">{$dictionary.none}</option>{#each assets.filter(a=>a.mime.startsWith('image/')) as a}<option value={a.id}>{a.name}</option>{/each}</select></div>
          {#if logoAsset}<div class="preview-shell" style="min-height:130px"><img src={assetUrl(logoAsset.id)} alt={logoAsset.name} style="max-width:180px;max-height:110px;object-fit:contain"/></div>{/if}
          <div class="field"><label for="brands-field-4">{$dictionary.defaultOutro}</label><select id="brands-field-4" class="select" bind:value={draft.assets.defaultOutro}><option value="">{$dictionary.noOutro}</option>{#each assets as a}<option value={a.id}>{a.name}</option>{/each}</select></div>
          {#if outroAsset}<div class="help">{outroAsset.name} · {Math.max(1,Math.round(outroAsset.size/1024/1024))} MB</div>{/if}
        </div></section>
        <section class="card"><div class="card-header"><strong>{$dictionary.brandDefaults}</strong></div><div class="card-body stack">
          {#each formatKeys as key}<div class="field"><label for={`brand-default-${key}`}>{formatLabel(key)}</label><select id={`brand-default-${key}`} class="select" value={draft.defaultPresetByFormat[key]||''} on:change={(e)=>setDefault(key,(e.currentTarget as HTMLSelectElement).value)}><option value="">{$dictionary.none}</option>{#each presets.filter(p=>!p.brandId||!draft.id||p.brandId===draft.id) as p}<option value={p.id}>{p.name}</option>{/each}</select></div>{/each}
        </div></section>
      </div>
      <section class="card"><div class="card-header"><strong>{$dictionary.assetLibrary}</strong><span class="muted small">{assets.length}</span></div><div class="card-body"><div class="library-grid">{#if !assets.length}<div class="empty">{$dictionary.noAssets}</div>{/if}{#each assets as a}<div class="resource-card"><h3>{a.name}</h3><p>{a.mime||'application/octet-stream'} · {Math.max(1,Math.round(a.size/1024))} KB</p><div class="row between"><span class="chip mono">{a.id.slice(0,8)}</span><button class="btn danger" on:click={async()=>{if(confirm($dictionary.confirmDelete)){await api.deleteAsset(a.id);await refresh()}}}>{$dictionary.delete}</button></div></div>{/each}</div></div></section>
    </div>
  </div>
</div>
<PathPicker open={importPicker} mode="file" extensions="" title={$dictionary.importAsset} onselect={importAsset} onclose={()=>importPicker=false}/>
