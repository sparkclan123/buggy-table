<script lang="ts">
  import { onDestroy, tick } from 'svelte';
  import { Table2, X } from '@lucide/svelte';
  import { pendingRows, type TableWorkspace } from '../lib/workspace';

  let {
    tabs,
    activeId,
    busy,
    onactivate,
    onclose,
    onreorder,
  }: {
    tabs: TableWorkspace[];
    activeId: string;
    busy: boolean;
    onactivate: (id: string) => void;
    onclose: (id: string) => void;
    onreorder: (ids: string[]) => void;
  } = $props();
  const visible = $derived(tabs.filter((tab) => tab.id !== 'empty'));
  let strip: HTMLDivElement;
  let drag = $state<{
    id: string;
    pointer: number;
    target: HTMLButtonElement;
    startX: number;
    x: number;
    y: number;
    offset: number;
    width: number;
    height: number;
    lifted: boolean;
    index: number;
    left: number;
    top: number;
    marker: number;
    inside: boolean;
  } | null>(null);
  let hold: ReturnType<typeof setTimeout> | undefined;
  let frame = 0;
  let suppressClick = false;
  let announcement = $state('');
  function wheelScroll(node: HTMLDivElement) {
    const wheel = (event: WheelEvent) => {
      if (event.ctrlKey || node.scrollWidth <= node.clientWidth) return;
      const unit = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? node.clientWidth : 1;
      const delta =
        (Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY) * unit;
      const before = node.scrollLeft;
      node.scrollLeft += delta;
      if (node.scrollLeft !== before) event.preventDefault();
    };
    node.addEventListener('wheel', wheel, { passive: false });
    return { destroy: () => node.removeEventListener('wheel', wheel) };
  }
  const draggedTab = $derived(visible.find((tab) => tab.id === drag?.id));

  function position() {
    if (!drag?.lifted) return;
    const bounds = strip.getBoundingClientRect();
    const others = [...strip.querySelectorAll<HTMLElement>('.table-tab')].filter(
      (element) => element.dataset.tabId !== drag!.id,
    );
    drag.inside =
      drag.x >= bounds.left &&
      drag.x <= bounds.right &&
      drag.y >= bounds.top - 12 &&
      drag.y <= bounds.bottom + 12;
    drag.index = others.findIndex((element) => {
      const rect = element.getBoundingClientRect();
      return drag!.x < rect.left + rect.width / 2;
    });
    if (drag.index === -1) drag.index = others.length;
    const next = others[drag.index]?.getBoundingClientRect();
    const last = others.at(-1)?.getBoundingClientRect();
    drag.marker = Math.max(
      bounds.left + 1,
      Math.min(bounds.right - 2, next?.left ?? last?.right ?? bounds.left),
    );
    drag.left = Math.max(bounds.left, Math.min(bounds.right - drag.width, drag.x - drag.offset));
    drag.top = bounds.top - 3;
  }

  function scrollFrame() {
    if (!drag?.lifted) return;
    const bounds = strip.getBoundingClientRect();
    if (drag.inside) {
      const edge = Math.min(40, bounds.width / 4);
      const distance =
        drag.x < bounds.left + edge
          ? drag.x - bounds.left - edge
          : drag.x > bounds.right - edge
            ? drag.x - bounds.right + edge
            : 0;
      if (distance) strip.scrollLeft += distance / 5;
    }
    position();
    frame = requestAnimationFrame(scrollFrame);
  }

  function lift() {
    if (!drag || busy || visible.length < 2) return;
    drag.lifted = true;
    clearTimeout(hold);
    position();
    frame = requestAnimationFrame(scrollFrame);
  }

  function start(event: PointerEvent, id: string) {
    suppressClick = false;
    if (event.button !== 0 || !event.isPrimary || busy || visible.length < 2) return;
    finish(false);
    const target = event.currentTarget as HTMLButtonElement;
    const rect = target.parentElement!.getBoundingClientRect();
    drag = {
      id,
      target,
      pointer: event.pointerId,
      startX: event.clientX,
      x: event.clientX,
      y: event.clientY,
      offset: event.clientX - rect.left,
      width: rect.width,
      height: rect.height,
      lifted: false,
      index: 0,
      left: rect.left,
      top: rect.top,
      marker: rect.left,
      inside: true,
    };
    target.setPointerCapture(event.pointerId);
    hold = setTimeout(lift, 160);
  }

  function move(event: PointerEvent) {
    if (!drag || drag.pointer !== event.pointerId) return;
    drag.x = event.clientX;
    drag.y = event.clientY;
    if (!drag.lifted && Math.abs(drag.x - drag.startX) >= 6) lift();
    position();
  }

  async function focusTab(id: string) {
    await tick();
    const element = [...strip.querySelectorAll<HTMLElement>('.table-tab')].find(
      (tab) => tab.dataset.tabId === id,
    );
    element?.querySelector<HTMLButtonElement>('[role="tab"]')?.focus({ preventScroll: true });
    element?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  }

  function finish(commit: boolean) {
    clearTimeout(hold);
    cancelAnimationFrame(frame);
    const previous = drag;
    drag = null;
    if (!previous) return;
    if (previous.target.hasPointerCapture(previous.pointer))
      previous.target.releasePointerCapture(previous.pointer);
    if (!previous.lifted) return;
    suppressClick = true;
    if (commit && previous.inside && !busy) {
      const ids = visible.filter((tab) => tab.id !== previous.id).map((tab) => tab.id);
      ids.splice(previous.index, 0, previous.id);
      onreorder(ids);
      announcement = `Tab moved to position ${previous.index + 1} of ${ids.length}`;
    }
    void focusTab(previous.id);
  }

  function keyboard(event: KeyboardEvent, id: string) {
    if (!drag && ['Enter', ' '].includes(event.key)) suppressClick = false;
    if (busy || !event.altKey || !['ArrowLeft', 'ArrowRight'].includes(event.key)) return;
    event.preventDefault();
    const ids = visible.map((tab) => tab.id);
    const current = ids.indexOf(id);
    const next = Math.max(
      0,
      Math.min(ids.length - 1, current + (event.key === 'ArrowLeft' ? -1 : 1)),
    );
    ids.splice(current, 1);
    ids.splice(next, 0, id);
    onreorder(ids);
    announcement = `Tab moved to position ${next + 1} of ${ids.length}`;
    void focusTab(id);
  }

  $effect(() => {
    if (busy && drag) finish(false);
  });
  onDestroy(() => {
    clearTimeout(hold);
    cancelAnimationFrame(frame);
  });
