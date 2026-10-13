<script lang="ts">
  import { onMount } from 'svelte';
  import { Database, LockKeyhole, X, ArrowRight, ShieldCheck } from '@lucide/svelte';
  import type { ConnectRequest, ConnectionInfo, TlsMode } from '../lib/types';
  import { api, errorMessage } from '../lib/api';
  let {
    onconnected,
    onclose,
  }: { onconnected: (connection: ConnectionInfo) => void; onclose: () => void } = $props();
  let name = $state('Local PostgreSQL');
  let host = $state('127.0.0.1');
  let port = $state(5432);
  let database = $state('');
  let username = $state('');
  let password = $state('');
  let schema = $state('public');
  let tls = $state<TlsMode>('verifyFull');
  let manualEdits = $state(true);
  let busy = $state(false);
  let error = $state('');
  let dialog: HTMLDialogElement;
  onMount(() => dialog.showModal());

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy) return;
    busy = true;
    error = '';
    const request: ConnectRequest = {
      name,
      host,
      port,
      database,
      username,
      password,
      schema,
      tls,
      manualEdits,
    };
    password = '';
    try {
      onconnected(await api.connect(request));
    } catch (e) {
      error = errorMessage(e);
    } finally {
      request.password = '';
      busy = false;
    }
  }
</script>

<dialog
  bind:this={dialog}
  class="connection-dialog"
  aria-labelledby="connection-title"
  oncancel={(event) => {
    event.preventDefault();
    if (!busy) onclose();
  }}
>
  <div class="dialog-heading">
    <div class="icon-box"><Database size={23} /></div>
    <button
      class="icon-button"
      aria-label="Close connection dialog"
      onclick={onclose}
      disabled={busy}><X size={20} /></button
    >
  </div>
  <h2 id="connection-title">Connect your database</h2>
  <p class="muted">A direct connection. Your credentials stay in this app session.</p>
  <div class="engine-choice">
    <Database size={19} /><strong>PostgreSQL</strong><span class="badge">Direct connection</span>
  </div>
  <form onsubmit={submit} autocomplete="off">
    <label
      >Connection name<input
        bind:value={name}
        required
        maxlength="253"
        placeholder="My development database"
      /></label
    >
    <div class="form-row host-row">
      <label>Host<input bind:value={host} required maxlength="253" spellcheck="false" /></label
      ><label>Port<input type="number" min="1" max="65535" bind:value={port} required /></label>
    </div>
    <div class="form-row">
      <label
        >Database<input
          bind:value={database}
          required
          maxlength="253"
          placeholder="my_database"
          spellcheck="false"
        /></label
      ><label>Schema<input bind:value={schema} required maxlength="253" spellcheck="false" /></label
      >
    </div>
    <div class="form-row">
      <label
        >Username<input
          bind:value={username}
          required
          maxlength="253"
          placeholder="database_user"
          autocomplete="off"
          spellcheck="false"
        /></label
      ><label
        >Password<input
          type="password"
          bind:value={password}
          maxlength="4096"
          autocomplete="new-password"
        /></label
      >
    </div>
    <label
      >Transport security<select bind:value={tls}
        ><option value="verifyFull">TLS · verify certificate and hostname</option><option
          value="localOnly">Local only · unencrypted loopback IP</option
        ></select
      ></label
    >
    <label class="checkbox-label"
      ><input type="checkbox" bind:checked={manualEdits} />Allow manual cell edits</label
    >
    <div class="info-note">
      <ShieldCheck size={18} /><span
        >Save applies only the cell edits you make, using your database permissions. SQL stays
        read-only. Turn off manual edits to require a restricted reader account.</span
      >
    </div>
    {#if error}<div class="error-banner" role="alert">{error}</div>{/if}
    <div class="dialog-footer">
      <span class="muted small"><LockKeyhole size={13} /> Not saved to disk</span><button
        class="primary-button"
        type="submit"
        disabled={busy}
        >{busy ? 'Checking access…' : 'Connect securely'}<ArrowRight size={16} /></button
      >
    </div>
  </form>
</dialog>
