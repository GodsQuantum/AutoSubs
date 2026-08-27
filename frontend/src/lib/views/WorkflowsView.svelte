<script lang="ts">
  import { api } from '$lib/api';
  import { dictionary } from '$lib/i18n';
  import type { Brand, FormatKey, Preset, Workflow } from '$lib/types';
  import PathPicker from '$lib/components/PathPicker.svelte';
  export let workflows:Workflow[]=[]; export let brands:Brand[]=[]; export let presets:Preset[]=[];
  export let refresh:()=>Promise<void>=async()=>{}; export let notify:(type:'error'|'success'|'info',message:string)=>void=()=>{};
  let selected='';let draft:Workflow=makeWorkflow();let picker:''|'watch'|'output'|'archive'='';
  $: current=workflows.find(w=>w.id===selected);$: if(current&&draft.id!==current.id)draft=clone(current);
  const clone=(w:Workflow):Workflow=>JSON.parse(JSON.stringify(w));
  function makeWorkflow():Workflow{return{id:'',name:'',watchDir:'',outputDir:'',archiveDir:'',format:{key:'source',fit:'preserve'},enabled:false}}
  function create(){selected='';draft=makeWorkflow()}
  async function save(){try{const w=await api.saveWorkflow(draft);selected=w.id;draft=clone(w);await refresh();notify('success',$dictionary.saved)}catch(e){notify('error',e instanceof Error?e.message:String(e))}}
  async function remove(){if(!draft.id||!confirm($dictionary.confirmDelete))return;try{await api.deleteWorkflow(draft.id);selected='';draft=makeWorkflow();await refresh()}catch(e){notify('error',e instanceof Error?e.message:String(e))}}
  function setPath(path:string){if(picker==='watch')draft.watchDir=path;if(picker==='output')draft.outputDir=path;if(picker==='archive')draft.archiveDir=path;picker='';draft={...draft}}
</script>
<div class="page">
  <div class="page-head"><div><h1 class="page-title">{$dictionary.workflows}</h1><p class="page-kicker">{$dictionary.workflowHint}</p></div><div class="page-actions"><button class="btn" on:click={create}>＋ {$dictionary.newWorkflow}</button><button class="btn primary" on:click={save} disabled={!draft.name.trim()}>{$dictionary.save}</button></div></div>
  <div class="split-editor">
    <div class="list-pane">{#if !workflows.length}<div class="empty">{$dictionary.noWorkflows}</div>{/if}{#each workflows as w}<button class="list-item" class:active={selected===w.id} on:click={()=>selected=w.id}><strong>{w.name}</strong><span>{w.enabled?$dictionary.enabled:$dictionary.disabled} · {w.watchDir}</span></button>{/each}</div>
    <div class="stack">
      <section class="card"><div class="card-header"><strong>{draft.name||$dictionary.newWorkflow}</strong>{#if draft.id}<button class="btn danger" on:click={remove}>{$dictionary.delete}</button>{/if}</div><div class="card-body stack">
        <div class="row between"><div class="field" style="flex:1"><label>{$dictionary.workflowName}</label><input class="input" bind:value={draft.name}/></div><label class="check" style="padding-top:20px"><input type="checkbox" bind:checked={draft.enabled}/>{draft.enabled?$dictionary.enabled:$dictionary.disabled}</label></div>
        <div class="field"><label>{$dictionary.watchDir}</label><div class="row"><input class="input mono" bind:value={draft.watchDir}/><button class="btn" on:click={()=>picker='watch'}>{$dictionary.browse}</button></div></div>
        <div class="field"><label>{$dictionary.outputDir}</label><div class="row"><input class="input mono" bind:value={draft.outputDir}/><button class="btn" on:click={()=>picker='output'}>{$dictionary.browse}</button></div></div>
        <div class="field"><label>{$dictionary.archiveDir}</label><div class="row"><input class="input mono" bind:value={draft.archiveDir}/><button class="btn" on:click={()=>picker='archive'}>{$dictionary.browse}</button></div></div>
      </div></section>
      <section class="card"><div class="card-header"><strong>{$dictionary.outputSection}</strong></div><div class="card-body grid two">
        <div class="field"><label>{$dictionary.brand}</label><select class="select" bind:value={draft.brandId}><option value="">{$dictionary.noBrand}</option>{#each brands as b}<option value={b.id}>{b.name}</option>{/each}</select></div>
        <div class="field"><label>{$dictionary.presetOverride}</label><select class="select" bind:value={draft.presetId}><option value="">{$dictionary.brandDefault}</option>{#each presets as p}<option value={p.id}>{p.name}</option>{/each}</select></div>
        <div class="field"><label>{$dictionary.format}</label><select class="select" bind:value={draft.format.key}><option value="source">{$dictionary.sourceFormat}</option><option value="portrait916">9:16</option><option value="landscape169">16:9</option><option value="square11">1:1</option><option value="portrait45">4:5</option><option value="custom">{$dictionary.custom}</option></select></div>
        <div class="field"><label>{$dictionary.fit}</label><select class="select" bind:value={draft.format.fit} disabled={draft.format.key==='source'}><option value="preserve">{$dictionary.preserve}</option><option value="contain">{$dictionary.contain}</option><option value="cover">{$dictionary.cover}</option><option value="stretch">{$dictionary.stretch}</option></select></div>
        {#if draft.format.key==='custom'}<div class="field"><label>{$dictionary.width}</label><input class="input" type="number" bind:value={draft.format.width}/></div><div class="field"><label>{$dictionary.height}</label><input class="input" type="number" bind:value={draft.format.height}/></div>{/if}
      </div></section>
      <section class="card"><div class="card-body"><div class="help">{$dictionary.workflowHint}</div></div></section>
    </div>
  </div>
</div>
<PathPicker open={picker!==''} mode="directory" initialPath={picker==='watch'?draft.watchDir:picker==='output'?draft.outputDir:draft.archiveDir} title={picker==='watch'?$dictionary.watchDir:picker==='output'?$dictionary.outputDir:$dictionary.archiveDir} onselect={setPath} onclose={()=>picker=''}/>
