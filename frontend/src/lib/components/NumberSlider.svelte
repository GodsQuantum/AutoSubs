<script lang="ts">
  export let label = '';
  export let value = 0;
  export let min = 0;
  export let max = 100;
  export let step = 1;
  export let unit = '';
  export let onChange: (value:number)=>void = ()=>{};

  function update(event: Event) {
    const raw = Number((event.currentTarget as HTMLInputElement).value);
    if (!Number.isFinite(raw)) return;
    const next = Math.min(max, Math.max(min, raw));
    value = next;
    onChange(next);
  }
</script>

<label class="number-slider">
  <span class="label">{label}</span>
  <span class="controls">
    <input class="slider" type="range" {min} {max} {step} value={value} on:input={update} aria-label={label + ' slider'} />
    <span class="numeric">
      <input class="input" type="number" {min} {max} {step} value={value} on:input={update} aria-label={label} />
      {#if unit}<span>{unit}</span>{/if}
    </span>
  </span>
</label>

<style>
  .number-slider { display:grid; gap:6px; min-width:0; }
  .label { font-size:12px; font-weight:700; color:var(--muted); }
  .controls { display:grid; grid-template-columns:minmax(90px,1fr) minmax(86px,112px); gap:8px; align-items:center; }
  .slider { width:100%; accent-color:var(--accent); }
  .numeric { display:flex; gap:5px; align-items:center; min-width:0; }
  .numeric .input { min-width:0; width:100%; }
  .numeric span { color:var(--muted); font-size:11px; }
  @media (max-width:640px) { .controls { grid-template-columns:1fr; } }
</style>
