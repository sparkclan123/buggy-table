import { expect, test } from 'vitest';
import { temporalKind, temporalParts, temporalValue } from './temporal';
import { numeric, parseRedisInfo } from './stats';

test('date and time picker preserves wall clock, microseconds and offset', () => {
  for (const [type, value] of [
    ['timestamptz', '2026-10-19 23:59:01.123456+07:00'],
    ['timestamp', '2026-10-19 01:02:03.000001'],
    ['date', '2026-10-19'],
    ['timetz', '12:30:45.123-05:30'],
    ['time', '00:00:00'],
  ])
    expect(temporalValue(type, temporalParts(value))).toBe(value);
  expect(temporalKind('jsonb')).toBeNull();
});

test('missing statistics are unavailable, never invented as zero', () => {
  for (const value of [null, undefined, '', 'NaN', '-1']) expect(numeric(value)).toBeNull();
  expect(numeric('0')).toBe(0);
  const fields = parseRedisInfo('# Memory\r\nused_memory:4096\r\npath:a:b\r\n');
  expect(fields.used_memory).toBe('4096');
  expect(fields.path).toBe('a:b');
  expect(() => parseRedisInfo('# No metrics')).toThrow('unavailable');
  expect(() => parseRedisInfo({})).toThrow('text snapshot');
  expect(Object.keys(parseRedisInfo('# Keyspace\r\n'))).toEqual([]);
});
