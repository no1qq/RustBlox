# RustBlox

A desktop client manager for Roblox on Windows, written entirely in Rust.

RustBlox finds your Roblox install, starts it, tells you honestly what happened, and
keeps your launch configuration in one place. It is a launcher and a manager, not a
mod, an injector or an account tool.

## What it does

- Downloads and installs Roblox itself, straight from the official CDN: it reads the
  package manifest, checks every package against the MD5 Roblox publishes, unpacks
  each one into the folder the client expects, and writes `AppSettings.xml`.
- Prefers the copy it installed itself, falls back to a normal Roblox install, and
  reads the real file version out of `RobloxPlayerBeta.exe`.
- Checks the release channel on startup and says when a newer Roblox is out, without
  installing anything until you ask.
- Cleans up after itself: once a new version is in place the old version folders and
  the leftover downloads go, keeping the current copy and anything you pinned.
- Starts the client as a detached process and then watches for it, so the launch
  panel reports what actually happened rather than a guess.
- Opens saved places through the Roblox deep link handler.
- Can take over the `roblox-player` and `roblox` link handlers so launches from the
  website route through RustBlox, saving whatever handler was there before so it can
  be put back exactly.
- Edits the client flag file (`ClientAppSettings.json`) with validation and a
  timestamped backup on every write.
- Keeps settings, state and logs in a per-user folder, and survives a damaged or
  outdated configuration file without losing your data.
- Updates itself from its own GitHub releases, on request rather than silently.
- Follows the Windows light and dark setting, or stays on whichever you pick.

## Building

Requirements: a stable Rust toolchain with the `x86_64-pc-windows-msvc` target, and
the MSVC build tools with the Windows SDK.

```
cargo build            # debug
cargo build --release  # release, single self-contained exe
cargo test             # unit tests
cargo clippy           # lints
```

The release binary lands at `target/release/RustBlox.exe`. It has no runtime
dependencies beyond the system itself: fonts are loaded from the Windows font
directory when available and fall back to the fonts compiled into the binary, and
the icon and manifest are embedded by `build.rs`.

## Command line

```
RustBlox                    open the window
RustBlox --settings         open the window on the Settings page
RustBlox --launch           start Roblox using the configured startup target
RustBlox --forward <uri>    hand a roblox: or roblox-player: link to the client
RustBlox --portable         keep configuration next to the executable
RustBlox --reset            start with default settings, keeping the old file
RustBlox --version          print the version
RustBlox --help             print usage
```

A bare `roblox:` or `roblox-player:` argument is treated as `--forward`. That is the
form the registry entry uses when RustBlox is registered as the link handler.

## Where things are stored

| What | Location |
| --- | --- |
| Settings | `%APPDATA%\RustBlox\config\settings.json` |
| State, logs, backups | `%LOCALAPPDATA%\RustBlox\data\` |
| Flag profile | `%LOCALAPPDATA%\RustBlox\data\flag-profiles\default.json` |
| Roblox copies it installs | `%LOCALAPPDATA%\RustBlox\data\Versions\version-<id>\` |
| Downloaded packages | `%LOCALAPPDATA%\RustBlox\data\Downloads\version-<id>\` |

Set `RUSTBLOX_HOME` to override the root, or pass `--portable` to use a
`rustblox-data` folder beside the executable.

Settings carry a format version and are migrated forward on load. A file written by
a newer build is left untouched and defaults are used instead, so downgrading never
destroys your configuration. A file that cannot be parsed is moved aside with a
timestamped `.bad` suffix rather than deleted.

## How launching works

Roblox registers `RobloxPlayerBeta.exe` itself as the handler for its launch links,
and the executable accepts a launch URI directly as its first argument. RustBlox
uses the same three entry points the client already supports:

| Target | What is passed |
| --- | --- |
| Home screen | `--app` |
| A saved place | `roblox://experiences/start?placeId=<id>` |
| A link from the browser | the incoming URI, unchanged after validation |

After spawning, RustBlox polls for the client process until it appears, the child
exits, or the configured timeout elapses. Each of those is reported as a distinct
outcome. There is no progress bar, because there is no progress figure to report.

## How installing works

RustBlox does the same job the official bootstrapper does, in Rust:

1. Ask `clientsettingscdn.roblox.com/v2/client-version/WindowsPlayer` for the current
   version and its `clientVersionUpload` folder, optionally for a named channel.
2. Fetch `<folder>-rbxPkgManifest.txt` and parse the package list.
3. Download the packages a few at a time, trying four CDN mirrors and retrying each
   with a backoff, resuming any package that was half fetched.
