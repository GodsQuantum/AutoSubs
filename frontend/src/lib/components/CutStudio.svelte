<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { addRange, totalDuration, timeLabel, nextPlaybackTime, restoreRanges } from '$lib/cut-selection.js';

  export let jobId: string = '';
  export let currentTime = 0;
  export let locked = false;
  export let notify: (type:'error'|'success'|'info',message:string)=>void = ()=>{};
  const draftKey = () => `autosubs:cut:draft:${jobId}`;
  const taskKey = () => `autosubs:cut:task:${jobId}`;

  type Range={start:number;end:number};
  type CutTask={id:string;status:string;outputPath?:string;error?:string};
  let video:HTMLVideoElement|null=null;
  let duration=0, cutIn=0, cutOut=0, hasIn=false,hasOut=false;
  let keepRanges:Range[]=[];
  let cutTask:CutTask|undefined;
  let cutWorking=false, previewing=false;
  let cutPoll:ReturnType<typeof setInterval>|undefined;

  function seek(time:number){if(video)video.currentTime=Math.max(0,Math.min(duration,time));}
  function markIn(){cutIn=Number(currentTime.toFixed(3));hasIn=true;}
  function markOut(){cutOut=Number(currentTime.toFixed(3));hasOut=true;}
  function add(){
    try{
      if(!hasIn||!hasOut)throw new Error('Marquez un début (I) puis une fin (O).');
      keepRanges=addRange(keepRanges,Number(cutIn),Number(cutOut),duration);
      localStorage.setItem(draftKey(),JSON.stringify(keepRanges));
      hasIn=false;hasOut=false;cutIn=0;cutOut=0;
    }catch(e){notify('error',e instanceof Error?e.message:String(e))}
  }
  function remove(index:number){
    keepRanges=keepRanges.filter((_,i)=>i!==index);
    localStorage.setItem(draftKey(),JSON.stringify(keepRanges));
    if(previewing)stopPreview();
  }
  function stopPreview(){previewing=false;video?.pause();}
  function onPlayback(){
    if(!previewing||!video)return;
    const next=nextPlaybackTime(keepRanges,video.currentTime);
    if(next===null){stopPreview();return;}
    if(Math.abs(video.currentTime-next)>.035)seek(next);
  }
  function playKept(){
    if(!video||!keepRanges.length)return;
    if(previewing){stopPreview();return;}
    previewing=true;seek(keepRanges[0].start);
    video.play().catch(()=>{previewing=false;notify('error','Lecture impossible.');});
  }
  function shortcuts(event:KeyboardEvent){
    if(event.metaKey||event.ctrlKey||event.altKey||event.shiftKey||locked||cutWorking||!duration)return;
    const target=event.target as HTMLElement|null;
    if(target?.isContentEditable || ['INPUT','TEXTAREA','SELECT'].includes(target?.tagName||''))return;
    if(event.key.toLowerCase()==='i'){markIn();event.preventDefault();}
    if(event.key.toLowerCase()==='o'){markOut();event.preventDefault();}
  }
  async function checkTask(id:string){
    try{
      const status=await api.cutStatus(id);
      if(cutTask?.id!==id)return;
      cutTask=status;
      if(status.status==='done'||status.status==='error'||status.status==='interrupted'){
        cutWorking=false;
        if(cutPoll){clearInterval(cutPoll);cutPoll=undefined;}
        if(status.status==='done')notify('success','Clip enregistré. Vidéo originale inchangée.');
        if(status.status==='error'||status.status==='interrupted')notify('error',status.error||'Découpage interrompu.');
      }
    }catch(e){
      if(cutPoll){clearInterval(cutPoll);cutPoll=undefined;}
      cutWorking=false;
      notify('error',e instanceof Error?e.message:String(e));
    }
  }
  function watchTask(id:string){
    if(cutPoll)clearInterval(cutPoll);
    cutPoll=setInterval(()=>{void checkTask(id)},1800);
    void checkTask(id);
  }
  async function cut(){
    if(locked||cutWorking||!duration||!keepRanges.length)return;
    stopPreview();cutWorking=true;cutTask=undefined;
    try{
      cutTask=await api.cut(jobId,keepRanges);
      localStorage.setItem(taskKey(),cutTask.id);
      watchTask(cutTask.id);
    }catch(e){cutWorking=false;notify('error',e instanceof Error?e.message:String(e));}
  }
  onMount(()=>{
    video=document.getElementById('autosubs-editor-video') as HTMLVideoElement|null;
    if(!video)return;
    const load=()=>{
      duration=Number.isFinite(video?.duration)&&Number(video?.duration)>0?Number(video?.duration):0;
      keepRanges=restoreRanges(localStorage.getItem(draftKey()),duration);
    };
    video.addEventListener('loadedmetadata',load);
    video.addEventListener('durationchange',load);
    video.addEventListener('timeupdate',onPlayback);
    if(video.readyState>=1)load();
    const pending=localStorage.getItem(taskKey());
    if(pending){cutTask={id:pending,status:'queued'};cutWorking=true;watchTask(pending);}
    return ()=>{
      if(cutPoll)clearInterval(cutPoll);
      video?.removeEventListener('loadedmetadata',load);
      video?.removeEventListener('durationchange',load);
      video?.removeEventListener('timeupdate',onPlayback);
      previewing=false;
    };
  });
</script>

