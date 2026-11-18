<div align="center">

<img src="src-tauri/icons/128x128@2x.png" width="112" alt="App icon">

# Collector

### A small tray app for Windows

It sits in the tray and works through a list you give it.<br>
Nothing installed, no account with us, no cloud.

[![Latest release](https://img.shields.io/github/v/release/tsunsora/collector?style=for-the-badge&label=Release&labelColor=1c1b19&color=9146ff)](../../releases/latest)
[![Downloads](https://img.shields.io/github/downloads/tsunsora/collector/total?style=for-the-badge&label=Downloads&labelColor=1c1b19&color=aaa59d)](../../releases)
[![Platform](https://img.shields.io/badge/Windows%2010%20%C2%B7%2011-aaa59d?style=for-the-badge&label=Platform&labelColor=1c1b19)](#-install)

**[Download](#-install)** · **[Changelog](CHANGELOG.md)** · **[Report an issue](../../issues)**

</div>

---

## 📥 Install

1. Download the `.zip` from the [latest release](../../releases/latest).
2. Unzip it anywhere and run the `.exe` inside. A short tour walks you through setting it up.
3. Windows may say the app is from an unknown publisher, because it isn't code-signed. Choose **More info**, then **Run anyway**.

It needs Windows 10 or 11 with the WebView2 runtime, which Windows 11 includes.

## 🧭 Everyday use

| To | Do this |
| --- | --- |
| See what it's doing | Click the tray icon. Windows may keep the icon behind the **^** arrow on the taskbar. |
| Change your list | Open the window and go to **Channels**. Drag a row to reorder it. |
| Pause | **Pause** in the tray view or on the first page of the window. |
| Quit | **Settings › Help and quitting › Quit**, or right-click the tray icon. Closing the window only hides it. |

## 🔄 Updates

The app checks for a newer version once a day while it's open, downloads it in the background and installs it when you quit. **Settings › Updates › Restart and update** installs it right away. Turn off **Update automatically** there and it only tells you.

Every update is checked against the release's SHA-256 and its signature before it's used.

## 🗂 Where things are kept

Nothing is installed. Everything the app writes is in the `data` folder beside it: your list, activity, settings and a log file. Your sign-in is the one exception; Windows Credential Manager keeps it, so it never travels with the folder.

To remove the app, sign out in **Settings**, quit it, and delete the folder.

## 🐛 If something goes wrong

**Settings › Help and quitting › Show in folder** opens `log.txt`. It records what the app did and anything that failed. It never holds your sign-in, your account name or request addresses. Attach it when you [report an issue](../../issues).

## 🛠 Build from source

Needs [Node.js](https://nodejs.org) 22 or later and [Rust](https://rustup.rs).

```bash
npm install
npm run tauri dev
```

## 👀 Visitors

<div align="center">

[![Visitor counter](https://count.getloli.com/@tsunsora-collector?theme=rule34&padding=8&offset=0&align=top&scale=1&pixelated=1&darkmode=auto)](https://count.getloli.com/)

<sub>Powered by [Moe Counter](https://github.com/journey-ad/Moe-Counter). This counts README image requests, not unique visitors; GitHub image caching can affect the total.</sub>

</div>
