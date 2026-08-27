<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { dictionary } from '$lib/i18n';
  import type { BrowseEntry } from '$lib/types';
  export let open = false;
  export let mode: 'file'|'directory'|'any' = 'any';
  export let initialPath = '';
  export let extensions = '';
  export let title = '';
  export let onselect: (path:string) => void = () => {};
  export let onclose: () => void = () => {};

  let currentPath = '';
  let parentPath: string | undefined;
  let roots: string[] = [];
  let entries: BrowseEntry[] = [];
  let loading = false;
  let error = '';
  let filter = '';
  let lastOpen = false;

  async function load(path = '') {
    loading = true; error = '';
    try {
      const data = await api.browse(path, mode, extensions);
      currentPath = data.currentPath; parentPath = data.parentPath; roots = data.roots; entries = data.entries;
    } catch (e) { error = e instanceof Error ? e.message : String(e); }
    finally { loading = false; }
  }
  function activate(entry: BrowseEntry) {
    if (entry.isDir) { load(entry.path); return; }
    if (entry.selectable) { onselect(entry.path); onclose(); }
  }
  function chooseCurrent() {
    if (mode === 'directory' || mode === 'any') { onselect(currentPath); onclose(); }
  }
  $: filtered = entries.filter((e) => e.name.toLowerCase().includes(filter.toLowerCase()));
  $: if (open && !lastOpen) { filter = ''; load(initialPath); }
  $: lastOpen = open;
  onMount(() => { if (open) load(initialPath); });
</script>

{#if open}
  <div class="modal-backdrop" role="presentation" on:click={(e)=>{ if(e.currentTarget===e.target)onclose(); }}>
    <div class="modal" role="dialog" aria-modal="true" aria-label={title || $dictionary.filePicker}>
      <div class="modal-head">
        <strong>{title || $dictionary.filePicker}</strong>
        <button class="btn icon ghost" on:click={onclose} aria-label={$dictionary.close}>×</button>
      </div>
      <div class="picker-path mono" title={currentPath}>{currentPath || $dictionary.loading}</div>
      <div class="picker-list">
        <div class="row" style="margin-bottom:8px">
          {#if parentPath}<button class="btn" on:click={()=>load(parentPath)}>← {$dictionary.back}</button>{/if}
          <input class="input" bind:value={filter} placeholder={$dictionary.filter} aria-label={$dictionary.filter} />
          <button class="btn icon" on:click={()=>load(currentPath)} aria-label={$dictionary.refresh}>↻</button>
        </div>
        {#if roots.length > 1}
          <div class="row wrap" style="margin-bottom:8px">
            {#each roots as root}<button class="btn ghost small mono" on:click={()=>load(root)}>{root}</button>{/each}
          </div>
        {/if}
        {#if loading}<div class="empty">{$dictionary.loading}</div>
        {:else if error}<div class="empty" style="color:var(--danger)">{error}</div>
        {:else if filtered.length===0}<div class="empty">{$dictionary.noEntries}</div>
        {:else}
          {#each filtered as entry}
            <button class="picker-row" on:dblclick={()=>activate(entry)} on:click={()=> entry.isDir ? load(entry.path) : entry.selectable && activate(entry)}>
              <span>{entry.isDir ? '▰' : '▤'}</span>
              <span><strong>{entry.name}</strong><span class="meta mono">{entry.path}</span></span>
              {#if !entry.isDir && entry.size !== undefined}<span class="meta">{Math.max(1,Math.round(entry.size/1024/1024))} MB</span>{/if}
            </button>
          {/each}
        {/if}
      </div>
      <div class="modal-foot">
        <span class="muted small">{mode==='directory' ? $dictionary.folder : mode==='file' ? $dictionary.file : $dictionary.filePicker}</span>
        <div class="row"><button class="btn" on:click={onclose}>{$dictionary.close}</button>{#if mode!=='file'}<button class="btn primary" disabled={!currentPath} on:click={chooseCurrent}>{$dictionary.choose}</button>{/if}</div>
      </div>
    </div>
  </div>
{/if}
