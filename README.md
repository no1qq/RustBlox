# RustBlox

A modern, fast, and secure desktop client launcher for Roblox on Windows, written entirely in Rust.

RustBlox installs its own isolated copy of Roblox, launches it cleanly, protects your session with an active security watchdog, and keeps your game settings and launch configuration in one organized place.

## Highlights

- **Direct & Fast Official Installation**: Downloads Roblox directly from the official CDN and installs it into an isolated folder so it never conflicts with existing installs.
- **TheWatcher Anti-Cheat**: Built-in background security watchdog and system tray icon active while Roblox runs from RustBlox, enforcing protection against external cheat tools, DLL injection, and script executors.
- **FastFlags Management**: Direct profile editor for `ClientAppSettings.json` with native JSON formatting, import/export, and instant apply.
- **In-Game Settings Control**: Unlock your FPS limit, graphics quality, performance stats overlay, mouse sensitivity, and reduced motion directly from the launcher.
- **Custom Mods & Font Tool**: Easily apply custom fonts and textures with automatic backups so originals are always safely restored.
- **Discord Rich Presence**: Show the game you are playing and elapsed time directly on your Discord profile.
- **Customizable Shortcuts & Launch Options**: Desktop and Start menu shortcuts, deep link support (`roblox:` / `roblox-player:`), and programs launched alongside Roblox.
- **Modern Adaptive Interface**: Clean interface with light and dark mode following Windows system preferences.

## Getting Started

Starting RustBlox opens a compact launcher with three simple options:

1. **Launch Roblox**: Checks for updates, verifies files, applies your settings, and starts the game.
2. **Configure Settings**: Opens the full dashboard to manage game settings, FastFlags, mods, shortcuts, and appearance.
3. **Uninstall**: Cleanly removes RustBlox and its data without affecting your system.

### Command Line Options

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

## Building from Source

Requirements: Stable Rust toolchain with the `x86_64-pc-windows-msvc` target and MSVC build tools with the Windows SDK.

```bash
cargo build --release
```

The release executable will be located at `target/release/RustBlox.exe`.

## License

MIT
