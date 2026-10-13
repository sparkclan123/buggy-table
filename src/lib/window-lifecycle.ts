import { getCurrentWindow } from '@tauri-apps/api/window';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { desktopAvailable } from './api';

// Keep native close behavior in one place; no shell or file permissions needed.
export async function guardWindowClose(
  hasPendingWork: () => boolean,
  notify: () => void,
): Promise<UnlistenFn> {
  if (!desktopAvailable) return () => {};
  return getCurrentWindow().onCloseRequested((event) => {
    if (hasPendingWork()) {
      event.preventDefault();
      notify();
    }
  });
}
