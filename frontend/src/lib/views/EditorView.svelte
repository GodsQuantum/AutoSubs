<script lang="ts">
  import { api, subtitleExportUrl, videoUrl } from '$lib/api';
  import { dictionary } from '$lib/i18n';
  import type { FormatKey, FitMode, Job, Preset, SubtitleLine } from '$lib/types';
  import StatusPill from '$lib/components/StatusPill.svelte';
  import PathPicker from '$lib/components/PathPicker.svelte';

  export let job: Job | undefined;
  export let presets: Preset[] = [];
  export let refresh: () => Promise<void> = async()=>{};
  export let notify: (type:'error'|'success'|'info', message:string)=>void = ()=>{};

  let loadedId='';
  let lines:SubtitleLine[]=[];
  let dirty=false;
  let saving=false;
  let currentTime=0;
  let search='';
  let replace='';
  let shiftMs=0;
  let maxChars=25;
  let maxLines=2;
  let selectedPreset='';
  let formatKey:FormatKey='source';
  let fit:FitMode='preserve';
  let customWidth=1080;
  let customHeight=1920;
  let sidecarPicker=false;
  let fileInput:HTMLInputElement;
  let report:{repairedLineOverlaps:number;retimedWordLines:number;droppedEmptyLines:number}|undefined;

  $: if(job && job.id!==loadedId){ loadedId=job.id; hydrate(job); }
  $: locked = job ? ['pending','uploading','probing','transcribing','correcting','rendering'].includes(job.status) : true;
  $: activeLine = lines.findIndex(l=>currentTime>=l.start&&currentTime<l.end);
  $: currentPreset = presets.find(p=>p.id===selectedPreset);

  function hydrate(j:Job){
    lines=(j.lines??[]).map(l=>({...l,words:l.words?.map(w=>({...w}))})); dirty=false; report=undefined;
    selectedPreset=j.presetId??''; formatKey=j.format?.key??'source'; fit=j.format?.fit??'preserve'; customWidth=j.format?.width??1080; customHeight=j.format?.height??1920;
    const p=presets.find(p=>p.id===selectedPreset); if(p){maxChars=p.maxChars;maxLines=p.maxLines;}
  }
  function mark(){dirty=true;}
  async function save(){ if(!job)return; saving=true; try{const r=await api.saveSubtitles(job.id,lines);lines=r.lines;report=r;dirty=false;await refresh();notify('success',$dictionary.saved)}catch(e){notify('error',e instanceof Error?e.message:String(e))}finally{saving=false} }
  async function regroup(){if(!job)return;try{lines=await api.regroup(job.id,maxChars,maxLines);dirty=false;await refresh()}catch(e){notify('error',e instanceof Error?e.message:String(e))}}
  function shiftAll(){const d=Number(shiftMs||0)/1000;lines=lines.map(l=>({...l,start:Math.max(0,l.start+d),end:Math.max(.02,l.end+d),words:l.words?.map(w=>({...w,start:Math.max(0,w.start+d),end:Math.max(.02,w.end+d)}))}));dirty=true}
  function replaceText(){if(!search)return; lines=lines.map(l=>({...l,text:l.text.split(search).join(replace)}));dirty=true}
  async function applyJob(){if(!job)return;try{await api.updateJob(job.id,{presetId:selectedPreset||null,format:{key:formatKey,fit:formatKey==='source'?'preserve':fit,width:formatKey==='custom'?Number(customWidth):undefined,height:formatKey==='custom'?Number(customHeight):undefined}});await refresh();notify('success',$dictionary.saved)}catch(e){notify('error',e instanceof Error?e.message:String(e))}}
  async function render(){if(!job)return;try{if(dirty)await save();await applyJob();await api.render(job.id);await refresh()}catch(e){notify('error',e instanceof Error?e.message:String(e))}}
  async function removeSidecar(){if(!job)return;try{await api.removeSidecar(job.id);await refresh();notify('success',$dictionary.prepared)}catch(e){notify('error',e instanceof Error?e.message:String(e))}}
  async function setServerSidecar(path:string){if(!job)return;try{await api.setSidecar(job.id,path);sidecarPicker=false;await refresh()}catch(e){notify('error',e instanceof Error?e.message:String(e))}}
  async function uploadSidecar(file:File){if(!job)return;try{await api.uploadSidecar(job.id,file);await refresh()}catch(e){notify('error',e instanceof Error?e.message:String(e))}}
  function jump(line:SubtitleLine){const v=document.querySelector<HTMLVideoElement>('#autosubs-editor-video');if(v){v.currentTime=line.start;v.play().catch(()=>{});}}
</script>

