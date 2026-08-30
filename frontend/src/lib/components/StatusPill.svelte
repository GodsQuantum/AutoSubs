<script lang="ts">
  import { dictionary } from '$lib/i18n';
  import type { JobStatus } from '$lib/types';
  import type { Dictionary } from '$lib/i18n';
  export let status: JobStatus;
  const labelKey: Record<JobStatus, keyof Dictionary> = {
    pending:'pending', uploading:'uploadingStatus', probing:'probing', transcribing:'transcribing', correcting:'correcting',
    ready:'ready', rendering:'rendering', done:'done', error:'failed', cancelled:'cancelled', interrupted:'interrupted'
  } as const;
</script>
<span class="status-pill {status}"><span class="status-dot"></span>{$dictionary[labelKey[status]]}</span>
