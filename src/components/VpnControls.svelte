<script lang="ts">
  import { Settings2, RotateCw } from '@lucide/svelte';
  import VpnSwitch from './VpnSwitch.svelte';
  import type { VpnStatus } from '../lib/types';
  let {
    enabled,
    disabled = false,
    label = 'connection',
    status = null,
    ontoggle,
    onsettings,
    onrefresh,
  }: {
    enabled: boolean;
    disabled?: boolean;
    label?: string;
    status?: VpnStatus | null;
    ontoggle: () => void;
    onsettings: () => void;
    onrefresh?: () => void;
  } = $props();
</script>

<div class="vpn-controls" aria-label={`VPN for ${label}`}>
  <VpnSwitch {enabled} {disabled} accessibleLabel={`Use VPN for ${label}`} {ontoggle} />
  {#if status && enabled}<span
      class="vpn-dot"
      class:healthy={status.state === 'ready'}
      class:failed={status.state === 'disconnected' || status.state === 'unreachable'}
      title={status.message}
      aria-label={`VPN ${status.state}`}
    ></span>{/if}
  {#if onrefresh && enabled}<button
      type="button"
      class="icon-button vpn-icon"
      aria-label={`Reconnect VPN for ${label}`}
      title="Reconnect VPN and database"
      {disabled}
      onclick={onrefresh}><RotateCw size={13} /></button
    >{/if}
  <button
    type="button"
    class="icon-button vpn-icon"
    aria-label={`VPN settings for ${label}`}
    title="VPN settings"
    {disabled}
    onclick={onsettings}><Settings2 size={13} /></button
  >
</div>
