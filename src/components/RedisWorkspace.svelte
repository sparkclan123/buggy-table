<script lang="ts">
  import { onMount } from 'svelte';
  import { api, errorMessage } from '../lib/api';
  import type { ConnectionInfo } from '../lib/types';
  let {
    connection,
    showOverview = false,
    busy = $bindable(false),
  }: { connection: ConnectionInfo; showOverview?: boolean; busy?: boolean } = $props();
  let pattern = $state('*');
  let cursor = $state('0');
  let scanned = $state(false);
  let keys = $state<string[]>([]);
  let selected = $state('');
  let keyType = $state('');
  let ttl = $state('');
  let output = $state<unknown>(null);
  let hasOutput = $state(false);
  let appliedPattern = $state('*');
  let error = $state('');
  let elapsed = $state(0);
  let command = $state('["PING"]');
  let activeRequest = $state<string | null>(null);
  let generation = 0;
  async function read(args: string[]) {
    activeRequest = crypto.randomUUID();
    return api.redis(connection.id, activeRequest, args);
  }
  async function run(work: (token: number) => Promise<void>) {
    if (busy) return;
    busy = true;
    error = '';
    const token = ++generation;
    try {
      await work(token);
    } catch (e) {
      if (generation === token) error = errorMessage(e);
    } finally {
      if (generation === token) {
        busy = false;
        activeRequest = null;
      }
    }
  }
  async function scan(reset = false) {
    await run(async () => {
      const result = await read([
        'SCAN',
        reset || pattern !== appliedPattern ? '0' : cursor,
        'MATCH',
        pattern,
      ]);
      const value = result.value;
      if (!Array.isArray(value) || value.length !== 2 || !Array.isArray(value[1]))
        throw new Error('Unexpected scan response.');
      appliedPattern = pattern;
      cursor = String(value[0]);
      keys = [...new Set(value[1].filter((v): v is string => typeof v === 'string'))];
      scanned = true;
      elapsed = result.elapsedMs;
    });
  }
  async function inspect(key: string) {
    await run(async () => {
      selected = key;
      output = null;
      hasOutput = false;
      keyType = '';
      ttl = '';
      keyType = String((await read(['TYPE', key])).value);
      const lifetime = String((await read(['TTL', key])).value);
      ttl =
        lifetime === '-1'
          ? 'No expiry'
          : lifetime === '-2'
            ? 'Key no longer exists'
            : `${lifetime} seconds`;
      const args =
        keyType === 'string'
          ? ['GET', key]
          : keyType === 'hash'
            ? ['HSCAN', key, '0']
            : keyType === 'list'
              ? ['LRANGE', key, '0', '199']
              : keyType === 'set'
                ? ['SSCAN', key, '0']
                : keyType === 'zset'
                  ? ['ZRANGE', key, '0', '199']
                  : ['TYPE', key];
      const result = await read(args);
      output = result.value;
      hasOutput = true;
      elapsed = result.elapsedMs;
    });
  }
  async function execute() {
    await run(async () => {
      const args: unknown = JSON.parse(command);
      if (!Array.isArray(args) || !args.every((v) => typeof v === 'string'))
        throw new Error('Enter a JSON array of command arguments, for example ["GET", "my key"].');
      const result = await read(args);
      output = result.value;
      hasOutput = true;
      elapsed = result.elapsedMs;
      selected = '';
      keyType = '';
      ttl = '';
    });
  }
  function showInfo(section: string) {
    command = JSON.stringify(['INFO', section]);
    void execute();
  }
  async function cancel() {
    if (activeRequest) await api.cancel(connection.id, activeRequest).catch(() => {});
  }
  function shortcut(event: KeyboardEvent) {
    if (event.isComposing || document.querySelector('dialog[open]')) return;
    if (
      event.key === 'F5' ||
      ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'r')
    ) {
      event.preventDefault();
      if (showOverview) void execute();
      else void scan(true);
    }
  }
  onMount(() => {
    if (showOverview) showInfo('server');
    else void scan(true);
    return () => {
      generation++;
      void cancel();
      busy = false;
    };
  });
</script>

<svelte:window onkeydown={shortcut} />
<section class="redis-workspace" aria-label="Redis workspace">
  <div class="table-context">
    <h1>Redis · {showOverview ? 'Server information' : `DB ${connection.database}`}</h1>
    <span>Read-only</span>
  </div>
  <div class="table-toolbar">
    {#if showOverview}
      {#each ['server', 'memory', 'stats', 'clients', 'keyspace'] as section}<button
          class="secondary-button"
          disabled={busy}
          onclick={() => showInfo(section)}>{section}</button
        >{/each}
    {:else}
      <label
        >Key pattern <input
          aria-label="Redis key pattern"
          bind:value={pattern}
          maxlength="4096"
          disabled={busy}
        /></label
      >
      <button class="secondary-button" onclick={() => scan(true)} disabled={busy}
        >Scan / refresh</button
      >
      <button
        class="secondary-button"
        onclick={() => scan()}
        disabled={busy || (scanned && cursor === '0' && pattern === appliedPattern)}
        >Next scan</button
      >
      <span class="muted small">Cursor {cursor} · {keys.length} keys in this batch</span>
    {/if}
    {#if busy}<button class="secondary-button" onclick={cancel}>Cancel</button>{/if}
  </div>
  {#if error}<div class="error-banner" role="alert">{error}</div>{/if}
  <div class="redis-browser" class:overview={showOverview}>
    {#if !showOverview}<div class="redis-keys" aria-label="Redis keys">
        {#each keys as key}<button
            class:active={selected === key}
            onclick={() => inspect(key)}
            disabled={busy}>{key}</button
          >{/each}
        {#if !keys.length}<p class="muted">
            {busy
              ? 'Scanning…'
              : cursor === '0'
                ? 'No keys in this batch.'
                : 'Empty batch; continue with Next scan.'}
          </p>{/if}
      </div>
    {/if}
    <div class="redis-value">
      <div class="table-toolbar">
        <strong>{selected || 'Command result'}</strong><span
          >{keyType} {ttl ? `· TTL: ${ttl}` : ''}</span
        >
      </div>
      <pre aria-label="Redis value">{!hasOutput
          ? 'Select a key or run a read command.'
          : typeof output === 'string'
            ? output
            : JSON.stringify(output, null, 2)}</pre>
    </div>
  </div>
  <form
    class="redis-command"
    onsubmit={(event) => {
      event.preventDefault();
      void execute();
    }}
  >
    <label
      >Read command · JSON arguments<input
        aria-label="Redis command"
        bind:value={command}
        maxlength="32768"
        spellcheck="false"
      /></label
    >
    <button class="primary-button" disabled={busy}>Run command</button>
  </form>
  <p class="muted small redis-help">
    Examples: ["GET", "key"], ["TTL", "key"], ["INFO", "memory"], ["HSCAN", "hash", "0"]. String
    previews: 64 KiB. Collections: first batch; use returned cursor to continue. SCAN can repeat
    keys and does not give a snapshot. Binary keys appear only in command output.
  </p>
  <div class="result-footer">
    <span>{elapsed} ms · Direct Redis connection</span><span>⌘/Ctrl+R or F5 refresh</span>
  </div>
</section>
