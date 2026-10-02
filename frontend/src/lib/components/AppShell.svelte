<script lang="ts">
  import { dictionary, locale, setLocale } from '$lib/i18n';
  export let active: 'queue'|'editor'|'presets'|'brands'|'workflows'|'settings' = 'queue';
  export let onnav: (id: typeof active) => void = () => {};
  const items = [
    {id:'queue',key:'queue',glyph:'���'}, {id:'editor',key:'editor',glyph:'CC'}, {id:'presets',key:'presets',glyph:'���'},
    {id:'brands',key:'brands',glyph:'���'}, {id:'workflows',key:'workflows',glyph:'���'}, {id:'settings',key:'settings',glyph:'���'}
  ] as const;
</script>
<div class="app-shell">
  <aside class="app-rail">
    <div class="brand-lockup"><img src="/icon.svg" alt=""><div><div class="brand-title">AutoSubs</div><div class="brand-sub">Rust �� FFmpeg</div></div></div>
    <nav class="nav-list" aria-label="Main navigation">
      {#each items as item}
        <button class="nav-button" class:active={active===item.id} on:click={()=>onnav(item.id)} aria-label={$dictionary[item.key]} aria-current={active===item.id?'page':undefined}><span class="nav-icon" aria-hidden="true">{item.glyph}</span><span class="nav-label">{$dictionary[item.key]}</span></button>
      {/each}
    </nav>
    <div class="rail-spacer"></div>
    <div class="locale-switch" aria-label={$dictionary.uiLanguage}><button class:active={$locale==='en'} on:click={()=>setLocale('en')} aria-pressed={$locale==='en'} aria-label="English">EN</button><button class:active={$locale==='fr'} on:click={()=>setLocale('fr')} aria-pressed={$locale==='fr'} aria-label="Fran��ais">FR</button></div>
  </aside>
  <main class="app-main"><slot /></main>
</div>
<div class="mobile-locale" aria-label={$dictionary.uiLanguage}><button class:active={$locale==='en'} on:click={()=>setLocale('en')} aria-pressed={$locale==='en'} aria-label="English">EN</button><button class:active={$locale==='fr'} on:click={()=>setLocale('fr')} aria-pressed={$locale==='fr'} aria-label="Fran��ais">FR</button></div>
<nav class="bottom-nav" aria-label="Mobile navigation">
  {#each items as item}<button class:active={active===item.id} on:click={()=>onnav(item.id)} aria-label={$dictionary[item.key]} aria-current={active===item.id?'page':undefined}><span class="nav-icon" aria-hidden="true">{item.glyph}</span><span>{$dictionary[item.key]}</span></button>{/each}
</nav>