</script>

<svelte:window
  onblur={() => finish(false)}
  onkeydown={(event) => {
    if (event.key === 'Escape' && drag) {
      event.preventDefault();
      finish(false);
    }
  }}
/>

<div class="table-tabbar" role="tablist" aria-label="Open tables" bind:this={strip} use:wheelScroll>
  {#each visible as item (item.id)}
    <div
      class="table-tab"
      class:active={item.id === activeId}
      class:tab-drag-origin={drag?.lifted && drag.id === item.id}
      data-tab-id={item.id}
    >
      <button
        role="tab"
        aria-selected={item.id === activeId}
        disabled={busy}
        title="Click to open · drag to reorder · Alt+←/→ move tab"
        onpointerdown={(event) => start(event, item.id)}
        onpointermove={move}
        onpointerup={() => finish(true)}
        onpointercancel={() => finish(false)}
        onlostpointercapture={() => finish(false)}
        onkeydown={(event) => keyboard(event, item.id)}
        onclick={() => {
          if (!suppressClick) onactivate(item.id);
        }}
      >
        <Table2 size={14} />{item.selectedName}
        {#if item.schema && item.schema !== 'public'}<small class="muted">
            · {item.schema}</small
          >{/if}
        {#if pendingRows(item) || item.uncertain}<span
            class="dirty-dot"
            aria-label="Unsaved changes"
          ></span>{/if}
      </button>
      <button
        class="close-tab"
        aria-label={`Close ${item.selectedName} tab`}
        onclick={() => onclose(item.id)}
        disabled={busy}><X size={13} /></button
      >
    </div>
  {/each}
  {#if !visible.length}<span class="muted small">Select a table from the sidebar to open it.</span
    >{/if}
</div>
{#if drag?.lifted && draggedTab}
  <div
    class="tab-drag-ghost"
    aria-hidden="true"
    style:left={`${drag.left}px`}
    style:top={`${drag.top}px`}
    style:width={`${drag.width}px`}
    style:height={`${drag.height}px`}
  >
    <Table2 size={14} />{draggedTab.selectedName}
    {#if draggedTab.schema !== 'public'}<small class="muted"> · {draggedTab.schema}</small>{/if}
    {#if pendingRows(draggedTab) || draggedTab.uncertain}<span class="dirty-dot"></span>{/if}
  </div>
  {#if drag.inside}<div
      class="tab-drop-marker"
      aria-hidden="true"
      style:left={`${drag.marker}px`}
      style:top={`${drag.top + 3}px`}
      style:height={`${drag.height}px`}
    ></div>{/if}
{/if}
{#if announcement}<span class="tab-order-announcement" role="status">{announcement}</span>{/if}
