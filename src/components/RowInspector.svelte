<script lang="ts">
  import { X } from '@lucide/svelte';
  import { inspectorValue, inspectorWidth } from '../lib/row-inspector';
  import type { ColumnInfo } from '../lib/types';

  let {
    fields,
    rowNumber,
    deleted = false,
    widthPercent = 10,
    onresize,
    onclose,
  }: {
    fields: { name: string; value: string | null; column?: ColumnInfo; edited: boolean }[];
    rowNumber: number;
    deleted?: boolean;
    widthPercent?: number;
    onresize: (percent: number) => void;
    onclose: () => void;
  } = $props();
  let panel: HTMLElement;
  let drag = $state<{ x: number; width: number; total: number } | null>(null);
  let draftWidth = $state(10);
  const percent = $derived(drag ? draftWidth : widthPercent);
  const formatted = $derived(
    fields.map((field) => ({
      ...field,
      text: inspectorValue(field.value, field.column?.dataType),
    })),
  );
  function endResize() {
    if (!drag) return;
    onresize(draftWidth);
    drag = null;
  }
  function resetScroll(node: HTMLElement, row: number) {
    return {
      update(next: number) {
        if (next !== row) node.scrollTop = 0;
        row = next;
      },
    };
  }
</script>

<aside class="row-inspector" aria-label="Row details" style:width={`${percent}%`} bind:this={panel}>
  <!-- A focusable separator is the ARIA window splitter pattern; arrow keys resize it. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="inspector-resizer"
    role="separator"
    tabindex="0"
    aria-label="Resize row details"
    aria-orientation="vertical"
    aria-valuemin="8"
    aria-valuemax="60"
    aria-valuenow={Math.round(percent)}
    aria-valuetext={`${Math.round(percent)}% of table area`}
    title="Drag to resize · ←/→ adjust · double-click to reset"
    onpointerdown={(event) => {
      if (event.button !== 0) return;
      event.preventDefault();
      event.currentTarget.focus();
      draftWidth = widthPercent;
      drag = {
        x: event.clientX,
        width: widthPercent,
        total: panel.parentElement!.getBoundingClientRect().width,
      };
      event.currentTarget.setPointerCapture(event.pointerId);
    }}
    onpointermove={(event) => {
      if (drag && drag.total > 0)
        draftWidth = inspectorWidth(drag.width + ((drag.x - event.clientX) / drag.total) * 100);
    }}
    onpointerup={endResize}
    onpointercancel={endResize}
    onlostpointercapture={endResize}
    ondblclick={() => onresize(10)}
    onkeydown={(event) => {
      if (['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) {
        event.preventDefault();
        onresize(
          event.key === 'Home'
            ? 10
            : event.key === 'End'
              ? 60
              : inspectorWidth(
                  percent + (event.key === 'ArrowLeft' ? 1 : -1) * (event.shiftKey ? 10 : 2),
                ),
        );
      }
    }}
  ></div>
  <header class="inspector-header">
    <span title={`Row ${rowNumber} · Read only`}>Row {rowNumber}</span>
    <button
      class="icon-button"
      aria-label="Close row details"
      title="Close row details"
      onclick={onclose}><X size={13} /></button
    >
  </header>
  <!-- Keyboard users must be able to focus and scroll the read-only values.
       Backspace must not invoke WebKit history navigation from this region. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="inspector-scroll"
    tabindex="0"
    role="region"
    aria-label={`Row ${rowNumber} values`}
    use:resetScroll={rowNumber}
    onkeydown={(event) => {
      if (['Backspace', 'Delete'].includes(event.key)) {
        event.preventDefault();
        event.stopPropagation();
      }
    }}
  >
    <p class="inspector-caption">{deleted ? 'Pending deletion' : 'Read only'}</p>
    <dl>
      {#each formatted as field, i (i)}
        <div
          class="inspector-field"
          class:inspector-edited={field.edited}
          class:inspector-deleted={deleted}
        >
          <dt>
            <span class="inspector-name">{field.name}</span>
            {#if field.column}<small>{field.column.dataType}</small>{/if}
            {#if field.edited && !deleted}<small class="inspector-draft">Unsaved</small>{/if}
          </dt>
          <dd><pre class:null-value={field.value === null}>{field.text}</pre></dd>
        </div>
      {/each}
    </dl>
  </div>
</aside>
