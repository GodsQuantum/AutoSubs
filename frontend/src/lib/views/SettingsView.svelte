<script lang="ts">
  import { api } from '$lib/api';
  import { dictionary } from '$lib/i18n';
  import type { Capabilities, SettingsView } from '$lib/types';
  export let settings:SettingsView|undefined; export let capabilities:Capabilities|undefined;
  export let refresh:()=>Promise<void>=async()=>{}; export let notify:(type:'error'|'success'|'info',message:string)=>void=()=>{};
  let draft:SettingsView|undefined; let loaded:SettingsView|undefined;
  let transKey='';let transAction:'keep'|'replace'|'clear'='keep';let localKey='';let localAction:'keep'|'replace'|'clear'='keep';let llmKey='';let llmAction:'keep'|'replace'|'clear'='keep';
  let saving=false;let primaryModels:string[]=[];let localModels:string[]=[];
  $: if(settings&&settings!==loaded){loaded=settings;draft=JSON.parse(JSON.stringify(settings));transKey='';localKey='';llmKey='';transAction=localAction=llmAction='keep'}
  const secret=(action:string,value:string)=>action==='keep'?undefined:{action,value};
  async function save(){if(!draft)return;saving=true;try{await api.saveSettings({...draft,transcriptionApiKey:secret(transAction,transKey),localTranscriptionApiKey:secret(localAction,localKey),llmApiKey:secret(llmAction,llmKey)});await refresh();notify('success',$dictionary.saved)}catch(e){notify('error',e instanceof Error?e.message:String(e))}finally{saving=false}}
  async function models(kind:'primary'|'local'){if(!draft)return;try{const endpoint=kind==='primary'?draft.transcriptionUrl:draft.localTranscriptionUrl;const key=kind==='primary'?transKey:localKey;const result=await api.models(endpoint,key);if(kind==='primary')primaryModels=result.models;else localModels=result.models}catch(e){notify('error',e instanceof Error?e.message:String(e))}}
  const yes=(v:boolean|undefined)=>v?$dictionary.available:$dictionary.unavailable;
