<script lang="ts">
  import { api, tusUpload } from '$lib/api';
  import { dictionary } from '$lib/i18n';
  import type { Job, Preset } from '$lib/types';
  import StatusPill from '$lib/components/StatusPill.svelte';
  import PathPicker from '$lib/components/PathPicker.svelte';
  import { pairFiles } from '$lib/pairing.js';

  export let jobs: Job[] = [];
  export let presets: Preset[] = [];
  export let refresh: () => Promise<void> = async () => {};
  export let openEditor: (id:string) => void = () => {};
  export let notify: (type:'error'|'success'|'info', message:string) => void = () => {};

  let fileInput: HTMLInputElement;
  let drag = false;
  let uploads: {name:string; progress:number; state:string; controller?:AbortController}[] = [];
  let serverPicker = false;
  let serverVideo = '';
  let serverSidecar = '';
  let serverSidecarPicker = false;
  let serverPreset = '';

  $: activeCount = jobs.filter(j=>['pending','uploading','probing','transcribing','correcting','rendering'].includes(j.status)).length;
  $: readyCount = jobs.filter(j=>j.status==='ready').length;
  $: doneCount = jobs.filter(j=>j.status==='done').length;
  $: errorCount = jobs.filter(j=>j.status==='error').length;

  async function ingest(files: FileList | File[]) {
    const pairs = pairFiles(Array.from(files) as any) as {video:File;sidecar?:File}[];
    for (const pair of pairs) {
      const item = { name: pair.video.name, progress: 0, state: $dictionary.uploading, controller: new AbortController() };
      uploads = [...uploads, item];
      try {
        const jobId = await tusUpload(pair.video, p => { item.progress = p; uploads = [...uploads]; }, item.controller.signal);
        if (pair.sidecar) await api.uploadSidecar(jobId, pair.sidecar);
        else await api.prepare(jobId);
        item.state = pair.sidecar ? $dictionary.paired : $dictionary.videoOnly;
        item.progress = 100; uploads = [...uploads];
        await refresh();
      } catch (e) {
        item.state = e instanceof DOMException && e.name==='AbortError' ? $dictionary.cancelled : $dictionary.failed;
        uploads = [...uploads]; notify('error', e instanceof Error ? e.message : String(e));
      }
    }
  }
  async function fromServer() {
    if (!serverVideo) return;
    try {
      await api.createFromPath(serverVideo, serverSidecar || undefined, serverPreset || undefined);
      serverVideo='';serverSidecar=''; await refresh(); notify('success',$dictionary.prepared);
    } catch(e){notify('error',e instanceof Error?e.message:String(e));}
  }
  async function action(job:Job, kind:'prepare'|'render'|'cancel') {
    try { if(kind==='prepare')await api.prepare(job.id); else if(kind==='render')await api.render(job.id); else await api.cancel(job.id); await refresh(); }
    catch(e){notify('error',e instanceof Error?e.message:String(e));}
  }
  const active = (job:Job) => ['pending','uploading','probing','transcribing','correcting','rendering'].includes(job.status);
</script>

