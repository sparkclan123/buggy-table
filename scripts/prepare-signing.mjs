// Private credentials stay in environment variables, never in generated config.
import { writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
function required(name) {
  if (!process.env[name]) throw new Error(`Missing release secret/variable: ${name}`);
  return process.env[name];
}
required('TAURI_SIGNING_PRIVATE_KEY');
const bundle = {};
if (process.platform === 'darwin') {
  for (const name of [
    'APPLE_CERTIFICATE',
    'APPLE_CERTIFICATE_PASSWORD',
    'APPLE_SIGNING_IDENTITY',
    'APPLE_ID',
    'APPLE_PASSWORD',
    'APPLE_TEAM_ID',
  ])
    required(name);
  bundle.macOS = { signingIdentity: required('APPLE_SIGNING_IDENTITY') };
} else if (process.platform === 'win32') {
  for (const name of [
    'AZURE_CLIENT_ID',
    'AZURE_CLIENT_SECRET',
    'AZURE_TENANT_ID',
    'AZURE_SIGNING_ENDPOINT',
    'AZURE_SIGNING_ACCOUNT',
    'AZURE_SIGNING_PROFILE',
  ])
    required(name);
  bundle.windows = {
    signCommand: {
      cmd: 'pwsh',
      args: ['-NoProfile', '-File', resolve('scripts/sign-windows.ps1'), '%1'],
    },
  };
} else {
  throw new Error('Release signing supports macOS and Windows runners only');
}
writeFileSync('src-tauri/tauri.signing.conf.json', JSON.stringify({ bundle }, null, 2));
