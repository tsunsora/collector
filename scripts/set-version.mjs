// Sets the app's version in the three places it is written: `npm run set-version 0.5.1`.
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version ?? '')) {
  console.error('Give a version as three numbers, for example: npm run set-version 0.5.1');
  process.exit(1);
}

const edit = (file, change) => {
  const path = join(root, file);
  const before = readFileSync(path, 'utf8');
  const after = change(before);
  if (after === before && !before.includes(version)) {
    console.error(`Could not find the version in ${file}.`);
    process.exit(1);
  }
  writeFileSync(path, after);
};

edit('package.json', (s) => s.replace(/("version":\s*")[^"]+(")/, `$1${version}$2`));
edit('src-tauri/tauri.conf.json', (s) => s.replace(/("version":\s*")[^"]+(")/, `$1${version}$2`));
// Only the package's own version: the first one in the file.
edit('src-tauri/Cargo.toml', (s) => s.replace(/^(version\s*=\s*")[^"]+(")/m, `$1${version}$2`));
console.log(`Version set to ${version}. Add a "## ${version} (date)" heading to CHANGELOG.md.`);
