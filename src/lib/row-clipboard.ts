import type { DraftRow } from './workspace';

// Keep typed rows in memory, never in persisted settings. The clipboard token
// (or exact text on platforms that strip custom types) prevents stale pastes.
const mime = 'application/x-buggy-table-rows';
let copied: { scope: string; token: string; text: string; rows: DraftRow[] } | null = null;
export function copyRows(
  event: ClipboardEvent,
  scope: string,
  rows: DraftRow[],
  columns: string[],
) {
  if (!event.clipboardData || !rows.length || rows.length > 100) return false;
  const text = rows
    .map((row) =>
      columns
        .map((c) => {
          const value = Object.hasOwn(row, c) ? (row[c] ?? 'NULL') : 'DEFAULT';
          return /[\t\r\n"]/.test(value) ? `"${value.replaceAll('"', '""')}"` : value;
        })
        .join('\t'),
    )
    .join('\n');
  if (new TextEncoder().encode(text).length > 4 * 1024 * 1024) return false;
  copied = { scope, token: crypto.randomUUID(), text, rows };
  event.clipboardData.setData('text/plain', text);
  event.clipboardData.setData(mime, copied.token);
  event.preventDefault();
  return true;
}
export function pastedRows(event: ClipboardEvent, scope: string): DraftRow[] | null {
  if (!copied || !event.clipboardData || copied.scope !== scope) return null;
  const token = event.clipboardData.getData(mime);
  if (token ? token !== copied.token : event.clipboardData.getData('text/plain') !== copied.text)
    return null;
  event.preventDefault();
  return copied.rows.map((r) => ({ ...r }));
}