<svelte:window on:keydown={shortcuts}/>
<section class="card cut-card">
  <div class="card-header">
    <strong>✂ Découper dans la vidéo</strong>
    <span class="muted small">Original conservé · montage non destructif</span>
  </div>
  <div class="card-body stack">
    <div class="row between wrap">
      <span class="mono small">Lecture : {timeLabel(currentTime)}</span>
      <span class="mono small">{duration?timeLabel(duration):'Chargement de la durée…'}</span>
    </div>
    <div class="cut-track">
      <div class="cut-ranges" aria-hidden="true">
        {#each keepRanges as range}
          <div class="cut-range" style:left={`${range.start/duration*100}%`} style:width={`${(range.end-range.start)/duration*100}%`}></div>
        {/each}
        {#if hasIn&&duration}<div class="cut-mark start" style:left={`${cutIn/duration*100}%`}></div>{/if}
        {#if hasOut&&duration}<div class="cut-mark end" style:left={`${cutOut/duration*100}%`}></div>{/if}
      </div>
      <input type="range" aria-label="Déplacer la tête de lecture" min="0" max={duration||1} step="0.01"
             value={Math.min(currentTime,duration||0)} disabled={!duration}
             on:input={e=>seek(Number((e.currentTarget as HTMLInputElement).value))}/>
    </div>
    <div class="help">Déplacez le curseur, marquez <kbd>I</kbd> (début) puis <kbd>O</kbd> (fin). Ajoutez les passages à conserver.</div>
    <div class="row wrap cut-buttons">
      <button class="btn" disabled={locked||cutWorking||!duration} on:click={markIn} title="Raccourci I">⟦ Début (I)</button>
      <button class="btn" disabled={locked||cutWorking||!duration} on:click={markOut} title="Raccourci O">Fin (O) ⟧</button>
      <button class="btn primary" disabled={locked||cutWorking||!hasIn||!hasOut} on:click={add}>+ Garder le passage</button>
    </div>
    <div class="grid two">
      <div class="field"><label for="cut-mark-in">Début · secondes</label><input id="cut-mark-in" class="input mono" type="number" step="0.01" min="0" max={duration} bind:value={cutIn} on:input={()=>hasIn=true} disabled={cutWorking||locked||!duration}/></div>
      <div class="field"><label for="cut-mark-out">Fin · secondes</label><input id="cut-mark-out" class="input mono" type="number" step="0.01" min="0" max={duration} bind:value={cutOut} on:input={()=>hasOut=true} disabled={cutWorking||locked||!duration}/></div>
    </div>
    {#if keepRanges.length}
      <div class="row between wrap"><strong class="small">{keepRanges.length} passage(s) · {timeLabel(totalDuration(keepRanges))}</strong>
        <div class="row wrap">
          <button class="btn ghost" disabled={cutWorking} on:click={playKept}>{previewing?'■ Arrêter la prévisualisation':'▶ Prévisualiser les passages gardés'}</button>
          <button class="btn ghost" disabled={cutWorking} on:click={()=>remove(keepRanges.length-1)}>↶ Annuler le dernier</button>
        </div>
      </div>
      {#each keepRanges as range,i}
        <div class="row between wrap cut-piece">
          <button class="btn ghost mono" on:click={()=>seek(range.start)} title="Aller au début">▶ {i+1}. {timeLabel(range.start)} → {timeLabel(range.end)}</button>
          <button class="btn danger" disabled={cutWorking} on:click={()=>remove(i)} title="Enlever cette plage, pas la vidéo">Retirer</button>
        </div>
      {/each}
    {:else}
      <div class="help">Aucun passage sélectionné. Les zones conservées apparaîtront en couleur sur la timeline.</div>
    {/if}
    <div class="row between wrap">
      <button class="btn primary" disabled={locked||cutWorking||keepRanges.length===0||!duration} on:click={cut}>{cutWorking?'Export en cours…':'✂ Cut — créer le clip'}</button>
      <span class="help">Sortie MP4 H.264/AAC · points de coupe précis</span>
    </div>
    {#if cutTask?.status==='done'}
      <div class="help">Clip créé : <code>{cutTask.outputPath}</code></div>
      <a class="btn" href={`/api/v1/cuts/${cutTask.id}/download`}>↓ Télécharger le MP4</a>
    {:else if cutWorking}
      <div class="help" role="status">Découpage en cours sur Cloud9. Vous pouvez naviguer dans le reste d’AutoSubs.</div>
    {:else if cutTask?.status==='error'||cutTask?.status==='interrupted'}
      <div class="help" role="alert">Échec : {cutTask.error||'Interrompu lors du redémarrage du serveur.'}</div>
    {/if}
  </div>
</section>
<style>
  .cut-card{margin-block:8px 14px}
  .cut-track{position:relative;height:38px;border-radius:9px;background:var(--surface-2,#242831);border:1px solid var(--border,#4a4c52);overflow:hidden;display:flex;align-items:center}
  .cut-ranges{position:absolute;inset:6px 10px;pointer-events:none;border-radius:4px;background:rgba(127,127,127,.16)}
  .cut-range{position:absolute;top:0;bottom:0;border-radius:3px;background:#47b4a2;opacity:.88}
  .cut-mark{position:absolute;top:-4px;bottom:-4px;width:3px;background:#f0ba63}
  .cut-mark.end{background:#c8aeec}
  .cut-track input[type="range"]{position:relative;z-index:2;width:100%;margin:0 10px;accent-color:#ecf1f8;cursor:ew-resize;background:transparent}
  .cut-track input[type="range"]::-webkit-slider-runnable-track{background:transparent}
  .cut-track input[type="range"]::-moz-range-track{background:transparent}
  .cut-buttons button{min-height:37px}
  .cut-piece{padding:3px 0;border-bottom:1px solid var(--border,rgba(160,160,160,.2))}
  kbd{font-family:inherit;border:1px solid var(--border,#666);border-radius:4px;padding:1px 5px}
</style>
