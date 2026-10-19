import { expect, test } from 'vitest';
import { inspectorValue } from './row-inspector';

test('JSON formatting preserves numeric lexemes, key order, duplicates, escapes and strings', () => {
  const source =
    '{"z":900719925474099312345,"10":1.23000,"2":1e999,"z":-0,"s":"a, } [ \\u0041 \\" b","a":[true,null,{},[]]}';
  const formatted = inspectorValue(source, 'jsonb');
  expect(formatted).toBe(
    '{\n  "z": 900719925474099312345,\n  "10": 1.23000,\n  "2": 1e999,\n  "z": -0,\n  "s": "a, } [ \\u0041 \\" b",\n  "a": [\n    true,\n    null,\n    {},\n    []\n  ]\n}',
  );
  expect(inspectorValue(source, 'JSON')).toBe(formatted);
  expect(inspectorValue(source, 'pg_catalog.json')).toBe(formatted);
  expect(inspectorValue('"a string"', 'jsonb')).toBe('"a string"');
  expect(inspectorValue('12345678901234567890.000001', 'json')).toBe('12345678901234567890.000001');
});

test('database values remain exact; NULL, empty text and JSON null are distinct', () => {
  for (const [value, type] of [
    ['2026-10-19 23:59:01.123456+07:00', 'timestamptz'],
    ['99999999999999999999.000100', 'numeric'],
    ['{1,2,NULL}', 'int8[]'],
    ['{"b":1,"a":2}', 'text'],
    ['false', 'bool'],
    ['  multiline\n text  ', 'varchar'],
    ['{"unfinished":', 'jsonb'],
    ['<script>alert("unsafe")</script>', 'text'],
  ])
    expect(inspectorValue(value, type)).toBe(value);
  expect(inspectorValue(null, 'jsonb')).toBe('NULL');
  expect(inspectorValue('', 'text')).toBe('""');
  expect(inspectorValue('null', 'jsonb')).toBe('null');
});

test('deep JSON preserves all data without unbounded indentation expansion', () => {
  const source = '['.repeat(1000) + '42' + ']'.repeat(1000);
  const formatted = inspectorValue(source, 'jsonb');
  expect(formatted.replace(/\s/g, '')).toBe(source);
  expect(formatted.length).toBeLessThan(source.length * 70);
});
