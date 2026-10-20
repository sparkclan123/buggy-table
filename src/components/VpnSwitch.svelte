<script lang="ts">
  let {
    enabled,
    disabled = false,
    label = 'VPN',
    accessibleLabel,
    ontoggle,
  }: {
    enabled: boolean;
    disabled?: boolean;
    label?: string;
    accessibleLabel: string;
    ontoggle: () => void;
  } = $props();
</script>

<button
  type="button"
  class="vpn-switch"
  role="switch"
  aria-label={accessibleLabel}
  aria-checked={enabled}
  title={enabled ? 'VPN enabled' : 'VPN off · device network'}
  {disabled}
  onclick={ontoggle}
>
  <span class="vpn-switch-label">{label}</span>
  <span class="vpn-switch-track" aria-hidden="true"><span class="vpn-switch-thumb"></span></span>
</button>
