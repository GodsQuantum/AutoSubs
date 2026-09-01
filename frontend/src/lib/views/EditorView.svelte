<script lang="ts">
  import { api, subtitleExportUrl, videoUrl } from '$lib/api';
  import { dictionary } from '$lib/i18n';
  import { subtitlesToVtt } from '$lib/captions.js';
  import { splitSubtitleLine, mergeSubtitleLines, deleteSubtitleLine } from '$lib/subtitle-edit.js';
  import type { FormatKey, FitMode, FormatProfile, Job, Preset, SubtitleLine } from '$lib/types';
  import StatusPill from '$lib/components/StatusPill.svelte';
  import PathPicker from '$lib/components/PathPicker.svelte';
  import FormatPreview from '$lib/components/FormatPreview.svelte';

  export let job: Job | undefined;
  export let presets: Preset[] = [];
  export let refresh: () => Promise<void> = async()=>{};
  export let notify: (type:'error'|'success'|'info', message:string)=>void = ()=>{};

  type SafeZoneKey = 'off'|'generic'|'tiktok'|'reels'|'shorts';
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
  let previousFormatKey:FormatKey=formatKey;
  let customWidth=1080;
  let customHeight=1920;
  let safeZone:SafeZoneKey='off';
  let captionTrackUrl='';
  let sidecarPicker=false;
  let fileInput:HTMLInputElement;
  let textareas:HTMLTextAreaElement[]=[];
  let report:{repairedLineOverlaps:number;retimedWordLines:number;droppedEmptyLines:number}|undefined;
  let previewFormat:FormatProfile={key:'source',fit:'preserve'};

  $: if(job && job.id!==loadedId){ loadedId=job.id; hydrate(job); }
  $: locked = job ? ['pending','uploading','probing','transcribing','correcting','rendering'].includes(job.status) : true;
  $: activeLine = lines.findIndex(l=>currentTime>=l.start&&currentTime<l.end);
  $: activeText = activeLine>=0 ? lines[activeLine]?.text ?? '' : '';
  $: currentPreset = presets.find(p=>p.id===selectedPreset);
  $: captionTrackUrl = `data:text/vtt;charset=utf-8,${encodeURIComponent(subtitlesToVtt(lines))}`;
  $: if(formatKey!==previousFormatKey){previousFormatKey=formatKey;if(formatKey==='source')fit='preserve';else if(fit==='preserve')fit='cover';}
  $: previewFormat={key:formatKey,fit:formatKey==='source'?'preserve':fit,width:formatKey==='custom'?Number(customWidth):undefined,height:formatKey==='custom'?Number(customHeight):undefined};

  function hydrate(j:Job){
    lines=(j.lines??[]).map(l=>({...l,words:l.words?.map(w=>({...w}))})); dirty=false; report=undefined;
    selectedPreset=j.presetId??''; formatKey=j.format?.key??'source'; fit=j.format?.fit??'preserve'; previousFormatKey=formatKey; customWidth=j.format?.width??1080; customHeight=j.format?.height??1920;
    const p=presets.find(p=>p.id===selectedPreset); if(p){maxChars=p.maxChars;maxLines=p.maxLines;}
  }
  function mark(){dirty=true;}
  async function save():Promise<boolean>{ if(!job)return false; saving=true; try{const r=await api.saveSubtitles(job.id,lines);lines=r.lines;report=r;dirty=false;await refresh();notify('success',$dictionary.saved);return true}catch(e){notify('error',e instanceof Error?e.message:String(e));return false}finally{saving=false} }
  async function regroup(){if(!job)return;try{lines=await api.regroup(job.id,maxChars,maxLines);dirty=false;await refresh()}catch(e){notify('error',e instanceof Error?e.message:String(e))}}
  function shiftAll(){const d=Number(shiftMs||0)/1000;lines=lines.map(l=>({...l,start:Math.max(0,l.start+d),end:Math.max(.02,l.end+d),words:l.words?.map(w=>({...w,start:Math.max(0,w.start+d),end:Math.max(.02,w.end+d)}))}));dirty=true}
  function replaceText(){if(!search)return; lines=lines.map(l=>({...l,text:l.text.split(search).join(replace)}));dirty=true}
  function split(i:number){const textarea=textareas[i];lines=splitSubtitleLine(lines,i,textarea?.selectionStart??Math.floor(lines[i].text.length/2));dirty=true}
  function merge(i:number){lines=mergeSubtitleLines(lines,i);dirty=true}
  function remove(i:number){lines=deleteSubtitleLine(lines,i);dirty=true}
  async function applyJob():Promise<boolean>{
    if(!job)return false;
    if(formatKey==='custom' && (!Number.isInteger(Number(customWidth))||!Number.isInteger(Number(customHeight))||Number(customWidth)<16||Number(customHeight)<16||Number(customWidth)>16384||Number(customHeight)>16384||Number(customWidth)%2!==0||Number(customHeight)%2!==0)){notify('error',`${$dictionary.custom}: ${$dictionary.width} × ${$dictionary.height}`);return false;}
    try{await api.updateJob(job.id,{presetId:selectedPreset||null,format:previewFormat});await refresh();notify('success',$dictionary.saved);return true}catch(e){notify('error',e instanceof Error?e.message:String(e));return false}
  }
  async function render(){if(!job)return;try{if(dirty && !(await save()))return;if(!(await applyJob()))return;await api.render(job.id);await refresh()}catch(e){notify('error',e instanceof Error?e.message:String(e))}}
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
        <div class="video-stage" style="padding:14px">
          <FormatPreview format={previewFormat} preset={currentPreset} text={activeText} videoSrc={videoUrl(job.id)} controls={true} videoId="autosubs-editor-video" captionsSrc={captionTrackUrl} {safeZone} onVideoTimeUpdate={(time)=>currentTime=time}/>
        </div>
        <div class="row between wrap">
          <div class="resource-meta"><span class="chip">{formatKey==='source'?$dictionary.sourceFormat:formatKey==='portrait916'?'9:16':formatKey==='landscape169'?'16:9':formatKey==='square11'?'1:1':formatKey==='portrait45'?'4:5':`${customWidth}×${customHeight}`}</span><span class="chip">{formatKey==='source'?$dictionary.preserve:fit}</span></div>
          <div class="segmented"><button class:active={safeZone==='off'} on:click={()=>safeZone='off'}>{$dictionary.none}</button><button class:active={safeZone==='generic'} on:click={()=>safeZone='generic'}>Generic</button><button class:active={safeZone==='tiktok'} on:click={()=>safeZone='tiktok'}>TikTok</button><button class:active={safeZone==='reels'} on:click={()=>safeZone='reels'}>Reels</button><button class:active={safeZone==='shorts'} on:click={()=>safeZone='shorts'}>Shorts</button></div>
        </div>
        <section class="card">
          <div class="card-header"><strong>{$dictionary.subtitles}</strong><span class="muted small">{lines.length} · {currentTime.toFixed(2)}s</span></div>
          <div class="card-body">
            <div class="subtitle-toolbar">
              <div class="field" style="flex:1"><label for="editor-field-1">{$dictionary.search}</label><input id="editor-field-1" class="input" bind:value={search}/></div>
              <div class="field" style="flex:1"><label for="editor-field-2">{$dictionary.replace}</label><input id="editor-field-2" class="input" bind:value={replace}/></div>
              <button class="btn" disabled={locked||!search} on:click={replaceText}>{$dictionary.replaceAll}</button>
              <div class="field" style="width:105px"><label for="editor-field-3">{$dictionary.shift}</label><input id="editor-field-3" class="input" type="number" bind:value={shiftMs}/></div>
              <button class="btn" disabled={locked} on:click={shiftAll}>± {$dictionary.milliseconds}</button>
            </div>
            {#if locked}<div class="help" style="margin-top:9px;color:var(--warning)">{$dictionary.activeJobLocked}</div>{/if}
            <div class="subtitle-list">
              {#each lines as line, i (line.id)}
                <div class="subtitle-row" class:active={i===activeLine}>
                  <input class="input mono" type="number" step="0.01" bind:value={line.start} disabled={locked} on:input={mark}/>
                  <input class="input mono" type="number" step="0.01" bind:value={line.end} disabled={locked} on:input={mark}/>
                  <textarea class="textarea" bind:this={textareas[i]} bind:value={line.text} disabled={locked} on:input={mark}></textarea>
                  <button class="btn icon ghost" on:click={()=>jump(line)} title={$dictionary.jumpToLine}>▶</button>
                  <div class="row wrap" style="grid-column:2/-1"><button class="btn" disabled={locked} on:click={()=>split(i)}>{$dictionary.split}</button><button class="btn" disabled={locked||i===0} on:click={()=>merge(i-1)}>{$dictionary.mergePrevious}</button><button class="btn" disabled={locked||i===lines.length-1} on:click={()=>merge(i)}>{$dictionary.mergeNext}</button><button class="btn danger" disabled={locked} on:click={()=>remove(i)}>{$dictionary.delete}</button></div>
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
            <div class="field"><label for="editor-field-4">{$dictionary.preset}</label><select id="editor-field-4" class="select" bind:value={selectedPreset} disabled={locked}><option value="">{$dictionary.none}</option>{#each presets as p}<option value={p.id}>{p.name}</option>{/each}</select></div>
            <div class="grid two">
              <div class="field"><label for="editor-field-5">{$dictionary.format}</label><select id="editor-field-5" class="select" bind:value={formatKey} disabled={locked}><option value="source">{$dictionary.sourceFormat}</option><option value="portrait916">9:16</option><option value="landscape169">16:9</option><option value="square11">1:1</option><option value="portrait45">4:5</option><option value="custom">{$dictionary.custom}</option></select></div>
              <div class="field"><label for="editor-field-6">{$dictionary.fit}</label><select id="editor-field-6" class="select" bind:value={fit} disabled={locked||formatKey==='source'}><option value="contain">{$dictionary.contain}</option><option value="cover">{$dictionary.cover}</option><option value="stretch">{$dictionary.stretch}</option></select></div>
            </div>
            {#if formatKey==='custom'}<div class="grid two"><div class="field"><label for="editor-field-7">{$dictionary.width}</label><input id="editor-field-7" class="input" type="number" min="16" max="16384" step="2" bind:value={customWidth}/></div><div class="field"><label for="editor-field-8">{$dictionary.height}</label><input id="editor-field-8" class="input" type="number" min="16" max="16384" step="2" bind:value={customHeight}/></div></div>{/if}
            <div class="help">{$dictionary.sourcePreserveHint}</div>
            <button class="btn" disabled={locked} on:click={applyJob}>{$dictionary.applyToJob}</button>
          </div>
        </section>

        <section class="card">
          <div class="card-header"><strong>{$dictionary.segmentation}</strong></div>
          <div class="card-body stack">
            <div class="grid two"><div class="field"><label for="editor-field-9">{$dictionary.maxChars}</label><input id="editor-field-9" class="input" type="number" min="5" bind:value={maxChars}/></div><div class="field"><label for="editor-field-10">{$dictionary.maxLines}</label><input id="editor-field-10" class="input" type="number" min="1" max="4" bind:value={maxLines}/></div></div>
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
            <input bind:this={fileInput} hidden type="file" accept=".srt,.ass,.ssa,.json" on:change={(e)=>{const f=(e.currentTarget as HTMLInputElement).files?.[0];if(f)uploadSidecar(f);(e.currentTarget as HTMLInputElement).value=''}}/>
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

<PathPicker open={sidecarPicker} mode="file" initialPath="" extensions="srt,ass,ssa,json" title={$dictionary.attachedSidecar} onselect={setServerSidecar} onclose={()=>sidecarPicker=false}/>
