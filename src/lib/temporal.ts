export function temporalKind(type = ''): 'date' | 'time' | 'datetime' | null {
  if (type === 'date') return 'date';
  if (['time', 'timetz'].includes(type)) return 'time';
  if (['timestamp', 'timestamptz', 'datetime'].includes(type)) return 'datetime';
  return null;
}

export function temporalParts(value: string) {
  // Work with the displayed wall clock and offset. Date() would silently convert time zones.
  const match = value.match(
    /^(?:(\d{4}-\d{2}-\d{2})(?:[ T]|$))?(?:(\d{2}:\d{2})(?::(\d{2}))?(?:\.(\d{1,6}))?)?(Z|[+-]\d{2}(?::?\d{2})?(?::?\d{2})?)?$/,
  );
  return {
    date: match?.[1] ?? '',
    time: match?.[2] ? `${match[2]}:${match[3] ?? '00'}` : '00:00:00',
    fraction: match?.[4] ?? '',
    offset: match?.[5] ?? '+00:00',
  };
}

export function temporalValue(type: string, parts: ReturnType<typeof temporalParts>) {
  const kind = temporalKind(type);
  const time = parts.time.length === 5 ? `${parts.time}:00` : parts.time;
  return (
    (kind === 'date' ? parts.date : kind === 'datetime' ? `${parts.date} ${time}` : time) +
    (kind !== 'date' && parts.fraction ? `.${parts.fraction}` : '') +
    (['timestamptz', 'timetz'].includes(type) ? parts.offset : '')
  );
}
