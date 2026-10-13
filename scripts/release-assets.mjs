// Give every architecture unique names and assemble one complete update manifest.
import { readdirSync, readFileSync, writeFileSync, mkdirSync, copyFileSync } from 'node:fs';
import { join, basename } from 'node:path';
import { createHash } from 'node:crypto';
import { pathToFileURL } from 'node:url';
export const platforms = ['darwin-aarch64', 'darwin-x86_64', 'windows-x86_64'];
export const repository = 'sparkclan123/buggy-table-releases';
export function walk(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory() ? walk(join(directory, entry.name)) : [join(directory, entry.name)],
  );
}
export function collect(version, platform, bundle, output) {
  if (!platforms.includes(platform)) throw new Error('Unsupported platform');
  const files = walk(bundle);
  const extension = platform.startsWith('darwin') ? '.app.tar.gz' : '.exe';
  const signed = files.filter((file) => file.endsWith(`${extension}.sig`));
  if (signed.length !== 1) throw new Error(`Expected exactly one signed ${extension} artifact`);
  const source = signed[0].slice(0, -4);
  const filename = `Table-Por_${version}_${platform}${extension}`;
  const signature = readFileSync(signed[0], 'utf8').trim();
  if (!signature || !/^[A-Za-z0-9+/=]+$/.test(signature))
    throw new Error('Invalid updater signature');
  mkdirSync(output, { recursive: true });
  copyFileSync(source, join(output, filename));
  copyFileSync(signed[0], join(output, `${filename}.sig`));
  if (platform.startsWith('darwin')) {
    const installers = files.filter((file) => file.endsWith('.dmg'));
    if (installers.length !== 1) throw new Error('Expected exactly one DMG installer');
    copyFileSync(installers[0], join(output, `Table-Por_${version}_${platform}.dmg`));
  }
  const entry = {
    version,
    platform,
    signature,
    url: `https://github.com/${repository}/releases/download/v${version}/${filename}`,
  };
  writeFileSync(join(output, `${platform}.json`), JSON.stringify(entry, null, 2));
}
export function manifest(version, directory, notes = '') {
  const entries = platforms.map((platform) =>
    JSON.parse(readFileSync(join(directory, `${platform}.json`), 'utf8')),
  );
  const result = { version, notes, pub_date: new Date().toISOString(), platforms: {} };
  for (const [index, entry] of entries.entries()) {
    const expected = platforms[index];
    if (
      entry.version !== version ||
      entry.platform !== expected ||
      !entry.signature ||
      !entry.url.startsWith(`https://github.com/${repository}/releases/download/v${version}/`)
    ) {
      throw new Error('Release parts do not match this version/platform/repository');
    }
    const file = join(directory, basename(new URL(entry.url).pathname));
    readFileSync(file); // Fail if a manifest references a missing package.
    if (readFileSync(`${file}.sig`, 'utf8').trim() !== entry.signature)
      throw new Error('Signature mismatch');
    result.platforms[expected] = { signature: entry.signature, url: entry.url };
  }
  writeFileSync(join(directory, 'latest.json'), JSON.stringify(result, null, 2) + '\n');
  const checksums = readdirSync(directory)
    .filter((file) => /\.(dmg|exe|gz)$/.test(file))
    .sort()
    .map(
      (file) =>
        `${createHash('sha256')
          .update(readFileSync(join(directory, file)))
          .digest('hex')}  ${file}`,
    )
    .join('\n');
  writeFileSync(join(directory, 'SHA256SUMS.txt'), `${checksums}\n`);
  return result;
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const version = JSON.parse(readFileSync('package.json', 'utf8')).version;
  if (!/^\d+\.\d+\.\d+$/.test(version))
    throw new Error('Stable release requires a numeric x.y.z version');
  const [command, ...args] = process.argv.slice(2);
  if (command === 'collect') collect(version, ...args);
  else if (command === 'manifest')
    manifest(version, args[0], readFileSync('docs/RELEASE_NOTES.md', 'utf8'));
  else
    throw new Error(
      'Usage: release-assets.mjs collect <platform> <bundle> <output> | manifest <directory>',
    );
}
