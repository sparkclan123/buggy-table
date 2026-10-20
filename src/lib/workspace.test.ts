import { describe, expect, it } from 'vitest';
import {
  appendRows,
  beginCellEdit,
  cellValue,
  changeCell,
  changedRows,
  clearHistory,
  createWorkspace,
  endCellEdit,
  pendingRows,
  setDeletedRows,
  undoDraft,
} from './workspace';
const make = () => {
  const w = createWorkspace();
  w.result = {
    columns: ['id', 'note'],
    rows: [['1', null]],
    elapsedMs: 0,
    truncated: false,
    sql: '',
  };
  return w;
};
describe('unsaved draft history', () => {
  it('groups typing by cell activation, drops reverted/no-op edits and preserves NULL', () => {
    const w = make();
    beginCellEdit(w, 0, 'note');
    changeCell(w, 0, 'note', 'a');
    changeCell(w, 0, 'note', 'ab');
    endCellEdit(w);
    expect(w.undo).toHaveLength(1);
    beginCellEdit(w, 0, 'note');
    changeCell(w, 0, 'note', 'c');
    changeCell(w, 0, 'note', 'ab');
    endCellEdit(w);
    expect(w.undo).toHaveLength(1);
    undoDraft(w);
    expect(cellValue(w, 0, 'note')).toBe(null);
    expect(pendingRows(w)).toBe(0);
    undoDraft(w, true);
    expect(cellValue(w, 0, 'note')).toBe('ab');
    expect(pendingRows(make())).toBe(0);
  });
  it('undoes active edits, deletions and appended batches in order, retaining defaults', () => {
    const w = make();
    appendRows(w, [{ note: 'new' }, { note: null }]);
    beginCellEdit(w, 1, 'id');
    changeCell(w, 1, 'id', '2');
    undoDraft(w);
    expect(cellValue(w, 1, 'id')).toBeUndefined();
    setDeletedRows(w, [0, 2]);
    expect(changedRows(w.result, w.edits, w.deletedRows, w.insertedRows)).toEqual([
      { delete: true, original: { id: '1', note: null }, changes: {} },
      { insert: true, original: {}, changes: { note: 'new' } },
    ]);
    undoDraft(w);
    expect(w.deletedRows).toEqual([]);
    undoDraft(w);
    expect(w.insertedRows).toEqual([]);
    undoDraft(w, true);
    expect(w.insertedRows).toEqual([{ note: 'new' }, { note: null }]);
    clearHistory(w);
    expect(w.undo).toEqual([]);
    expect(w.redo).toEqual([]);
  });
  it('bounds history, branches after new input, and blocks uncertain saves', () => {
    const w = make();
    for (let i = 0; i < 110; i++) {
      beginCellEdit(w, 0, 'note');
      changeCell(w, 0, 'note', String(i));
      endCellEdit(w);
    }
    expect(w.undo).toHaveLength(100);
    undoDraft(w);
    expect(w.redo).toHaveLength(1);
    beginCellEdit(w, 0, 'note');
    changeCell(w, 0, 'note', 'branch');
    endCellEdit(w);
    expect(w.redo).toEqual([]);
    w.uncertain = true;
    undoDraft(w);
    expect(cellValue(w, 0, 'note')).toBe('branch');
  });
});

it('edits SQL columns named like JavaScript object properties without confusing defaults', () => {
  const w = make();
  w.result!.columns = ['__proto__', 'constructor'];
  w.result!.rows = [['1', 'original']];
  beginCellEdit(w, 0, 'constructor');
  changeCell(w, 0, 'constructor', 'changed');
  endCellEdit(w);
  beginCellEdit(w, 0, '__proto__');
  changeCell(w, 0, '__proto__', '2');
  endCellEdit(w);
  expect(cellValue(w, 0, '__proto__')).toBe('2');
  undoDraft(w);
  expect(cellValue(w, 0, '__proto__')).toBe('1');
  appendRows(w, [{}]);
  expect(cellValue(w, 1, 'constructor')).toBeUndefined();
  beginCellEdit(w, 1, '__proto__');
  changeCell(w, 1, '__proto__', '3');
  endCellEdit(w);
  expect(cellValue(w, 1, '__proto__')).toBe('3');
  undoDraft(w);
  expect(cellValue(w, 1, '__proto__')).toBeUndefined();
});
