import { readFileSync, writeFileSync } from 'node:fs';
const files = ['package.json', 'src-tauri/tauri.conf.json'];
const version = process.argv[2];
const cargoPath = 'src-tauri/Cargo.toml';
const cargo = readFileSync(cargoPath, 'utf8');
if (version === '--check') {
  const versions = files.map((file) => JSON.parse(readFileSync(file, 'utf8')).version);
  versions.push(cargo.match(/^version = "([^"]+)"/m)[1]);
  if (new Set(versions).size !== 1) throw new Error('Package, Tauri and Cargo versions must match');
  if (process.env.RELEASE_TAG && process.env.RELEASE_TAG !== `v${versions[0]}`)
    throw new Error('Tag does not match the application version');
  console.log(`Version ${versions[0]} matches all packages.`);
} else {
  if (!/^\d+\.\d+\.\d+$/.test(version ?? ''))
    throw new Error('Usage: node scripts/version.mjs <x.y.z> | --check');
  for (const file of files) {
    const data = JSON.parse(readFileSync(file, 'utf8'));
    data.version = version;
    writeFileSync(file, JSON.stringify(data, null, 2) + '\n');
  }
  writeFileSync(cargoPath, cargo.replace(/^version = "[^"]+"/m, `version = "${version}"`));
  const lock = 'src-tauri/Cargo.lock';
  writeFileSync(
    lock,
    readFileSync(lock, 'utf8').replace(/(name = "buggy-table"\nversion = ")[^"]+/, `$1${version}`),
  );
  console.log(
    `Set version ${version}. Update docs/RELEASE_NOTES.md, validate, commit and tag v${version}.`,
  );
}
