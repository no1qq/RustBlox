<div align="center">
  <img src="assets/logo.png" width="96" alt="RustBlox logo" />

# RustBlox

a lightweight, fast Roblox launcher and bootstrapper for Windows, written in Rust.

  <p>
    <a href="https://github.com/no1qq/RustBlox/releases"><img src="https://img.shields.io/badge/release-v0.2.1-ff5c00?style=flat-square&logo=github&logoColor=white" alt="Release" /></a>
    <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/rust-1.85+-dea584?style=flat-square&logo=rust&logoColor=white" alt="Rust" /></a>
    <a href="https://github.com/no1qq/RustBlox/releases"><img src="https://img.shields.io/badge/platform-Windows%20x64-0078d4?style=flat-square&logo=windows&logoColor=white" alt="Platform" /></a>
    <a href="https://github.com/no1qq/RustBlox"><img src="https://img.shields.io/badge/license-MIT-2ea44f?style=flat-square" alt="License" /></a>
  </p>
</div>

RustBlox downloads its own isolated copy of Roblox so your main installation is never touched, lets you manage FastFlags and game settings, applies custom fonts and mods cleanly, and runs a lightweight security watchdog while you play.

## features

- clean Roblox installation: downloads directly from the official Roblox CDN into an isolated data folder so it never conflicts with other copies on your machine.
- FastFlag manager: built-in editor for `ClientAppSettings.json` with typed values, import/export support, and instant apply without manual JSON editing.
- in-game settings control: tweak your FPS cap, graphics quality, mouse sensitivity, and motion settings directly from the launcher. (note: this modifies `%LOCALAPPDATA%\Roblox\GlobalBasicSettings_<n>.xml`, the shared Roblox settings file. it is turned off by default and automatically creates a backup before making any changes.)
- custom mods and fonts: drop in custom fonts or textures with one click; original files are backed up automatically and restored cleanly when mods are disabled.
- TheWatcher security watchdog: background scanner with a system tray icon that runs while Roblox is active to detect external memory reading handles, unbacked executable memory regions, transparent click-through overlays, and executor named pipes.
- Discord Rich Presence: shows your current game and playtime on Discord. place titles are queried once from Roblox's public web API only when presence is enabled; no other tracking or analytics requests are ever made.
- adaptive UI & portable mode: follows your Windows dark/light preference, starts instantly, and supports `--portable` to keep all configuration inside the local folder.

## getting started

running `RustBlox.exe` opens a small menu with three options:

1. launch Roblox: checks for updates, verifies files, applies your flags/mods, and starts the game.
2. configure settings: opens the full dashboard to manage flags, graphics, mods, shortcuts, and themes.
3. uninstall: cleanly wipes RustBlox data and shortcuts without leaving leftovers behind.

### command line options

```text
RustBlox                    open the launcher
RustBlox --launch           launch Roblox directly
RustBlox --settings         open the settings window
RustBlox --forward <uri>    launch a roblox: or roblox-player: link
RustBlox --portable         store configuration beside the executable
RustBlox --reset            reset settings to defaults
RustBlox --version          print the version
RustBlox --help             show usage help
```

## building from source

requires a stable Rust toolchain targeting `x86_64-pc-windows-msvc` and MSVC build tools with the Windows SDK.

```bash
cargo build --release
```

the compiled binary will be placed at `target/release/RustBlox.exe`.

## license

MIT
