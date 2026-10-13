import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { collect, manifest, platforms } from './release-assets.mjs';

test('release manifest covers every architecture and points to unique signed packages', () => {
  const root = mkdtempSync(join(tmpdir(), 'table-por-release-'));
  try {
    const output = join(root, 'output');
    for (const platform of platforms) {
      const bundle = join(root, platform);
      mkdirSync(bundle);
      const file = platform.startsWith('darwin') ? 'buggy-table.app.tar.gz' : 'buggy-table.exe';
      writeFileSync(join(bundle, file), platform);
      writeFileSync(join(bundle, `${file}.sig`), 'c2lnbmF0dXJl');
      if (platform.startsWith('darwin')) writeFileSync(join(bundle, 'buggy-table.dmg'), platform);
      collect('0.1.0', platform, bundle, output);
    }
    const result = manifest('0.1.0', output, 'Notes');
    assert.deepEqual(Object.keys(result.platforms), platforms);
    assert.equal(new Set(Object.values(result.platforms).map((part) => part.url)).size, 3);
    assert.equal(readFileSync(join(output, 'SHA256SUMS.txt'), 'utf8').trim().split('\n').length, 5);
    assert.throws(() => manifest('0.2.0', output), /do not match/);
    rmSync(join(output, 'windows-x86_64.json'));
    assert.throws(() => manifest('0.1.0', output), /ENOENT/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