4. Check every package against the MD5 in the manifest and refuse it if it differs.
5. Unpack each one into the folder Roblox expects, rejecting any archive entry that
   tries to escape the target directory.
6. Write `AppSettings.xml`.

It installs into its own `Versions` folder and never touches a copy owned by Roblox
or by another bootstrapper. Packages are fetched a few at a time, each one resumes
from the byte it stopped at, and every mirror is retried with a backoff before the
install gives up. Because the total size is known up front, the install panel shows a
real progress bar.

Nothing is written into the live version folder. Packages are unpacked into
`<folder>.incomplete` and that folder is swapped into place only once
`RobloxPlayerBeta.exe` is confirmed present, so a failed or cancelled install never
leaves a half working client behind. Free disk space is checked before the first byte
is downloaded.

The package-to-folder map is a table Roblox does not publish, so it can go out of
date. If Roblox ships a package RustBlox does not recognise, the install still
completes but you get an explicit warning naming the package rather than a silently
broken client. A test that runs against the live CDN checks the map is still complete.

### Known limits

- **RustBlox cannot sign you in.** Joining a specific server needs an authentication
  ticket that only Roblox can mint from your web session. RustBlox never asks for a
  password or a `.ROBLOSECURITY` cookie, which is why launching goes through the
  client or a deep link rather than a private join API. When RustBlox is registered
  as the link handler, the ticket in the incoming URI is passed straight through
  untouched, which is what makes launching from the website work.
- **Deep links depend on the client.** `roblox://experiences/start` is resolved by
  the Roblox client using the account signed in there. If no account is signed in,
  the client shows its own sign-in flow. That is outside RustBlox's control.
- **Flags are unsupported by Roblox.** The Flags page writes a file the client reads
  at startup. Roblox does not document it, and can change or ignore any value at any
  time. Roblox also wipes the version folder on update, which is why the profile is
  stored with RustBlox and rewritten on launch when you ask for it.
- **The package map can go stale.** Roblox does not publish where each package
  belongs, so the mapping is maintained by hand. An unrecognised package is reported
  rather than ignored.
- **Only two kinds of install are used.** RustBlox looks in its own `Versions` folder
  and in a normal Roblox install, in that order, and never in a folder owned by
  another launcher. An install somewhere else needs a custom path, which the
  Installation page accepts.

## Light and dark

Appearance is one setting with three positions: Automatic, Light and Dark.
Automatic is the default and reads the Windows setting
(`HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`),
rechecking about once a second so flipping Windows to dark changes RustBlox while it
is open. Light and Dark ignore Windows entirely, and switching back to Automatic
picks the system setting straight back up.

The accent defaults to the orange from the icon. In light mode it is darkened until a
label on top of it clears the WCAG AA contrast ratio, which a test enforces for every
accent in both modes.

## How RustBlox updates itself

On startup RustBlox reads the releases list for `no1qq/RustBlox`, ignores drafts and
prereleases, and picks the highest version tag that has a `RustBlox.exe` attached.
If that tag is newer than the running build the About page offers it. Nothing is
downloaded until you press the button.

Installing one works the way the Roblox installer does. The new build is written next
to the running executable as `RustBlox.exe.new`, checked for the `MZ` header and the
size GitHub published, and only then swapped in: the running build is renamed to
`RustBlox.exe.old` and the new one takes its place. If the swap fails the old build is
put straight back. Windows will not let a running program be deleted, so the `.old`
file stays until the next start, which clears it.

Your settings, flag profile and installed Roblox copies live outside the executable
and are untouched by an update.

## Project layout

```
src/
  main.rs         entry point, window setup, icon generation
  cli.rs          argument parsing
  error.rs        error type and context helpers
  app/            application state, background tasks, launch session, toasts
  config/         settings model, validation, migration, persistence
  platform/       Windows integration behind a portable interface
    windows/      process enumeration, file version info, shell, registry
  selfupdate.rs   GitHub releases, download and in place swap
  roblox/         detection, deployment, installer, version housekeeping, launch
                  pipeline, URIs, flags
  ui/             theme, icons, app icon, widgets, pages, chrome, overlays
  util/           filesystem helpers, formatting, version compare, logging
```

The layers only depend downward: `ui` reads `app`, `app` drives `roblox` and
`config`, and only `platform` contains Windows API calls. Everything that touches
the operating system sits behind a small interface with a non-Windows fallback, so
the rest of the code compiles and its tests run anywhere.

## Licence

MIT.
