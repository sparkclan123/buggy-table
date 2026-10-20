<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { Database, LockKeyhole, X, ArrowRight, ShieldCheck } from '@lucide/svelte';
  import type {
    ConnectRequest,
    ConnectedProfile,
    SavedConnection,
    TlsMode,
    Engine,
  } from '../lib/types';
  import { api, desktopAvailable, errorMessage } from '../lib/api';
  import VpnControls from './VpnControls.svelte';
  import VpnDialog from './VpnDialog.svelte';
  import type { VpnSelection } from '../lib/types';
  let {
    onconnected,
    onclose,
    initial = null,
  }: {
    onconnected: (result: ConnectedProfile) => void;
    onclose: () => void;
    initial?: SavedConnection | null;
  } = $props();
  let engine = $state<Engine>(untrack(() => initial?.engine ?? 'postgres'));
  const owner = untrack(() => initial?.id ?? crypto.randomUUID());
  let vpn = $state<VpnSelection>(
    untrack(() => initial?.vpn ?? { enabled: false, profileId: null }),
  );
  let showVpn = $state(false);
  let group = $state(untrack(() => initial?.group ?? ''));
  let remember = $state(true);
  let savePassword = $state(untrack(() => initial?.hasPassword ?? false));
  function changeEngine() {
    port = engine === 'postgres' ? 5432 : engine === 'mysql' ? 3306 : 6379;
    name = `Local ${engine === 'postgres' ? 'PostgreSQL' : engine === 'mysql' ? 'MySQL' : 'Redis'}`;
    database = engine === 'redis' ? '0' : '';
    schema = engine === 'postgres' ? 'public' : '';
    manualEdits = engine === 'postgres';
    if (tls === 'require') tls = 'verifyFull';
    rootCaPem = '';
    caName = '';
    password = '';
    error = '';
  }
  let name = $state(untrack(() => initial?.name ?? 'Local PostgreSQL'));
  let host = $state(untrack(() => initial?.host ?? '127.0.0.1'));
  let port = $state(untrack(() => initial?.port ?? 5432));
  let database = $state(untrack(() => initial?.database ?? ''));
  let username = $state(untrack(() => initial?.username ?? ''));
  let password = $state('');
  let schema = $state(untrack(() => initial?.schema ?? 'public'));
  let tls = $state<TlsMode>(untrack(() => initial?.tls ?? 'verifyFull'));
  let rootCaPem = $state(untrack(() => initial?.rootCaPem ?? ''));
  let caName = $state('');
  let readingCa = $state(false);
  let manualEdits = $state(untrack(() => initial?.manualEdits ?? true));
  let busy = $state(false);
  let error = $state('');
  let dialog: HTMLDialogElement;
  onMount(() => dialog.showModal());

  async function close() {
    if (busy) return;
    busy = true;
    try {
      if (!initial && desktopAvailable) await api.discardVpnDraft(owner);
      onclose();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }

  async function readCa(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    rootCaPem = '';
    caName = '';
    error = '';
    if (!file) return;
    readingCa = true;
    try {
      if (file.size > 64 * 1024) throw new Error('CA files must be at most 64 KiB.');
      const pem = await file.text();
      if (!pem.trim().startsWith('-----BEGIN CERTIFICATE-----') || pem.includes('PRIVATE KEY'))
        throw new Error('Choose a PEM CA certificate, not a private key.');
      rootCaPem = pem;
      caName = file.name;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Could not read the CA certificate file.';
      input.value = '';
    } finally {
      readingCa = false;
    }
  }

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy || readingCa) return;
    busy = true;
    error = '';
    const request: ConnectRequest = {
      engine,
      name,
      host,
      port,
      database,
      username,
      password,
      schema: engine === 'postgres' ? schema : database,
      tls,
      rootCaPem: engine !== 'redis' && tls === 'verifyFull' ? rootCaPem : '',
      manualEdits: engine === 'postgres' && manualEdits,
    };
    password = '';
    try {
      onconnected(
        await api.connectProfile(request, owner, group, remember, remember && savePassword, vpn),
      );
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
    void close();
  }}
