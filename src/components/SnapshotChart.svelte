<script lang="ts">
  import type { SnapshotPoint } from '../lib/stats';
  let {
    label,
    value,
    unit = '',
    points = [],
    ceiling = null,
  }: {
    label: string;
    value: string | number | null;
    unit?: string;
    points?: SnapshotPoint[];
    ceiling?: number | null;
  } = $props();
  const valid = $derived(
    value === null ? [] : points.filter((p) => Number.isFinite(p.value) && p.value >= 0),
  );
  const displayed = $derived(
    typeof value === 'string' && /^\d+$/.test(value) ? BigInt(value).toLocaleString() : value,
  );
  const max = $derived(Math.max(ceiling ?? 0, ...valid.map((p) => p.value), 1));
  const coords = $derived(
    valid.map((p, i) => ({
      x: valid.length === 1 ? 150 : 12 + (i * 276) / (valid.length - 1),
      y: 72 - (p.value / max) * 60,
      ...p,
    })),
  );
  const path = $derived(coords.map((p) => `${p.x},${p.y}`).join(' '));
  const latest = $derived(valid.at(-1));
</script>

<article class="snapshot-chart">
  <span>{label}</span><strong
    >{displayed ?? 'Unavailable'}{#if value !== null && unit}<small>{unit}</small>{/if}</strong
  >
  {#if coords.length}
    <svg
      viewBox="0 0 300 90"
      role="img"
      aria-label={`${label} graph: ${value} ${unit}; ${coords.length} manual snapshot${coords.length === 1 ? '' : 's'}`}
    >
      <title>{label} · {latest?.at} · manual snapshots</title>
      <path d="M12 12V72H288" class="axis" />
      {#if ceiling !== null && latest}<rect
          x="12"
          y="32"
          width="276"
          height="16"
          class="track"
        /><rect
          x="12"
          y="32"
          width={Math.max(0, Math.min(276, (latest.value / max) * 276))}
          height="16"
          class="bar"
        />{/if}
      {#if ceiling === null}<polyline points={path} class="line" />{#each coords as point}<circle
            cx={point.x}
            cy={point.y}
            r="3"
            class="dot"><title>{point.at}: {point.value} {unit}</title></circle
          >{/each}{/if}
      <text x="12" y="86">0</text><text x="288" y="10" text-anchor="end"
        >{max.toLocaleString(undefined, { maximumFractionDigits: 2 })}</text
      >
    </svg>
    <small
      >{ceiling !== null
        ? 'Current snapshot'
        : coords.length === 1
          ? '1 snapshot · refresh to compare'
          : `${coords.length} manual snapshots · latest at right`}</small
    >
  {:else}<p class="muted small">Statistics unavailable for this metric.</p>{/if}
</article>

<style>
  article {
    min-width: 0;
    padding: 16px;
    border: 1px solid var(--line);
    border-radius: 7px;
    background: var(--canvas);
  }
  article > span,
  article > small {
    display: block;
    color: var(--muted);
    font-size: 11px;
  }
  strong {
    display: block;
    margin: 10px 0 4px;
    font-size: 19px;
    overflow-wrap: anywhere;
  }
  strong small {
    margin-left: 4px;
    font-size: 11px;
    font-weight: 400;
  }
  svg {
    width: 100%;
    height: 96px;
    display: block;
  }
  .axis {
    fill: none;
    stroke: var(--border);
  }
  .line {
    fill: none;
    stroke: var(--focus);
    stroke-width: 2;
  }
  .dot,
  .bar {
    fill: var(--focus);
  }
  .track {
    fill: var(--hover);
  }
  text {
    font-size: 9px;
    fill: var(--muted);
  }
</style>
