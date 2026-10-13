// Run once. Only the public verification key is copied into the repository.
import { existsSync, mkdirSync, readFileSync, writeFileSync, chmodSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';
const directory = join(homedir(), '.config', 'table-por', 'signing');
const key = join(directory, 'updater.key');
mkdirSync(directory, { recursive: true, mode: 0o700 });
if (!existsSync(key)) {
  const cli = join('node_modules', '@tauri-apps', 'cli', 'tauri.js');
  execFileSync(process.execPath, [cli, 'signer', 'generate', '--ci', '-w', key], {
    stdio: 'ignore',
  });
  chmodSync(key, 0o600);
}
const pubkey = readFileSync(`${key}.pub`, 'utf8').trim();
const configPath = 'src-tauri/tauri.release.conf.json';
if (existsSync(configPath)) {
  const existing = JSON.parse(readFileSync(configPath, 'utf8')).plugins?.updater?.pubkey;
  if (existing && existing !== pubkey)
    throw new Error(
      'Key differs from release verification key. Restore the original private key; do not rotate it silently.',
    );
}
writeFileSync(
  configPath,
  JSON.stringify(
    {
      bundle: { createUpdaterArtifacts: true },
      plugins: { updater: { pubkey } },
    },
    null,
    2,
  ) + '\n',
);
console.log(`Updater key ready at ${key}. Back it up securely; do not commit it.`);
console.log('Only its public key was written to src-tauri/tauri.release.conf.json.');
