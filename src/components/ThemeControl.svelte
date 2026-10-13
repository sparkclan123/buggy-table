<script lang="ts">
  import { getTheme, setTheme, type ThemePreference } from '../lib/theme';
  let theme = $state(getTheme());
</script>

<label class="theme-control"
  >Appearance
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
