<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { dictionary } from '$lib/i18n';
  import type { Asset, Brand, Capabilities, Job, Preset, SettingsView, Workflow } from '$lib/types';
  import AppShell from '$lib/components/AppShell.svelte';
  import ToastHost from '$lib/components/ToastHost.svelte';
  import type { Toast } from '$lib/toast';
  import QueueView from '$lib/views/QueueView.svelte';
  import EditorView from '$lib/views/EditorView.svelte';
  import PresetsView from '$lib/views/PresetsView.svelte';
  import BrandsView from '$lib/views/BrandsView.svelte';
  import WorkflowsView from '$lib/views/WorkflowsView.svelte';
  import SettingsViewComponent from '$lib/views/SettingsView.svelte';

  type View = 'queue'|'editor'|'presets'|'brands'|'workflows'|'settings';
  let active:View='queue'; let selectedJobId='';
  let jobs:Job[]=[];let presets:Preset[]=[];let brands:Brand[]=[];let workflows:Workflow[]=[];let assets:Asset[]=[];let settings:SettingsView|undefined;let capabilities:Capabilities|undefined;
  let toasts:Toast[]=[];let toastSeq=0;let initialError='';
  $: selectedJob=jobs.find(j=>j.id===selectedJobId);

  function notify(type:Toast['type'],message:string){const id=++toastSeq;toasts=[...toasts,{id,type,message}];setTimeout(()=>toasts=toasts.filter(t=>t.id!==id),4500)}
  async function refreshJobs(){try{jobs=await api.jobs();if(selectedJobId&&!jobs.some(j=>j.id===selectedJobId))selectedJobId=''}catch(e){notify('error',e instanceof Error?e.message:String(e))}}
  async function loadAll(){
    try{
      const [j,p,b,w,a,s,c]=await Promise.all([api.jobs(),api.presets(),api.brands(),api.workflows(),api.assets(),api.settings(),api.capabilities()]);
      jobs=j;presets=p;brands=b;workflows=w;assets=a;settings=s;capabilities=c;initialError='';
    }catch(e){initialError=e instanceof Error?e.message:String(e);}
  }
  function nav(view:View){active=view;localStorage.setItem('autosubs:view',view)}
  function openEditor(id:string){selectedJobId=id;localStorage.setItem('autosubs:selectedJob',id);nav('editor')}
  onMount(()=>{
    const saved=localStorage.getItem('autosubs:view') as View|null;if(saved&&['queue','editor','presets','brands','workflows','settings'].includes(saved))active=saved;
    selectedJobId=localStorage.getItem('autosubs:selectedJob')||'';loadAll();
    let timer:ReturnType<typeof setTimeout>|undefined;
    const es=new EventSource('/api/v1/events');
    es.onmessage=()=>{if(timer)clearTimeout(timer);timer=setTimeout(refreshJobs,120)};
    es.onerror=()=>{};
    return()=>{es.close();if(timer)clearTimeout(timer)};
  });
</script>

<svelte:head><title>AutoSubs</title><meta name="description" content="Self-hosted subtitle production workbench"/><meta name="theme-color" content="#0a0d0f"/></svelte:head>
<AppShell {active} onnav={nav}>
  {#if initialError}<div class="page"><div class="card"><div class="card-body" style="color:var(--danger)"><strong>{$dictionary.connectionLost}</strong><div class="small" style="margin-top:6px">{initialError}</div><button class="btn" style="margin-top:12px" on:click={loadAll}>{$dictionary.retry}</button></div></div></div>{/if}
  {#if active==='queue'}<QueueView {jobs} {presets} refresh={refreshJobs} {openEditor} {notify}/>
  {:else if active==='editor'}<EditorView job={selectedJob} {presets} refresh={refreshJobs} {notify}/>
  {:else if active==='presets'}<PresetsView {presets} {brands} refresh={loadAll} {notify}/>
  {:else if active==='brands'}<BrandsView {brands} {presets} {assets} refresh={loadAll} {notify}/>
  {:else if active==='workflows'}<WorkflowsView {workflows} {brands} {presets} refresh={loadAll} {notify}/>
  {:else}<SettingsViewComponent {settings} {capabilities} refresh={loadAll} {notify}/>{/if}
</AppShell>
<ToastHost {toasts}/>
