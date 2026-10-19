<script lang="ts">
  import { onMount } from 'svelte';
  import { Channel, invoke } from '@tauri-apps/api/core';
  import { desktopAvailable, errorMessage } from '../lib/api';
  import { Settings2 } from '@lucide/svelte';

  let { blocked, installing = $bindable(false) }: { blocked: boolean; installing?: boolean } =
    $props();
  type Status = {
    configured: boolean;
    currentVersion: string;
    version: string | null;
    notes: string | null;
  };
  type Progress = { stage: 'downloading' | 'installing'; downloaded: number; total: number | null };
  const preferenceKey = 'table-por.auto-updates';
  let automatic = $state(true);
  let configured = $state(false);
  let open = $state(false);
  let checking = $state(false);
  let version = $state<string | null>(null);
  let notes = $state('');
  let message = $state('');
  let failed = $state(false);
  let progress = $state<Progress | null>(null);

  async function openDownloads() {
    try {
      await invoke('open_download_page');
    } catch (error) {
      failed = true;
      message = errorMessage(error);
      open = true;
    }
  }

  function preferenceChanged() {
    try {
      localStorage.setItem(preferenceKey, String(automatic));
    } catch {
      /* Session preference still applies. */
    }
  }

  async function check(manual = false) {
    if (checking || installing || !configured) return;
    checking = true;
    failed = false;
    message = '';
    version = null;
    try {
      const result = await invoke<Status>('check_update', { fetch: true });
      version = result.version;
      notes = result.notes ?? '';
      if (manual) message = version ? '' : 'You have the latest version.';
    } catch (error) {
      if (manual) {
        failed = true;
        message = errorMessage(error);
      }
    } finally {
      checking = false;
    }
  }

  async function install() {
    if (blocked || installing || !version) return;
    installing = true;
    failed = false;
    message = '';
    progress = { stage: 'downloading', downloaded: 0, total: null };
    const channel = new Channel<Progress>();
    channel.onmessage = (event) => {
      progress = event;
    };
    try {
      await invoke('install_update', { progress: channel });
      // A successful native installation restarts the app; keep the workspace locked until then.
      message = 'Update installed. Restarting…';
    } catch (error) {
      failed = true;
      message = errorMessage(error);
      progress = null;
      installing = false;
    }
  }

  onMount(() => {
    try {
      automatic = localStorage.getItem(preferenceKey) !== 'false';
    } catch {
      /* Default: enabled for signed releases. */
    }
    if (!desktopAvailable) return;
    let disposed = false;
    void invoke<Status>('check_update', { fetch: false })
      .then((status) => {
        if (disposed) return;
        configured = status?.configured ?? false;
        if (automatic && configured) void check();
      })
      .catch(() => {
        message = 'Update settings are unavailable.';
      });
    const timer = setInterval(
      () => {
        if (automatic) void check();
      },
      6 * 60 * 60 * 1000,
    );
    return () => {
      disposed = true;
      clearInterval(timer);
    };
  });
</script>

{#if desktopAvailable}
  <div class="update-control">
    {#if open}
      <section class="update-panel" aria-label="Application updates">
        <div class="update-heading">
          <strong>buggy-table · v{__APP_VERSION__}</strong><button
            class="button"
            aria-label="Close update menu"
            disabled={installing}
            onclick={() => (open = false)}>×</button
          >
        </div>
        {#if !configured}
          <p>Preview releases can be downloaded from GitHub and installed manually.</p>
        {:else}
          <label
            ><input
              type="checkbox"
              bind:checked={automatic}
              onchange={preferenceChanged}
              disabled={installing}
            /> Check automatically</label
          >
          <p class="muted">
            Checks contact GitHub. Database contents and credentials are never included.
          </p>
          {#if version}
            <strong>Version {version} is available</strong>
            {#if notes}<p class="release-notes">{notes}</p>{/if}
            <p>The app will download the signed package, install it and restart.</p>
            {#if blocked}<p>
                Save or discard pending edits and finish database operations first.
              </p>{/if}
            <button
              class="button primary"
              disabled={blocked || installing || checking}
              onclick={install}>Update & restart</button
            >
          {/if}
          {#if !installing}<button class="button" disabled={checking} onclick={() => check(true)}
              >{checking ? 'Checking…' : 'Check for updates'}</button
            >{/if}
        {/if}
        {#if progress}
          <p role="status">
            {progress.stage === 'installing'
              ? 'Installing verified update…'
              : `Downloading… ${(progress.downloaded / 1024 / 1024).toFixed(1)} MB`}
          </p>
          <progress
            aria-label="Update download"
            max={progress.total ?? undefined}
            value={progress.total ? progress.downloaded : undefined}
          ></progress>
        {/if}
        {#if message}<p role={failed ? 'alert' : 'status'}>{message}</p>{/if}
      </section>
    {/if}
    <button
      class="update-toggle"
      class:available={!!version}
      title="Open GitHub downloads in your browser"
      onclick={openDownloads}
      disabled={installing}
    >
      {installing ? 'Updating…' : version ? `Update v${version}` : 'Updates'}
    </button>
    {#if configured}
      <button
        class="icon-button"
        aria-label="Update options"
        title="Update options"
        aria-expanded={open}
        onclick={() => (open = !open)}
        disabled={installing}
      >
        <Settings2 size={12} />
      </button>
    {/if}
  </div>
{/if}
