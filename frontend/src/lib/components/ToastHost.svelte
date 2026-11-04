<script lang="ts">
  import type { Toast } from '#lib/toast.js';
  export let toasts: Toast[] = [];
</script>
<div class="toast-stack" aria-live="polite">
  {#each toasts as toast (toast.id)}
    <div class="toast {toast.type}">{toast.message}</div>
  {/each}
</div>
