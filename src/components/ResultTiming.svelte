<script lang="ts">
  import type { QueryResult } from '../lib/types';

  let { result }: { result: QueryResult } = $props();
  const description = $derived(
    result.timings
      ? `Connection: ${result.timings.connectionMs} ms · Checks and schema: ${result.timings.validationMs} ms · Query and transfer: ${result.timings.executionMs} ms. Total includes all backend stages; excludes UI rendering.`
      : 'Query and transfer time; excludes connection setup and UI rendering.',
  );
</script>

<span title={description} aria-label={description}>
  {result.elapsedMs} ms{result.timings ? ' total' : ''}
</span>
