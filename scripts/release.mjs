// Builds everything a release needs into release/: `npm run release`.
// It does not publish. It ends by printing the command that would.
//
//   Collector-<version>-windows.zip   the portable app, for new people
//   Collector-<version>-windows.exe   the same executable, for updates
//   Collector-<version>-windows.exe.sig
//   collector-update.json            the feed installed copies read
//   SHA256SUMS.txt
//   release-notes.md
//
// The feed has the shape Lock On's has: the version, the first paragraph
// under this version's heading in CHANGELOG.md as notes, and for the file its
// name, SHA-256 and size, plus the signature installed copies check.
// See RELEASING.md.
import { execSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { keyPath } from './release-key.mjs';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const { version, productName } = JSON.parse(readFileSync(join(root, 'src-tauri', 'tauri.conf.json'), 'utf8'));
const { releasesRepo, publicKey } = JSON.parse(readFileSync(join(root, 'src-tauri', 'update.json'), 'utf8'));
// What the release is called in public: its title, its notes and its file names.
const publicName = 'Collector';
const stem = `${publicName}-${version}-windows`;

const stop = (why) => {
  console.error(`\nCan't make a release: ${why}`);
  process.exit(1);
};

// ---- Before building: everything a release cannot do without.
if (!releasesRepo) stop('src-tauri/update.json has no releasesRepo (the public repo releases are uploaded to).');
if (!publicKey || !existsSync(keyPath)) stop('there is no signing key yet. Run `npm run release:key` first.');

/** The part of CHANGELOG.md under this version's heading. */
function changelogSection() {
  const lines = readFileSync(join(root, 'CHANGELOG.md'), 'utf8').split(/\r?\n/);
  const start = lines.findIndex((line) => line.startsWith(`## ${version}`));
  if (start < 0) return null;
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((line) => line.startsWith('## '));
  return (end < 0 ? rest : rest.slice(0, end)).join('\n').trim();
}
const section = changelogSection();
if (!section) stop(`CHANGELOG.md has no "## ${version}" heading. Add one saying what changed.`);
// The first paragraph is what an installed copy shows next to "Version x is ready".
const notes = section.split(/\n\s*\n/)[0].replace(/\s+/g, ' ').trim();

// ---- Build (scrubbed of this PC's paths) and gather.
execSync('node scripts/portable.mjs --dist', { cwd: root, stdio: 'inherit' });

const out = join(root, 'release');
rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
const zip = `${stem}.zip`;
const exe = `${stem}.exe`;
copyFileSync(join(root, 'portable', `${productName.replaceAll(' ', '')}_${version}_x64-portable.zip`), join(out, zip));
copyFileSync(join(root, 'src-tauri', 'target', 'release', 'collector.exe'), join(out, exe));

// ---- Sign. A key made without a password signs straight away; otherwise the
// signing tool asks for the password.
const sign = `npx tauri signer sign -f "${keyPath}" "${join(out, exe)}"`;
try {
  execSync(`${sign} -p ""`, { cwd: root, stdio: 'pipe' });
  console.log('\nSigned the update.');
} catch {
  console.log('\nSigning the update. Type the signing key password when asked.');
  execSync(sign, { cwd: root, stdio: 'inherit' });
}
const signature = readFileSync(join(out, `${exe}.sig`), 'utf8').trim();

const sha256 = (name) => createHash('sha256').update(readFileSync(join(out, name))).digest('hex');
const feed = {
  version,
  notes,
  windows: { file: exe, sha256: sha256(exe), size: statSync(join(out, exe)).size, signature },
};
writeFileSync(join(out, 'collector-update.json'), JSON.stringify(feed, null, 2) + '\n');
writeFileSync(join(out, 'SHA256SUMS.txt'), [zip, exe].map((name) => `${sha256(name)}  ${name}`).join('\n') + '\n');
writeFileSync(join(out, 'release-notes.md'), `# ${publicName} ${version}\n\n## What's new\n\n${section}\n`);

const files = [zip, exe, `${exe}.sig`, 'collector-update.json', 'SHA256SUMS.txt'];
console.log(`\nRelease files for ${version} are in release/:`);
for (const name of files) console.log(`  ${name}`);
console.log('\nNothing has been published. To publish, run:\n');
console.log(
  `gh release create v${version} --repo ${releasesRepo} --title "${publicName} ${version}" --notes-file release/release-notes.md ` +
    files.map((name) => `release/${name}`).join(' '),
);
console.log('\nInstalled copies see it within a day, or at once with Settings > Updates > Check now.');
