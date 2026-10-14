<script lang="ts">
  import { Sun, Moon, Monitor } from '@lucide/svelte';
  import { getTheme, setTheme, type ThemePreference } from '../lib/theme';
  let theme = $state(getTheme());
</script>

<label class="theme-control" title={`Appearance: ${theme}`}>
  {#if theme === 'dark'}<Moon size={14} />{:else if theme === 'light'}<Sun
      size={14}
    />{:else}<Monitor size={14} />{/if}
  <select
    aria-label="Appearance"
    bind:value={theme}
    onchange={() => setTheme(theme as ThemePreference)}
  >
    <option value="system">System</option><option value="light">Light</option><option value="dark"
      >Dark</option
    >
  </select>
</label>
