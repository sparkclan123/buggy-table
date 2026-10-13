import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
export type ThemePreference = 'system' | 'light' | 'dark';
const key = 'table-por.theme';
const media = window.matchMedia('(prefers-color-scheme: dark)');
let preference: ThemePreference = 'system';
try {
  const stored = localStorage.getItem(key);
  if (stored === 'light' || stored === 'dark') preference = stored;
} catch {
  /* System theme still works when storage is unavailable. */
}

function apply() {
  if (isTauri())
    void getCurrentWindow()
      .setTheme(preference === 'system' ? null : preference)
      .catch(() => {});
  document.documentElement.dataset.theme =
    preference === 'system' ? (media.matches ? 'dark' : 'light') : preference;
}
export function getTheme(): ThemePreference {
  return preference;
}
export function setTheme(value: ThemePreference) {
  preference = value;
  try {
    localStorage.setItem(key, value);
  } catch {
    /* Keep the session choice. */
  }
  apply();
}
export function initializeTheme() {
  apply();
  media.addEventListener('change', apply);
}