</script>
<div class="page">
  <div class="page-head"><div><h1 class="page-title">{$dictionary.settings}</h1><p class="page-kicker">{$dictionary.settingsInfo}</p></div><div class="page-actions"><button class="btn primary" disabled={!draft||saving} on:click={save}>{saving?$dictionary.saving:$dictionary.save}</button></div></div>
  {#if !draft}<div class="empty">{$dictionary.loading}</div>{:else}
  <div class="grid two">
    <section class="card"><div class="card-header"><strong>{$dictionary.primaryProvider}</strong></div><div class="card-body stack">
      <div class="field"><label for="settings-field-1">{$dictionary.endpoint}</label><input id="settings-field-1" class="input" bind:value={draft.transcriptionUrl} placeholder="https://…/v1/audio/transcriptions"/></div>
      <div class="grid two"><div class="field"><label for="settings-field-2">{$dictionary.model}</label><input id="settings-field-2" class="input" list="primary-models" bind:value={draft.transcriptionModel}/><datalist id="primary-models">{#each primaryModels as m}<option value={m}></option>{/each}</datalist></div><div class="field"><label for="settings-field-3">{$dictionary.transcriptionLanguage}</label><input id="settings-field-3" class="input" bind:value={draft.language} placeholder="fr"/></div></div>
      <div class="field"><label for="settings-field-4">{$dictionary.apiKey} {draft.transcriptionApiKeySet?`· ${$dictionary.keyStored}`:''}</label><div class="row"><select id="settings-field-4" class="select" style="max-width:150px" bind:value={transAction}><option value="keep">{$dictionary.keepKey}</option><option value="replace">{$dictionary.replaceKey}</option><option value="clear">{$dictionary.clearKey}</option></select><input class="input" type="password" aria-label={$dictionary.apiKey} disabled={transAction!=='replace'} bind:value={transKey}/></div></div>
      <button class="btn" disabled={!draft.transcriptionUrl} on:click={()=>models('primary')}>{$dictionary.testModels}</button>
    </div></section>

    <section class="card"><div class="card-header"><strong>{$dictionary.localProvider}</strong></div><div class="card-body stack">
      <div class="row wrap"><label class="check"><input type="checkbox" bind:checked={draft.localTranscriptionEnabled}/>{$dictionary.localEnabled}</label><label class="check"><input type="checkbox" bind:checked={draft.localFallbackEnabled}/>{$dictionary.fallbackEnabled}</label></div>
      <div class="field"><label for="settings-field-5">{$dictionary.endpoint}</label><input id="settings-field-5" class="input" bind:value={draft.localTranscriptionUrl} placeholder="http://transcriber:8000/v1/audio/transcriptions"/></div>
      <div class="field"><label for="settings-field-6">{$dictionary.model}</label><input id="settings-field-6" class="input" list="local-models" bind:value={draft.localTranscriptionModel}/><datalist id="local-models">{#each localModels as m}<option value={m}></option>{/each}</datalist></div>
      <div class="field"><label for="settings-field-7">{$dictionary.apiKey} {draft.localTranscriptionApiKeySet?`· ${$dictionary.keyStored}`:''}</label><div class="row"><select id="settings-field-7" class="select" style="max-width:150px" bind:value={localAction}><option value="keep">{$dictionary.keepKey}</option><option value="replace">{$dictionary.replaceKey}</option><option value="clear">{$dictionary.clearKey}</option></select><input class="input" type="password" aria-label={$dictionary.apiKey} disabled={localAction!=='replace'} bind:value={localKey}/></div></div>
      <button class="btn" disabled={!draft.localTranscriptionUrl} on:click={()=>models('local')}>{$dictionary.testModels}</button>
    </div></section>

    <section class="card"><div class="card-header"><strong>{$dictionary.llm}</strong></div><div class="card-body stack">
      <label class="check"><input type="checkbox" bind:checked={draft.llmEnabled}/>{$dictionary.llmEnabled}</label>
      <div class="field"><label for="settings-field-8">{$dictionary.endpoint}</label><input id="settings-field-8" class="input" bind:value={draft.llmEndpoint}/></div>
      <div class="field"><label for="settings-field-9">{$dictionary.model}</label><input id="settings-field-9" class="input" bind:value={draft.llmModel}/></div>
      <div class="field"><label for="settings-field-10">{$dictionary.prompt}</label><textarea id="settings-field-10" class="textarea" bind:value={draft.llmPrompt}></textarea></div>
      <div class="field"><label for="settings-field-11">{$dictionary.apiKey} {draft.llmApiKeySet?`· ${$dictionary.keyStored}`:''}</label><div class="row"><select id="settings-field-11" class="select" style="max-width:150px" bind:value={llmAction}><option value="keep">{$dictionary.keepKey}</option><option value="replace">{$dictionary.replaceKey}</option><option value="clear">{$dictionary.clearKey}</option></select><input class="input" type="password" aria-label={$dictionary.apiKey} disabled={llmAction!=='replace'} bind:value={llmKey}/></div></div>
    </div></section>

    <section class="card"><div class="card-header"><strong>{$dictionary.encoding}</strong></div><div class="card-body stack">
      <div class="field"><label for="settings-field-12">{$dictionary.encoder}</label><select id="settings-field-12" class="select" bind:value={draft.encoder.kind}><option value="auto">{$dictionary.auto}</option><option value="libx264">libx264</option><option value="libx265">libx265</option><option value="nvenc_h264">{$dictionary.nvencH264}</option><option value="nvenc_hevc">{$dictionary.nvencHevc}</option><option value="qsv_h264">{$dictionary.qsvH264}</option><option value="vaapi_h264">{$dictionary.vaapiH264}</option><option value="amf_h264">{$dictionary.amfH264}</option></select></div>
      <div class="grid two"><div class="field"><label for="settings-field-13">{$dictionary.quality}</label><input id="settings-field-13" class="input" type="number" min="0" max="51" bind:value={draft.encoder.quality}/></div><div class="field"><label for="settings-field-14">{$dictionary.encoderPreset}</label><input id="settings-field-14" class="input" bind:value={draft.encoder.preset}/></div></div>
      <div class="divider"></div><strong class="small">{$dictionary.capabilities}</strong>
      <div class="resource-meta"><span class="chip">FFmpeg: {yes(capabilities?.ffmpeg)}</span><span class="chip">libass: {yes(capabilities?.libass)}</span><span class="chip">NVENC: {yes(capabilities?.h264Nvenc)}</span><span class="chip">QSV: {yes(capabilities?.h264Qsv)}</span><span class="chip">VA-API: {yes(capabilities?.h264Vaapi)}</span><span class="chip">AMF: {yes(capabilities?.h264Amf)}</span></div>
    </div></section>
  </div>
  <section class="card" style="margin-top:14px"><div class="card-body"><div class="help">{$dictionary.mobileTip}</div></div></section>
  {/if}
</div>