>
  <div class="dialog-heading">
    <div class="icon-box"><Database size={23} /></div>
    <button class="icon-button" aria-label="Close connection dialog" onclick={close} disabled={busy}
      ><X size={20} /></button
    >
  </div>
  <h2 id="connection-title">{initial ? 'Reconnect database' : 'Connect your database'}</h2>
  <p class="muted">Passwords are session-only unless you choose system password storage.</p>
  <form onsubmit={submit} autocomplete="off">
    <div class="vpn-form-line">
      <span class="small muted">{vpn.enabled ? 'Connect through OpenVPN' : 'Device network'}</span
      ><VpnControls
        enabled={vpn.enabled}
        disabled={busy}
        label={name}
        ontoggle={() => {
          if (!vpn.profileId) showVpn = true;
          else vpn = { ...vpn, enabled: !vpn.enabled };
        }}
        onsettings={() => (showVpn = true)}
      />
    </div>
    {#if busy && vpn.enabled}
      <p class="muted small">
        First use may download the OpenVPN runtime. Later connections reuse it.
      </p>
    {/if}
    <label
      >Database engine<select
        bind:value={engine}
        onchange={changeEngine}
        disabled={busy || readingCa}
      >
        <option value="postgres">PostgreSQL</option><option value="mysql">MySQL</option><option
          value="redis">Redis</option
        >
      </select></label
    >
    <label
      >Connection name<input
        bind:value={name}
        required
        maxlength="253"
        placeholder="My development database"
      /></label
    >
    <label
      >Group (optional)<input
        bind:value={group}
        maxlength="100"
        placeholder="Project / environment"
      /></label
    >
    <div class="form-row host-row">
      <label>Host<input bind:value={host} required maxlength="253" spellcheck="false" /></label
      ><label>Port<input type="number" min="1" max="65535" bind:value={port} required /></label>
    </div>
    <div class="form-row">
      <label
        >{engine === 'redis'
          ? 'Database number'
          : engine === 'mysql'
            ? 'Default database (optional)'
            : 'Database'}<input
          bind:value={database}
          required={engine !== 'mysql'}
          maxlength="253"
          placeholder={engine === 'mysql' ? 'Choose after connecting' : 'my_database'}
          spellcheck="false"
        /></label
      >{#if engine === 'postgres'}<label
          >Schema<input bind:value={schema} required maxlength="253" spellcheck="false" /></label
        >{/if}
    </div>
    <div class="form-row">
      <label
        >Username<input
          bind:value={username}
          required={engine !== 'redis'}
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
      >Transport security<select bind:value={tls} disabled={busy || readingCa}
        ><option value="verifyFull">TLS · verify certificate and hostname</option>
        {#if engine !== 'redis'}<option value="require"
            >TLS · encrypted, certificate not verified</option
          >{/if}
        <option value="localOnly">Local only · unencrypted loopback IP</option></select
      ></label
    >
    {#if engine !== 'redis' && tls === 'require'}
      <p class="muted small" role="note">
        TLS is required, but the server's identity is not verified. Use only if you trust this
        endpoint. No plaintext fallback.
      </p>
    {:else if tls === 'localOnly'}
      <p class="muted small">
        Use 127.0.0.1 or ::1 for a local database or trusted tunnel. Remote IP addresses require
        TLS.
      </p>
    {/if}
    {#if engine !== 'redis' && tls === 'verifyFull'}
      <details>
        <summary class="small">Custom CA certificate (optional)</summary>
        <label
          >CA certificate (.pem / .crt)<input
            type="file"
            accept=".pem,.crt"
            onchange={readCa}
            disabled={busy || readingCa}
          /></label
        >
        <p class="muted small">
          {caName
            ? `${caName} · ${remember ? 'saved with connection' : 'session only'}`
            : 'For private CAs, choose the CA file from your database provider. Host must still match the server certificate.'}
        </p>
      </details>
    {/if}
    {#if engine === 'postgres'}<label class="checkbox-label"
        ><input type="checkbox" bind:checked={manualEdits} />Allow manual cell edits</label
      >
    {/if}
    <div class="info-note">
      <ShieldCheck size={18} /><span
        >{#if engine === 'postgres'}Save applies your cell edits, copied rows and staged deletions,
          using your database permissions. SQL stays read-only. Turn off manual edits to require a
          restricted reader account.{:else if engine === 'mysql'}MySQL 8.x · read-only in this app,
          including administrator accounts. Connect to the server, then expand a database to browse
          its tables.{:else}Redis · read-only commands. Use a restricted ACL user; the app does not
          change your server permissions. Standalone endpoints only.{/if}</span
      >
    </div>
    {#if error}<div class="error-banner" role="alert">{error}</div>{/if}
    <label class="checkbox-label"
      ><input type="checkbox" bind:checked={remember} disabled={!!initial} />Remember connection on
      this device</label
    >
    {#if remember}<label class="checkbox-label"
        ><input type="checkbox" bind:checked={savePassword} />Save password in system password store</label
      >{/if}
    <div class="dialog-footer">
      <span class="muted small"
        ><LockKeyhole size={13} /> {remember ? 'Settings saved locally' : 'Session only'}</span
      ><button class="primary-button" type="submit" disabled={busy || readingCa}
        >{busy
          ? vpn.enabled
            ? 'Starting VPN & database…'
            : 'Checking access…'
          : 'Connect securely'}<ArrowRight size={16} /></button
      >
    </div>
  </form>
</dialog>
{#if showVpn}<VpnDialog
    {owner}
    {name}
    selection={vpn}
    onapply={async (selection) => {
      vpn = selection;
    }}
    onclose={() => (showVpn = false)}
  />{/if}
