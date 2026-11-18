# Releasing Collector

Releases are published on this repo, and installed copies update from its latest release. The repo is named in [src-tauri/update.json](src-tauri/update.json) as `releasesRepo`. That file is built into the app.

## Once: the signing key

Installed copies only accept an update signed with your key. Create it once, in your own terminal:

```bash
npm run release:key
```

It asks you to choose a password (press Enter for none), then writes:

| File | What it is |
| --- | --- |
| `%USERPROFILE%\.collector-signing\update.key` | The private key. It signs every release. |
| `%USERPROFILE%\.collector-signing\update.key.pub` | The public key. The script also writes it into `src-tauri/update.json`. |

**Keep `%USERPROFILE%\.collector-signing` backed up outside this PC.** If the private key is lost, no copy already out there can be updated again: they would all refuse a release signed with a new key. If someone else gets it, they can ship an update as you. Never commit it; `.gitignore` already refuses `*.key`.

A key with no password signs without asking, which is convenient, but then the file alone is the key: anyone who can read it can sign an update. A password is the second lock.

Until the key exists, builds say **Updates aren't set up in this build** and never look for one.

## Build the release files

```bash
npm run release
```

It stops with an error if the version has no heading in [CHANGELOG.md](CHANGELOG.md), if there is no signing key, or if `releasesRepo` is empty. Otherwise it builds the app and signs it, asking for the signing key's password if it has one. The files go into `release/`, which git ignores:

| File | What it is |
| --- | --- |
| `Collector-<version>-windows.zip` | The portable app: the folder with the exe, a README and an empty `data` folder. This is what a new person downloads. |
| `Collector-<version>-windows.exe` | The same executable on its own. Installed copies download this to update. |
| `Collector-<version>-windows.exe.sig` | Its signature. |
| `collector-update.json` | The update feed installed copies read. It lists the version, the first paragraph under this version's heading in the changelog as release notes, and the file's name, SHA-256, size and signature. |
| `SHA256SUMS.txt` | The SHA-256 of the zip and the exe. |
| `release-notes.md` | The changelog section, ready to be the release's description. |

The build rewrites the source-file paths Rust records in the exe, which would otherwise carry this PC's user name.

## Publish an update

Installed copies read the feed attached to the **latest** release of this repo, once a day while they're open and 20 seconds after they start. So:

1. Set the version: `npm run set-version 0.5.1`. It changes `package.json`, `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml` together.
2. Add a heading for it to [CHANGELOG.md](CHANGELOG.md): `## 0.5.1 (2026-11-20)`, one paragraph saying what the release is, then the details as a list. The paragraph is what people see next to **Version 0.5.1 is ready**.
3. Run `npm run release`.
4. Commit and push, so the tag lands on the commit the release was built from.
5. Publish it with the `gh release create` command the script prints at the end. It tags `v<version>` and attaches the five files.

Drafts and pre-releases aren't "latest", so copies don't see them until the release is published.

What people see:

- With **Update automatically** on (the default), the update downloads in the background and installs when they quit. **Settings › Updates › Restart and update** installs it right away. A strip saying the version is ready shows in the window and the tray view.
- With it off, the app says a version is available and offers **Download**.
- Either way the app swaps its own exe for the new one. The `data` folder, the channel list and the sign-in are untouched.

Each download is checked twice before it's used: its SHA-256 against the feed, and its signature against the public key built into the copy doing the updating. A file that fails either is thrown away and the app says so.

The feed has to be readable without signing in, so the repo has to stay public for updates to work. If it ever goes private, publish releases somewhere public instead and point `releasesRepo` at that. File names in the feed are resolved next to the feed, so the files must be attached to the same release.

## Version

Three numbers, `major.minor.patch`. Every new version has to compare higher than the last one, or installed copies won't offer it. `npm run set-version` is the only place to change it; the app shows it under **Settings › Updates**.

## Trying an update without publishing

Demo mode can read a feed from anywhere, so the whole path can be run against a folder on this PC:

```bash
COLLECTOR_DEMO=1 COLLECTOR_UPDATE_FEED=http://127.0.0.1:8765/collector-update.json COLLECTOR_UPDATE_KEY=<public key> "Collector.exe"
```

Demo mode shows made-up channels and never goes online. Outside demo mode the feed address and the key are only ever the ones built in.

## Just the portable app

```bash
npm run portable:dist
```

builds only `portable/Collector_<version>_x64-portable.zip`, for handing to someone directly. `npm run portable` also lays out a working folder beside it.