<div class="page">
  <div class="page-head">
    <div><h1 class="page-title">{$dictionary.queue}</h1><p class="page-kicker">{$dictionary.queueReadyHint}</p></div>
    <div class="page-actions"><button class="btn" on:click={()=>serverPicker=true}>▰ {$dictionary.addServerVideo}</button><button class="btn primary" on:click={()=>fileInput?.click()}>＋ {$dictionary.addFiles}</button></div>
  </div>

  <input bind:this={fileInput} hidden type="file" multiple on:change={(e)=>{ const files=(e.currentTarget as HTMLInputElement).files; if(files)ingest(files); (e.currentTarget as HTMLInputElement).value=''; }} />

  <div class="kpi-row">
    <div class="kpi"><strong>{activeCount}</strong><span>{$dictionary.progress}</span></div>
    <div class="kpi"><strong>{readyCount}</strong><span>{$dictionary.ready}</span></div>
    <div class="kpi"><strong>{doneCount}</strong><span>{$dictionary.done}</span></div>
    <div class="kpi"><strong>{errorCount}</strong><span>{$dictionary.error}</span></div>
  </div>

  <div class="dropzone" class:dragging={drag} role="button" tabindex="0" on:click={()=>fileInput?.click()} on:keydown={(e)=>{if(e.key==='Enter'||e.key===' ')fileInput?.click()}} on:dragover={(e)=>{e.preventDefault();drag=true}} on:dragleave={()=>drag=false} on:drop={(e)=>{e.preventDefault();drag=false;if(e.dataTransfer?.files)ingest(e.dataTransfer.files)}}>
    <div><div class="drop-icon">CC</div><strong>{$dictionary.dropFiles}</strong><div class="muted small" style="margin-top:7px">{$dictionary.dropHint}</div></div>
  </div>

  {#if uploads.length}
    <div class="upload-list">
      {#each uploads as item}
        <div class="upload-row"><div class="job-name"><strong>{item.name}</strong><span>{item.state}</span></div><div><div class="progress" style={`--progress:${item.progress}%`}><span></span></div></div>{#if item.progress<100}<button class="btn icon ghost" on:click={()=>item.controller?.abort()} aria-label={$dictionary.cancel}>×</button>{:else}<span>✓</span>{/if}</div>
      {/each}
    </div>
  {/if}

  <div style="height:14px"></div>
  {#if jobs.length===0}
    <div class="empty"><div><strong>{$dictionary.noJobs}</strong><div class="small" style="margin-top:6px">{$dictionary.noJobsHint}</div></div></div>
  {:else}
    <div class="job-list">
      {#each jobs as job (job.id)}
        <article class="job-row">
          <div class="job-name"><strong>{job.originalName}</strong><span class="mono">{job.inputPath || job.outputPath || job.id}</span></div>
          <div><StatusPill status={job.status}/>{#if job.progress!==undefined}<div class="progress" style={`--progress:${job.progress}%;margin-top:7px`}><span></span></div>{/if}</div>
          <div class="job-output muted small">{job.outputPath || job.attachedSidecar || '—'}</div>
          <div class="job-actions">
            {#if job.status==='ready'||job.status==='done'||job.status==='error'||job.status==='interrupted'}<button class="btn" on:click={()=>openEditor(job.id)}>{$dictionary.edit}</button>{/if}
            {#if job.status==='ready'}<button class="btn primary" on:click={()=>action(job,'render')}>▶ {$dictionary.render}</button>{/if}
            {#if job.status==='error'||job.status==='cancelled'||job.status==='interrupted'}<button class="btn primary" on:click={()=>action(job,'prepare')}>↻ {$dictionary.retry}</button>{/if}
            {#if active(job)}<button class="btn danger" on:click={()=>action(job,'cancel')}>■ {$dictionary.cancel}</button>{/if}
          </div>
          {#if job.error}<div class="small" style="grid-column:1/-1;color:var(--danger)">{job.error}</div>{/if}
        </article>
      {/each}
    </div>
  {/if}
</div>

<PathPicker open={serverPicker} mode="file" initialPath={serverVideo} extensions="" title={$dictionary.addServerVideo} onselect={(path)=>serverVideo=path} onclose={()=>serverPicker=false}/>
<PathPicker open={serverSidecarPicker} mode="file" initialPath={serverSidecar} extensions="srt,ass,ssa,json" title={$dictionary.attachedSidecar} onselect={(path)=>serverSidecar=path} onclose={()=>serverSidecarPicker=false}/>

{#if serverVideo}
  <div class="modal-backdrop">
    <section class="modal" style="grid-template-rows:auto minmax(0,1fr) auto">
      <div class="modal-head"><strong>{$dictionary.addServerVideo}</strong><button class="btn icon ghost" on:click={()=>serverVideo=''}>×</button></div>
      <div class="card-body stack">
        <div class="field"><label for="queue-field-1">{$dictionary.source}</label><input id="queue-field-1" class="input mono" value={serverVideo} readonly /></div>
        <div class="field"><label for="queue-field-2">{$dictionary.attachedSidecar}</label><div class="row"><input id="queue-field-2" class="input mono" bind:value={serverSidecar} placeholder={$dictionary.none}/><button class="btn" on:click={()=>serverSidecarPicker=true}>{$dictionary.browse}</button></div></div>
        <div class="field"><label for="queue-field-3">{$dictionary.preset}</label><select id="queue-field-3" class="select" bind:value={serverPreset}><option value="">{$dictionary.none}</option>{#each presets as p}<option value={p.id}>{p.name}</option>{/each}</select></div>
        <div class="help">{$dictionary.serverFileHint}</div>
      </div>
      <div class="modal-foot"><button class="btn" on:click={()=>serverVideo=''}>{$dictionary.close}</button><button class="btn primary" on:click={fromServer}>{$dictionary.prepare}</button></div>
    </section>
  </div>
{/if}
