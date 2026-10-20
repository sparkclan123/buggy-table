<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { Plus, Pencil, Trash2, X, Shield, RotateCw, Activity } from '@lucide/svelte';
  import { api, errorMessage } from '../lib/api';
  import type {
    VpnSelection,
    VpnProfile,
    VpnStatus,
    VpnRuntimeInfo,
    SavedConnection,
    VpnEdit,
  } from '../lib/types';
  let {
    owner,
    name,
    selection,
    status = null,
    online = false,
    onapply,
    onclose,
    ontest,
    onrefresh,
    onprofileupdate,
  }: {
    owner: string;
    name: string;
    selection: VpnSelection;
    status?: VpnStatus | null;
    online?: boolean;
    onapply: (selection: VpnSelection) => Promise<void>;
    onclose: () => void;
    ontest?: () => Promise<VpnStatus | null>;
    onrefresh?: () => Promise<void>;
    onprofileupdate?: (profile: SavedConnection) => void;
  } = $props();
  let enabled = $state(untrack(() => selection.enabled));
  let defaultId = $state(untrack(() => selection.profileId));
  let profiles = $state<VpnProfile[]>([]);
  let runtime = $state<VpnRuntimeInfo | null>(null);
  let busy = $state(true);
  let message = $state('Loading VPN settings…');
  let error = $state('');
  let editing = $state<VpnProfile | null>(null);
  let editor = $state(false);
  let profileName = $state('');
  let username = $state('');
  let password = $state('');
  let keyPassword = $state('');
  let clearPassword = $state(false);
  let clearKeyPassword = $state(false);
  let dns = $state('');
  let config = $state<string | null>(null);
  let fileName = $state('');
  let removeId = $state('');
  let dialog: HTMLDialogElement;
  const activeProfile = $derived(online && selection.enabled ? selection.profileId : null);
  onMount(() => {
    dialog.showModal();
    void Promise.all([api.vpnProfiles(owner), api.vpnRuntime()])
      .then(([saved, info]) => {
        profiles = saved;
        runtime = info;
      })
      .catch((e) => {
        error = errorMessage(e);
      })
      .finally(() => {
        busy = false;
        message = '';
      });
  });
  function edit(profile: VpnProfile | null) {
    editing = profile;
    editor = true;
    error = '';
    config = null;
    fileName = '';
    profileName = profile?.name ?? '';
    username = profile?.username ?? '';
    dns = profile?.dns.join(', ') ?? '';
    password = '';
    keyPassword = '';
    clearPassword = false;
    clearKeyPassword = false;
  }
  function closeEditor() {
    editor = false;
    config = null;
    password = '';
    keyPassword = '';
  }
  async function choose(event: Event) {
    const input = event.currentTarget as HTMLInputElement,
      file = input.files?.[0];
    if (!file) return;
    busy = true;
    error = '';
    config = null;
    fileName = '';
    try {
      if (file.size > 512 * 1024) throw new Error('OpenVPN profiles must be at most 512 KiB.');
      config = await file.text();
      fileName = file.name;
      if (!profileName) profileName = file.name.replace(/\.ovpn$/i, '').slice(0, 80);
    } catch (e) {
      error = e instanceof Error ? e.message : 'Could not read this file.';
      input.value = '';
    } finally {
      busy = false;
    }
  }
  async function save(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    error = '';
    const edit: VpnEdit = {
      id: editing?.id ?? null,
      owner,
      name: profileName,
      config,
      username,
      password: clearPassword ? '' : password || null,
      keyPassword: clearKeyPassword ? '' : keyPassword || null,
      dns: dns.split(/[\s,]+/).filter(Boolean),
    };
    try {
      const saved = await api.saveVpnProfile(edit);
      profiles = [...profiles.filter((p) => p.id !== saved.id), saved];
      if (!defaultId) defaultId = saved.id;
      closeEditor();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      edit.config = null;
      edit.password = null;
      edit.keyPassword = null;
      password = '';
      keyPassword = '';
      busy = false;
    }
  }
  async function remove(id: string) {
    busy = true;
    error = '';
    try {
      const saved = await api.removeVpnProfile(owner, id);
      profiles = profiles.filter((p) => p.id !== id);
      if (defaultId === id) {
        defaultId = null;
        enabled = false;
      }
      if (saved) onprofileupdate?.(saved);
      removeId = '';
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }
  async function prepare() {
    busy = true;
    error = '';
    message = 'Preparing OpenVPN… First setup downloads the runtime; later connections reuse it.';
    try {
      await api.prepareVpn();
      runtime = await api.vpnRuntime();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
      message = '';
    }
  }
  async function apply() {
    busy = true;
    error = '';
    message = online ? 'Reconnecting database through the selected network…' : '';
    try {
      await onapply({ enabled, profileId: defaultId });
      onclose();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
      message = '';
    }
  }
  async function testRoute() {
    busy = true;
    error = '';
    try {
      await ontest?.();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }
  async function refresh() {
    busy = true;
    error = '';
    message = 'Reconnecting VPN and database…';
    try {
      await onrefresh?.();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
      message = '';
    }
  }
</script>

<dialog
  bind:this={dialog}
  class="connection-dialog vpn-dialog"
  aria-label={`VPN settings for ${name}`}
  oncancel={(e) => {
    e.preventDefault();
    if (!busy) onclose();
  }}
>
  <div class="manager-heading">
    <Shield size={18} />
    <h2>VPN <span class="muted">· {name}</span></h2>
    <button
      type="button"
      class="icon-button"
      aria-label="Close VPN settings"
      disabled={busy}
      onclick={onclose}><X size={18} /></button
    >
  </div>
  <p class="muted small">OpenVPN · isolated per connection · Docker Desktop</p>
  {#if status && online}
    <div class="vpn-health" role="status">
      <div>
        <strong
          >{status.state === 'ready'
            ? 'VPN connected'
            : status.state === 'unreachable'
              ? 'Database unreachable'
              : status.state === 'direct'
                ? 'Device network'
                : 'VPN disconnected'}</strong
        >{#if status.latencyMs !== null && status.state === 'ready'}<span
            >{status.latencyMs} ms · TCP route</span
          >{/if}
      </div>
      <p>{status.message}</p>
      {#if status.checkedAt}<small
          >Checked {new Date(status.checkedAt * 1000).toLocaleTimeString()}</small
        >{/if}
      {#if selection.enabled}<div class="vpn-health-actions">
          <button type="button" class="secondary-button" disabled={busy} onclick={testRoute}
            ><Activity size={13} />Test route</button
          ><button type="button" class="secondary-button" disabled={busy} onclick={refresh}
            ><RotateCw size={13} />Reconnect</button
          >
        </div>{/if}
    </div>
  {/if}
  {#if editor}
    <form onsubmit={save} autocomplete="off" class="vpn-editor">
      <label
        >Name<input
          bind:value={profileName}
          maxlength="80"
          required
          disabled={busy}
          placeholder="Office VPN"
        /></label
      >
      <label
        >{editing ? 'Replace profile (optional)' : 'OpenVPN profile'}<input
          type="file"
          accept=".ovpn,.conf"
          onchange={choose}
          disabled={busy}
          required={!editing && !config}
        /></label
      >
      <p class="muted small">
        {fileName ||
          'Use a .ovpn with inline CA, certificate and key. Scripts, TAP and browser SSO are not supported.'}
      </p>
      <div class="form-row">
        <label
          >VPN username<input
            bind:value={username}
            maxlength="253"
            disabled={busy}
            autocomplete="off"
          /></label
        ><label
          >VPN password<input
            type="password"
            bind:value={password}
            disabled={busy || clearPassword}
            autocomplete="new-password"
            placeholder={editing?.hasPassword ? 'Keep saved password' : 'Optional'}
          /></label
        >
      </div>
      <details>
        <summary class="small">DNS & key password</summary>
        <label
          >VPN DNS IPs<input
            bind:value={dns}
            disabled={busy}
            placeholder="Use DNS supplied by VPN"
          /></label
        >
        <p class="muted small">
          Optional, comma-separated. Database names resolve only through this VPN.
        </p>
        <label
          >Private key password<input
            type="password"
            bind:value={keyPassword}
            disabled={busy || clearKeyPassword}
            autocomplete="new-password"
            placeholder={editing?.hasKeyPassword ? 'Keep saved key password' : 'Optional'}
          /></label
        >
        {#if editing?.hasPassword}<label class="checkbox-label"
            ><input type="checkbox" bind:checked={clearPassword} disabled={busy} />Clear saved VPN
            password</label
          >{/if}
        {#if editing?.hasKeyPassword}<label class="checkbox-label"
            ><input type="checkbox" bind:checked={clearKeyPassword} disabled={busy} />Clear saved
            key password</label
          >{/if}
      </details>
      <p class="muted small">
        Secrets are encrypted on this device using a key in the system password store.
      </p>
      <div class="dialog-footer">
        <button type="button" class="secondary-button" disabled={busy} onclick={closeEditor}
          >Cancel</button
        ><button class="primary-button" disabled={busy || (!editing && !config)}>Save VPN</button>
      </div>
    </form>
  {:else}
    <div class="vpn-selection">
      <label class="checkbox-label"
        ><input type="checkbox" bind:checked={enabled} disabled={busy || !defaultId} />Use VPN for
        this database</label
      ><button type="button" class="secondary-button" disabled={busy} onclick={() => edit(null)}
        ><Plus size={13} />Add VPN</button
      >
    </div>
    <div class="vpn-profile-list" aria-label="VPN profiles">
      {#each profiles as profile (profile.id)}
        <div class="vpn-profile-row">
          <label
            ><input
              type="radio"
              name="vpn-default"
              checked={defaultId === profile.id}
              aria-label={`Default VPN ${profile.name}`}
              disabled={busy}
              onchange={() => (defaultId = profile.id)}
            /><span
              ><strong>{profile.name}</strong><small
                >{defaultId === profile.id ? 'Default' : 'OpenVPN'}{activeProfile === profile.id
                  ? ' · in use'
                  : ''}</small
              ></span
            ></label
          >
          <button
            type="button"
            class="icon-button"
            aria-label={`Edit VPN ${profile.name}`}
            title={activeProfile === profile.id ? 'Disconnect before editing' : 'Edit VPN'}
            disabled={busy || activeProfile === profile.id}
            onclick={() => edit(profile)}><Pencil size={14} /></button
          >
          <button
            type="button"
            class="icon-button"
            aria-label={`Remove VPN ${profile.name}`}
            title={activeProfile === profile.id ? 'Disconnect before removing' : 'Remove VPN'}
            disabled={busy || activeProfile === profile.id}
            onclick={() => (removeId = profile.id)}><Trash2 size={14} /></button
          >
        </div>
        {#if removeId === profile.id}<div class="manager-confirm">
            <span>Remove {profile.name}?</span><button
              type="button"
              class="secondary-button"
              disabled={busy}
              onclick={() => remove(profile.id)}>Remove</button
            ><button
              type="button"
              class="secondary-button"
              disabled={busy}
              onclick={() => (removeId = '')}>Keep</button
            >
          </div>{/if}
      {/each}
      {#if !profiles.length && !busy}<p class="muted small">
          Add a VPN, then choose it as the default for this database.
        </p>{/if}
    </div>
    {#if runtime && !runtime.prepared}<div class="vpn-runtime">
        <span class="small muted">{runtime.message}</span><button
          type="button"
          class="secondary-button"
          disabled={busy}
          onclick={prepare}>Prepare VPN</button
        >
      </div>{/if}
    <p class="muted small">
      {enabled
        ? 'Connect VPN first, then the database. No direct fallback if VPN fails.'
        : 'VPN is off. This database uses the device network.'}{online
        ? ' Applying a network change reconnects this database.'
        : ''}
    </p>
    <div class="dialog-footer">
      <button type="button" class="secondary-button" disabled={busy} onclick={onclose}
        >Cancel</button
      ><button
        type="button"
        class="primary-button"
        disabled={busy || (enabled && !defaultId)}
        onclick={apply}>Apply</button
      >
    </div>
  {/if}
  {#if busy && message}<p class="muted small" role="status">{message}</p>{/if}
  {#if error}<div class="error-banner" role="alert">{error}</div>{/if}
</dialog>
