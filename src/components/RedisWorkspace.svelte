<script lang="ts">
  import { onMount } from 'svelte';
  import SnapshotChart from './SnapshotChart.svelte';
  import { numeric, parseRedisInfo, redisMetrics } from '../lib/stats';
  import { createRedisWorkspace, type RedisView } from '../lib/redis-workspace';
  import { api, errorMessage } from '../lib/api';
  import type { ConnectionInfo } from '../lib/types';
  let {
    connection,
    showOverview = false,
    busy = $bindable(false),
    view = $bindable(createRedisWorkspace()),
  }: {
    connection: ConnectionInfo;
    showOverview?: boolean;
    busy?: boolean;
    view?: RedisView;
  } = $props();
  const pattern = $derived(view.pattern);
  const cursor = $derived(view.cursor);
  const scanned = $derived(view.scanned);
  const keys = $derived(view.keys);
  const selected = $derived(view.selected);
  const keyType = $derived(view.keyType);
  const ttl = $derived(view.ttl);
  const output = $derived(view.output);
  const hasOutput = $derived(view.hasOutput);
  const appliedPattern = $derived(view.appliedPattern);
  const error = $derived(view.error);
  const elapsed = $derived(view.elapsed);
  const command = $derived(view.command);
  let activeRequest = $state<string | null>(null);
  let generation = 0;
  let keyTop = $state(0);
  let keyScroller = $state<HTMLDivElement>();
  const firstKey = $derived(Math.max(0, Math.floor(keyTop / 32) - 4));
  const shownKeys = $derived(keys.slice(firstKey, firstKey + 60));
  const infoMetrics = $derived(
    view.infoSection === 'keyspace'
      ? Object.keys(view.info ?? {})
          .filter((key) => /^db[0-9]+$/.test(key))
          .map((key) => ({ key, label: `${key} keys`, unit: 'keys' }))
      : (redisMetrics[view.infoSection] ?? []),
  );
  function metric(fields: Record<string, string>, key: string) {
    return numeric(
      /^db[0-9]+$/.test(key) ? fields[key]?.match(/(?:^|,)keys=([0-9]+)/)?.[1] : fields[key],
    );
  }
  function acceptScan(value: unknown, nextPattern: string, reset: boolean) {
    if (
      !Array.isArray(value) ||
      value.length !== 2 ||
      !Array.isArray(value[1]) ||
      !/^[0-9]+$/.test(String(value[0]))
    )
      throw new Error('Unexpected scan response.');
    const records = value[1].map((key: unknown) => {
      if (typeof key === 'string') return { id: `text:${key}`, label: key, value: key };
      if (
        key &&
        typeof key === 'object' &&
        'encoding' in key &&
        key.encoding === 'hex' &&
        'data' in key &&
        typeof key.data === 'string'
      )
        return { id: `hex:${key.data}`, label: `hex:${key.data}`, value: null };
      throw new Error('Redis returned an unsupported key record.');
    });
    const merged = new Map((reset ? [] : view.keys).map((key) => [key.id, key]));
    for (const key of records) merged.set(key.id, key);
    if (merged.size > 10000)
      throw new Error(
        'Scan reached the 10,000-key display limit. Narrow the key pattern; this is not a complete key list.',
      );
    if (reset) {
      keyTop = 0;
      if (keyScroller) keyScroller.scrollTop = 0;
      view.selected = '';
      view.hasOutput = false;
      view.output = null;
      view.keyType = '';
      view.ttl = '';
    }
    view.pattern = nextPattern;
    view.appliedPattern = nextPattern;
    view.cursor = String(value[0]);
    view.keys = [...merged.values()];
    view.scanned = true;
  }
  async function read(args: string[]) {
    activeRequest = crypto.randomUUID();
    return api.redis(connection.id, activeRequest, args);
  }
  async function run(work: (token: number) => Promise<void>) {
    if (busy) return;
    busy = true;
    view.error = '';
    const token = ++generation;
    try {
      await work(token);
    } catch (e) {
      if (generation === token) view.error = errorMessage(e);
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
      acceptScan(result.value, pattern, reset || pattern !== appliedPattern);
      view.elapsed = result.elapsedMs;
    });
  }
  async function inspect(key: string) {
    await run(async () => {
      view.selected = key;
      view.output = null;
      view.hasOutput = false;
      view.keyType = '';
      view.ttl = '';
      view.keyType = String((await read(['TYPE', key])).value);
      const lifetime = String((await read(['TTL', key])).value);
      view.ttl =
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
      view.output = result.value;
      view.hasOutput = true;
      view.elapsed = result.elapsedMs;
    });
  }
  async function execute() {
    await run(async () => {
      const args: unknown = JSON.parse(command);
      if (!Array.isArray(args) || !args.every((v) => typeof v === 'string'))
        throw new Error('Enter a JSON array of command arguments, for example ["GET", "my key"].');
      const result = await read(args);
      if (args[0]?.toUpperCase() === 'SCAN') {
        const scannedPattern = args.length === 4 ? args[3] : '*';
        acceptScan(
          result.value,
          scannedPattern,
          args[1] === '0' || scannedPattern !== appliedPattern,
        );
        view.elapsed = result.elapsedMs;
        return;
      }
      view.output = result.value;
      view.hasOutput = true;
      view.elapsed = result.elapsedMs;
      view.selected = '';
      view.keyType = '';
      view.ttl = '';
    });
  }
  async function showInfo(section: string) {
    if (busy) return;
    view.infoSection = section;
    view.info = null;
    await run(async () => {
      const result = await read(['INFO', section]);
      const fields = parseRedisInfo(result.value);
      const at = new Date().toISOString();
      view.info = fields;
      view.infoAt = at;
      view.elapsed = result.elapsedMs;
      let history = view.infoHistory[section] ?? [];
      if (section === 'server' && history.at(-1)?.fields.run_id !== fields.run_id) {
        view.infoHistory = {};
        history = [];
      }
      view.infoHistory[section] = [...history.slice(-29), { at, fields }];
    });
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
      if (showOverview) void showInfo(view.infoSection);
      else void scan(true);
    }
  }
  onMount(() => {
    if (showOverview) void showInfo(view.infoSection);
    else if (!view.scanned) void scan(true);
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
      <form
        class="redis-scan-form"
        role="search"
        aria-label="Redis key search"
        onsubmit={(event) => {
          event.preventDefault();
          void scan(true);
        }}
      >
        <label
          >Key pattern <input
            aria-label="Redis key pattern"
            bind:value={view.pattern}
            maxlength="4096"
            disabled={busy}
          /></label
        >
        <button class="secondary-button" type="submit" disabled={busy}>Scan / refresh</button>
        <button
          class="secondary-button"
          type="button"
          onclick={() => scan()}
          disabled={busy || (scanned && cursor === '0' && pattern === appliedPattern)}
          >Next scan</button
        >
        <span class="muted small"
          >Cursor {cursor} · {keys.length} keys loaded{scanned && cursor === '0'
            ? ' · scan complete'
            : ''}</span
        >
      </form>
    {/if}
    {#if busy}<button class="secondary-button" onclick={cancel}>Cancel</button>{/if}
  </div>
  {#if error}<div class="error-banner" role="alert">{error}</div>{/if}
  {#if showOverview}
    {#if view.info && !error}
      <p class="muted small redis-help">
        Manual snapshot · {view.infoAt} · INFO {view.infoSection}
      </p>
      <div class="snapshot-charts redis-charts" aria-label="Redis statistics graphs">
        {#each infoMetrics as item}
          <SnapshotChart
            label={item.label}
            value={metric(view.info, item.key) === null
              ? null
              : /^db[0-9]+$/.test(item.key)
                ? (view.info[item.key]?.match(/(?:^|,)keys=([0-9]+)/)?.[1] ?? null)
                : view.info[item.key]}
            unit={item.unit}
            points={(view.infoHistory[view.infoSection] ?? []).flatMap((point) => {
              const value = metric(point.fields, item.key);
              return value === null ? [] : [{ value, at: point.at }];
            })}
          />
        {/each}
        {#if !infoMetrics.length}<p class="muted">
            No database keys were reported in this snapshot.
          </p>{/if}
      </div>
      <details class="redis-info-details">
        <summary>Snapshot fields</summary>
        <dl>
          {#each Object.entries(view.info) as [name, value]}<dt>{name}</dt>
            <dd>{value}</dd>{/each}
        </dl>
      </details>
    {:else}<p class="muted redis-help" role="status">
        {busy
          ? 'Reading server statistics…'
          : 'Statistics unavailable. The connected account may not have permission to read this INFO section.'}
      </p>{/if}
  {:else}
    <div class="redis-browser">
      <div
        class="redis-keys"
        bind:this={keyScroller}
        aria-label="Redis keys"
        onscroll={(event) => (keyTop = event.currentTarget.scrollTop)}
      >
        <table class="redis-key-records" aria-label="Redis key records">
          <thead><tr><th>#</th><th>Key</th></tr></thead><tbody>
            {#if firstKey > 0}<tr aria-hidden="true"
                ><td colspan="2" style:height={`${firstKey * 32}px`}></td></tr
              >{/if}
            {#each shownKeys as key, index (key.id)}<tr class:active={selected === key.value}
                ><td>{firstKey + index + 1}</td><td>
                  {#if key.value !== null}<button
                      class:active={selected === key.value}
                      onclick={() => inspect(key.value!)}
                      disabled={busy}
                      title={key.label}>{key.label || '(empty key)'}</button
                    >
                  {:else}<span title="Binary key · text-only inspection is unavailable"
                      >{key.label} (binary)</span
                    >{/if}
                </td></tr
              >{/each}
            {#if firstKey + shownKeys.length < keys.length}<tr aria-hidden="true"
                ><td
                  colspan="2"
                  style:height={`${(keys.length - firstKey - shownKeys.length) * 32}px`}
                ></td></tr
              >{/if}
          </tbody>
        </table>
        {#if !keys.length}<p class="muted">
            {busy
              ? 'Scanning…'
              : cursor === '0'
                ? 'No matching keys.'
                : 'No matches yet; continue with Next scan.'}
          </p>{/if}
      </div>
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
          bind:value={view.command}
          maxlength="32768"
          spellcheck="false"
        /></label
      >
      <button class="primary-button" disabled={busy}>Run command</button>
    </form>
    <p class="muted small redis-help">
      Examples: ["GET", "key"], ["TTL", "key"], ["INFO", "memory"], ["HSCAN", "hash", "0"]. String
      previews: 64 KiB. Collections: first batch; use returned cursor to continue. SCAN can repeat
      keys and does not give a snapshot. Binary keys are listed as hex records; text-key inspection
      is unavailable for them.
    </p>
  {/if}
  <div class="result-footer">
    <span>{elapsed} ms · Direct Redis connection</span><span>⌘/Ctrl+R or F5 refresh</span>
  </div>
</section>
