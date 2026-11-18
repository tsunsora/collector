// Creates the key that signs updates. Run once, by hand: `npm run release:key`.
//
// The private key goes in your user folder, outside the project, and you
// choose its password when asked. (Press Enter for none: then the file alone
// is the key, and `npm run release` signs without asking.) The public half is written into
// src-tauri/update.json, which is built into the app: an installed copy only
// accepts an update whose signature that public key verifies.
//
// Lose the private key and no copy already out there can be updated again.
// Let someone else have it and they can ship an update as you. Back it up
// somewhere that is not this PC, and never commit it.
import { execSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
export const keyPath = process.env.COLLECTOR_SIGNING_KEY ?? join(homedir(), '.collector-signing', 'update.key');
const configPath = join(root, 'src-tauri', 'update.json');

function savePublicKey() {
  const config = JSON.parse(readFileSync(configPath, 'utf8'));
  config.publicKey = readFileSync(`${keyPath}.pub`, 'utf8').trim();
  writeFileSync(configPath, JSON.stringify(config, null, 2) + '\n');
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  if (existsSync(keyPath)) {
    savePublicKey();
    console.log(`A signing key already exists at ${keyPath}. It was left alone.`);
    console.log('Its public half is in src-tauri/update.json.');
    process.exit(0);
  }
  mkdirSync(dirname(keyPath), { recursive: true });
  console.log('Choose a password for the signing key. You will type it each time you make a release.\n');
  execSync(`npx tauri signer generate -w "${keyPath}"`, { cwd: root, stdio: 'inherit' });
  savePublicKey();
  console.log(`\nPrivate key: ${keyPath}`);
  console.log('Public key:  written to src-tauri/update.json');
  console.log('\nBack up the private key somewhere that is not this PC. Never commit it.');
}
