const key = 'buggy-table.row-inspector.v1';

export type InspectorPreferences = { enabled: boolean; widthPercent: number };
export const inspectorWidth = (value: number) => Math.max(8, Math.min(60, value));

export function readInspectorPreferences(): InspectorPreferences {
  const defaults = { enabled: true, widthPercent: 10 };
  try {
    const value = JSON.parse(localStorage.getItem(key) ?? 'null');
    return {
      enabled: typeof value?.enabled === 'boolean' ? value.enabled : defaults.enabled,
      widthPercent:
        typeof value?.widthPercent === 'number' && Number.isFinite(value.widthPercent)
          ? inspectorWidth(value.widthPercent)
          : defaults.widthPercent,
    };
  } catch {
    return defaults;
  }
}

export function saveInspectorPreferences(value: InspectorPreferences) {
  try {
    // Persist display preferences only. Never store selected rows or their values.
    localStorage.setItem(
      key,
      JSON.stringify({ enabled: value.enabled, widthPercent: inspectorWidth(value.widthPercent) }),
    );
    return true;
  } catch {
    return false;
  }
}

export function inspectorValue(value: string | null, dataType?: string): string {
  if (value === null) return 'NULL';
  if (value === '') return '""';
  if (!/^(?:pg_catalog\.)?jsonb?$/i.test(dataType ?? '')) return value;
  try {
    // Validate syntax only; serializing the parsed object would round large numbers,
    // normalize decimals/escapes, reorder keys and discard duplicate keys.
    JSON.parse(value);
  } catch {
    return value; // An unsaved draft may not be valid JSON yet.
  }
  const tokens = value.match(/"(?:[^"\\]|\\[\s\S])*"|[{}\[\],:]|[^\s{}\[\],:]+/g) ?? [];
  const parts: string[] = [];
  let depth = 0;
  // Bound indentation cost for deeply nested documents; all tokens remain visible.
  const newline = () => parts.push('\n', '  '.repeat(Math.min(depth, 32)));
  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i];
    if (token === '{' || token === '[') {
      parts.push(token);
      depth++;
      if (tokens[i + 1] !== (token === '{' ? '}' : ']')) newline();
    } else if (token === '}' || token === ']') {
      depth--;
      if (tokens[i - 1] !== (token === '}' ? '{' : '[')) newline();
      parts.push(token);
    } else if (token === ',') {
      parts.push(token);
      newline();
    } else parts.push(token === ':' ? ': ' : token);
  }
  return parts.join('');
}