<div class="page">
  <div class="page-head">
    <div><h1 class="page-title">{$dictionary.editor}</h1><p class="page-kicker">{job ? job.originalName : $dictionary.noSelectedJob}</p></div>
    {#if job}<div class="page-actions"><StatusPill status={job.status}/><button class="btn" disabled={!dirty||locked||saving} on:click={save}>{saving?$dictionary.saving:$dictionary.save}</button><button class="btn primary" disabled={locked||!lines.length} on:click={render}>▶ {$dictionary.renderVideo}</button></div>{/if}
  </div>

  {#if !job}
    <div class="empty">{$dictionary.noSelectedJob}</div>
  {:else}
    <div class="editor-layout">
      <div class="editor-left">
        <div class="video-stage">
          <video id="autosubs-editor-video" src={videoUrl(job.id)} controls preload="metadata" playsinline on:timeupdate={(e)=>currentTime=(e.currentTarget as HTMLVideoElement).currentTime}></video>
        </div>
        <section class="card">
          <div class="card-header"><strong>{$dictionary.subtitles}</strong><span class="muted small">{lines.length} · {currentTime.toFixed(2)}s</span></div>
          <div class="card-body">
            <div class="subtitle-toolbar">
              <div class="field" style="flex:1"><label>{$dictionary.search}</label><input class="input" bind:value={search}/></div>
              <div class="field" style="flex:1"><label>{$dictionary.replace}</label><input class="input" bind:value={replace}/></div>
              <button class="btn" disabled={locked||!search} on:click={replaceText}>{$dictionary.replaceAll}</button>
              <div class="field" style="width:105px"><label>{$dictionary.shift}</label><input class="input" type="number" bind:value={shiftMs}/></div>
              <button class="btn" disabled={locked} on:click={shiftAll}>± {$dictionary.milliseconds}</button>
            </div>
            {#if locked}<div class="help" style="margin-top:9px;color:var(--warning)">{$dictionary.activeJobLocked}</div>{/if}
            <div class="subtitle-list">
              {#each lines as line, i (line.id)}
                <div class="subtitle-row" class:active={i===activeLine}>
                  <input class="input mono" type="number" step="0.01" bind:value={line.start} disabled={locked} on:input={mark}/>
                  <input class="input mono" type="number" step="0.01" bind:value={line.end} disabled={locked} on:input={mark}/>
                  <textarea class="textarea" bind:value={line.text} disabled={locked} on:input={mark}></textarea>
                  <button class="btn icon ghost" on:click={()=>jump(line)} title={$dictionary.jumpToLine}>▶</button>
                </div>
              {/each}
            </div>
          </div>
        </section>
      </div>

      <aside class="editor-panel stack">
        <section class="card">
          <div class="card-header"><strong>{$dictionary.outputSection}</strong></div>
          <div class="card-body stack">
            <div class="field"><label>{$dictionary.preset}</label><select class="select" bind:value={selectedPreset} disabled={locked}><option value="">{$dictionary.none}</option>{#each presets as p}<option value={p.id}>{p.name}</option>{/each}</select></div>
            <div class="grid two">
              <div class="field"><label>{$dictionary.format}</label><select class="select" bind:value={formatKey} disabled={locked}><option value="source">{$dictionary.sourceFormat}</option><option value="portrait916">9:16</option><option value="landscape169">16:9</option><option value="square11">1:1</option><option value="portrait45">4:5</option><option value="custom">{$dictionary.custom}</option></select></div>
              <div class="field"><label>{$dictionary.fit}</label><select class="select" bind:value={fit} disabled={locked||formatKey==='source'}><option value="contain">{$dictionary.contain}</option><option value="cover">{$dictionary.cover}</option><option value="stretch">{$dictionary.stretch}</option></select></div>
            </div>
            {#if formatKey==='custom'}<div class="grid two"><div class="field"><label>{$dictionary.width}</label><input class="input" type="number" bind:value={customWidth}/></div><div class="field"><label>{$dictionary.height}</label><input class="input" type="number" bind:value={customHeight}/></div></div>{/if}
            <div class="help">{$dictionary.sourcePreserveHint}</div>
            <button class="btn" disabled={locked} on:click={applyJob}>{$dictionary.applyToJob}</button>
          </div>
        </section>

        <section class="card">
          <div class="card-header"><strong>{$dictionary.segmentation}</strong></div>
          <div class="card-body stack">
            <div class="grid two"><div class="field"><label>{$dictionary.maxChars}</label><input class="input" type="number" min="5" bind:value={maxChars}/></div><div class="field"><label>{$dictionary.maxLines}</label><input class="input" type="number" min="1" max="4" bind:value={maxLines}/></div></div>
            <button class="btn" disabled={locked||!lines.length} on:click={regroup}>✦ {$dictionary.regroup}</button>
            <div class="help">{$dictionary.timingRepairHint}</div>
            {#if report}<div class="resource-meta"><span class="chip">{$dictionary.repairedOverlaps}: {report.repairedLineOverlaps}</span><span class="chip">{$dictionary.retimedLines}: {report.retimedWordLines}</span><span class="chip">{$dictionary.droppedEmpty}: {report.droppedEmptyLines}</span></div>{/if}
          </div>
        </section>

        <section class="card">
          <div class="card-header"><strong>{$dictionary.attachedSidecar}</strong></div>
          <div class="card-body stack">
            <div class="mono small muted" style="overflow-wrap:anywhere">{job.attachedSidecar || $dictionary.none}</div>
            <div class="row wrap"><button class="btn" disabled={locked} on:click={()=>fileInput?.click()}>{$dictionary.attachFile}</button><button class="btn" disabled={locked} on:click={()=>sidecarPicker=true}>{$dictionary.chooseServerFile}</button>{#if job.attachedSidecar}<button class="btn danger" disabled={locked} on:click={removeSidecar}>{$dictionary.removeSidecar}</button>{/if}</div>
            <input bind:this={fileInput} hidden type="file" accept=".srt,.ass,.json" on:change={(e)=>{const f=(e.currentTarget as HTMLInputElement).files?.[0];if(f)uploadSidecar(f);(e.currentTarget as HTMLInputElement).value=''}}/>
          </div>
        </section>

        <section class="card">
          <div class="card-header"><strong>{$dictionary.export}</strong></div>
          <div class="card-body row wrap"><a class="btn" href={subtitleExportUrl(job.id,'srt')} download>↓ SRT</a><a class="btn" href={subtitleExportUrl(job.id,'ass')} download>↓ ASS</a><a class="btn" href={subtitleExportUrl(job.id,'json')} download>↓ JSON</a></div>
        </section>
      </aside>
    </div>
  {/if}
</div>

<PathPicker open={sidecarPicker} mode="file" initialPath="" extensions="srt,ass,json" title={$dictionary.attachedSidecar} onselect={setServerSidecar} onclose={()=>sidecarPicker=false}/>
