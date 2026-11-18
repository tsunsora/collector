// Builds the release app and packages it as a portable app: a folder you can
// put anywhere and run, and a zip of it.
//
//   npm run portable        the folder (for this PC) and the zip
//   npm run portable:dist   the zip only, for giving to other people
//
//   portable/Collector/Collector.exe
//   portable/Collector/README.txt
//   portable/Collector/data/     everything the app writes goes here
//   portable/Collector_<version>_x64-portable.zip
//
// The folder is refreshed in place and its `data` is never touched, so a copy
// in use keeps its settings. The zip is made from a clean copy and never
// contains anyone's data. The build itself is scrubbed of this PC's folder
// names (see `build`), so the exe can be handed on.
import { execFileSync, execSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const zipOnly = process.argv.includes('--dist');
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const { version, productName } = JSON.parse(readFileSync(join(root, 'src-tauri', 'tauri.conf.json'), 'utf8'));
const exe = `${productName}.exe`;
const built = join(root, 'src-tauri', 'target', 'release', 'collector.exe');

// Rust writes the full path of every source file into the exe, for its error
// messages. Those paths hold this PC's user name and project location, so
// they are rewritten to neutral names. The separator is what Cargo expects
// between flags in CARGO_ENCODED_RUSTFLAGS; it copes with spaces in paths.
function build() {
  const remap = [
    `--remap-path-prefix=${process.env.CARGO_HOME ?? join(homedir(), '.cargo')}=cargo`,
    `--remap-path-prefix=${process.env.RUSTUP_HOME ?? join(homedir(), '.rustup')}=rustup`,
    `--remap-path-prefix=${homedir()}=home`,
    `--remap-path-prefix=${root}=app`,
  ];
  execSync('npx tauri build --no-bundle', {
    cwd: root,
    stdio: 'inherit',
    env: { ...process.env, CARGO_ENCODED_RUSTFLAGS: remap.join('\x1f') },
  });
}

const dataNote = [
  `${productName} keeps its settings, list and activity in this folder.`,
  `While the folder exists next to ${exe}, nothing is written elsewhere`,
  'on the PC, with one exception: your sign-in is kept in Windows',
  'Credential Manager, not in a file. On another PC you sign in again.',
  '',
  'Delete the contents of this folder to start fresh. If you delete the folder',
  `itself, ${productName} uses your Windows profile (AppData) instead.`,
  '',
  `If you move the ${productName} folder while "Open when I sign in to Windows"`,
  'is on, turn that setting off and on again so Windows has the new location.',
  '',
].join('\r\n');

const readme = [
  `${productName} ${version}`,
  '',
  'A small tray app for Windows.',
  '',
  'STARTING',
  `Run ${exe}. A short tour walks you through setting it up.`,
  'Windows may say the app is from an unknown publisher, because it is not',
  'code-signed. Choose "More info", then "Run anyway".',
  'Needs Windows 10 or 11 with the WebView2 runtime. Windows 11 includes it.',
  '',
  'WHERE THINGS ARE KEPT',
  'Everything the app writes is in the "data" folder beside it, except your',
  'sign-in, which Windows Credential Manager keeps. Nothing is installed. To',
  'remove the app, sign out in Settings, quit it from its tray icon and delete',
  'this folder.',
  '',
  'UPDATES',
  'The app checks for a newer version once a day and installs it when you quit.',
  '',
].join('\r\n');

/**
 * Lay the app out in `dir`. Leaves an existing data folder as it is. Returns
 * false if the app there is running, in which case it could not be replaced.
 */
function layOut(dir) {
  mkdirSync(join(dir, 'data'), { recursive: true });
  writeFileSync(join(dir, 'README.txt'), readme);
  const note = join(dir, 'data', 'readme.txt');
  if (!existsSync(note)) writeFileSync(note, dataNote);
  try {
    copyFileSync(built, join(dir, exe));
  } catch (e) {
    if (e.code === 'EBUSY' || e.code === 'EPERM') return false;
    throw e;
  }
  return true;
}

build();

const out = join(root, 'portable');
const folder = join(out, productName);
const refreshed = zipOnly ? null : layOut(folder);

// The zip comes from a clean copy, so it never carries the data of a folder in use.
const stage = join(out, '.stage');
rmSync(stage, { recursive: true, force: true });
layOut(join(stage, productName));
const zip = join(out, `${productName.replaceAll(' ', '')}_${version}_x64-portable.zip`);
rmSync(zip, { force: true });
// Windows' own tar writes a standard zip; PowerShell's Compress-Archive
// writes backslashes in paths, which other unzip tools complain about.
const tar = join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'tar.exe');
execFileSync(tar, ['-a', '-c', '-f', zip, '-C', stage, productName], { stdio: 'inherit' });
rmSync(stage, { recursive: true, force: true });

const mb = (path) => (statSync(path).size / 1024 / 1024).toFixed(1) + ' MB';
if (refreshed === true) console.log(`Portable folder: ${folder} (${exe} ${mb(join(folder, exe))})`);
if (refreshed === false) {
  console.log(`Portable folder: NOT UPDATED. ${productName} is running from it. Quit it from its tray icon and run this again.`);
}
console.log(`Portable zip:    ${zip} (${mb(zip)})`);
