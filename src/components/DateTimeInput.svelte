<script lang="ts">
  import { CalendarClock } from '@lucide/svelte';
  import { temporalKind, temporalParts, temporalValue } from '../lib/temporal';
  let {
    value,
    type,
    label,
    disabled = false,
    autofocus = false,
    cell = false,
    maxlength = 4096,
    onchange,
    onblur,
    oncommit,
    onrevert,
  }: {
    value: string;
    type: string;
    label: string;
    disabled?: boolean;
    autofocus?: boolean;
    cell?: boolean;
    maxlength?: number;
    onchange: (value: string) => void;
    onblur?: (event: FocusEvent) => void;
    oncommit?: () => void;
    onrevert?: () => void;
  } = $props();
  const kind = $derived(temporalKind(type));
  let dialog = $state<HTMLDialogElement>(null!);
  let parts = $state(temporalParts(''));
  let pickerError = $state('');
  function focus(node: HTMLInputElement) {
    if (autofocus) {
      node.focus();
      node.select();
    }
  }
  function open() {
    parts = temporalParts(value);
    pickerError = '';
    dialog.showModal();
  }
  function apply() {
    if (![...dialog.querySelectorAll('input')].every((input) => input.reportValidity())) return;
    if ((kind !== 'time' && !parts.date) || (kind !== 'date' && !parts.time)) {
      pickerError = 'Choose a date and time before applying.';
      return;
    }
    onchange(temporalValue(type, parts));
    dialog.close();
  }
</script>

<div class="temporal-input" class:in-cell={cell}>
  <input
    type="text"
    class:cell-input={cell}
    aria-label={label}
    {value}
    {disabled}
    {maxlength}
    spellcheck="false"
    use:focus
    placeholder={kind ? 'Type a value or choose date/time' : 'Value'}
    oninput={(event) => onchange(event.currentTarget.value)}
    {onblur}
    onkeydown={(event) => {
      if (event.isComposing) return;
      if (event.key === 'Escape' && onrevert) {
        event.preventDefault();
        event.stopPropagation();
        onrevert();
      }
      if (event.key === 'Enter' && oncommit) {
        event.preventDefault();
        oncommit();
      }
    }}
  />
  {#if kind}
    <button
      type="button"
      class="icon-button date-picker-button"
      aria-label={`Choose date/time for ${label}`}
      title="Choose date and time"
      onpointerdown={(event) => event.preventDefault()}
      {disabled}
      onclick={open}><CalendarClock size={14} /></button
    >
    <dialog
      bind:this={dialog}
      class="date-time-dialog"
      aria-label={`Date and time for ${label}`}
      onkeydown={(event) => {
        if (event.key === 'Enter' && event.target instanceof HTMLInputElement) {
          event.preventDefault();
          apply();
        }
      }}
    >
      <h3>Choose {kind === 'date' ? 'date' : kind === 'time' ? 'time' : 'date and time'}</h3>
      {#if kind !== 'time'}<label
          >Date<input
            type="date"
            aria-label="Calendar date"
            required
            bind:value={parts.date}
          /></label
        >{/if}
      {#if kind !== 'date'}
        <label
          >Time<input
            type="time"
            aria-label="Clock time"
            step="1"
            required
            bind:value={parts.time}
          /></label
        >
        <label
          >Fractional seconds<input
            type="text"
            aria-label="Fractional seconds"
            maxlength="6"
            pattern={'[0-9]{0,6}'}
            bind:value={parts.fraction}
            placeholder="Optional, up to 6 digits"
          /></label
        >
      {/if}
      {#if ['timestamptz', 'timetz'].includes(type)}
        <label
          >UTC offset<input
            type="text"
            aria-label="UTC offset"
            required
            pattern={'Z|[+\\-]([01][0-9]|2[0-3])(:?[0-5][0-9])?(:?[0-5][0-9])?'}
            bind:value={parts.offset}
          /></label
        >
        <p class="muted small">
          The displayed offset is preserved. No conversion to your computer's time zone.
        </p>
      {/if}
      {#if pickerError}<p role="alert">{pickerError}</p>{/if}
      <div class="date-time-actions">
        <button type="button" class="secondary-button" onclick={() => dialog.close()}>Cancel</button
        >
        <button type="button" class="primary-button" onclick={apply}>Apply date and time</button>
      </div>
    </dialog>
  {/if}
</div>

<style>
  .temporal-input {
    display: flex;
    min-width: 0;
    flex: 1;
    align-items: center;
  }
  .temporal-input > input {
    min-width: 0;
    width: 100%;
  }
  .date-picker-button {
    flex-shrink: 0;
  }
  .in-cell {
    height: 27px;
  }
  .date-time-dialog {
    width: 340px;
    padding: 20px;
    color: var(--ink);
    background: var(--canvas);
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  .date-time-dialog::backdrop {
    background: var(--backdrop);
  }
  .date-time-dialog label {
    display: grid;
    gap: 5px;
    margin-top: 12px;
    font-size: 12px;
  }
  .date-time-dialog h3 {
    margin: 0 0 12px;
  }
  .date-time-dialog input {
    width: 100%;
  }
  .date-time-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
  }
</style>
