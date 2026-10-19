const key = 'buggy-table.column-layouts.v1';

export function readColumnOrder(identity: string): string[] {
  try {
    const all = JSON.parse(localStorage.getItem(key) ?? '{}');
    const order = all[identity];
    return Array.isArray(order)
      ? order.filter((v): v is string => typeof v === 'string').slice(0, 5000)
      : [];
  } catch {
    return [];
  }
}

export function saveColumnOrder(identity: string, order: string[]) {
  try {
    const old = JSON.parse(localStorage.getItem(key) ?? '{}');
    const entries = Object.entries(old && typeof old === 'object' && !Array.isArray(old) ? old : {})
      .filter(([name]) => name !== identity)
      .slice(-199);
    localStorage.setItem(key, JSON.stringify(Object.fromEntries([...entries, [identity, order]])));
    return true;
  } catch {
    return false;
  }
}
