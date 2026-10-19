import { describe, expect, it } from 'vitest';
import { browseSample, sampleTables } from './sample';
import { quoteIdentifier, errorMessage } from './api';

describe('local sample explorer', () => {
  it('paginates and distinguishes null from an empty value', () => {
    expect(browseSample(sampleTables[0], [], 8).rows).toHaveLength(50);
    expect(browseSample(sampleTables[0], [], 8).truncated).toBe(false);
    expect(
      browseSample(sampleTables[1], [{ column: 'email', operator: 'isNull', value: '' }], 0).rows,
    ).toHaveLength(1);
    expect(
      browseSample(sampleTables[1], [{ column: 'email', operator: 'equals', value: '' }], 0).rows,
    ).toHaveLength(0);
  });
  it('uses literal filters instead of interpreting filter values as SQL', () => {
    expect(
      browseSample(
        sampleTables[0],
        [{ column: 'customer', operator: 'contains', value: "' OR 1=1 --" }],
        0,
      ).rows,
    ).toHaveLength(0);
  });
});

it('quotes identifiers and hides unstructured errors', () => {
  expect(quoteIdentifier('a"b')).toBe('"a""b"');
  expect(errorMessage('secret')).not.toContain('secret');
});
