# RustBlox

A desktop client manager for Roblox on Windows, written entirely in Rust.

RustBlox installs its own copy of Roblox, starts it, tells you honestly what happened,
and keeps your launch configuration in one place. It is a launcher and a manager, not
a mod, an injector or an account tool.

## What it does

- Downloads and installs Roblox itself, straight from the official CDN: it reads the
  package manifest, checks every package against the MD5 Roblox publishes, unpacks
  each one into the folder the client expects, and writes `AppSettings.xml`.
- Keeps that copy to itself. RustBlox never reads, launches or edits an install that
  Roblox made, so the two never fight over the same folder and anything RustBlox
  changes only ever affects its own copy.
- Checks for a newer Roblox when you press Launch, installs it if there is one, then
  starts the client, all from one small progress window.
- Cleans up after itself: once a new version is in place the old version folders and
  the leftover downloads go, keeping the current copy and anything you pinned.
- Starts the client as a detached process and then watches for it, so the launch
  panel reports what actually happened rather than a guess.
- Opens saved places through the Roblox deep link handler.
- Can take over the `roblox-player` and `roblox` link handlers so launches from the
  website route through RustBlox, saving whatever handler was there before so it can
  be put back exactly.
- Sets the things Roblox keeps for itself: the frame rate limit, the graphics quality,
  the performance stats overlay, interface transparency, reduced motion, text size,
  mouse sensitivity and VR. These are the settings the client actually honours, and
  RustBlox writes them straight into Roblox's own settings file before each launch.
- Edits the client flag file (`ClientAppSettings.json`) with validation, presets grouped
  by what they do, clipboard import and export, and a timestamped backup when it replaces
  a file it did not write. There is no save step: a flag you add, change or turn off is
  written to the client straight away, and written again before the next launch. Flags
  the client refused on the last run are marked as refused, read from its own log.
- Keeps settings, state and logs in a per-user folder, and survives a damaged or
  outdated configuration file without losing your data.
- Updates itself from its own GitHub releases, on request rather than silently.
- Follows the Windows light and dark setting, or stays on whichever you pick.
- Starts simple. The Flags page, the Installation page and the Advanced settings tab
  stay hidden until you turn on Advanced options in Settings.

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
RustBlox                    open the launcher
RustBlox --settings         open the full window on the Settings page
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

Pressing Launch opens a small progress window and runs one pipeline:

1. Ask the release channel what the current Roblox version is.
2. Install it if the copy on disk is not already that version. An install that is
   already current costs one request and finishes straight away.
3. Apply whatever is configured, verify the install, start the client.
4. Wait for the client process to appear, then close.

The update check is the only time RustBlox contacts the Roblox CDN on its own, and it
can be turned off in Settings, in which case a missing install is still downloaded.
When the check fails but a copy is already installed, the window says so and starts
that copy rather than refusing to launch.

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
outcome. The progress bar is real while a download is running and indeterminate while
it is waiting on the client, because there is no progress figure to report there.

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
- **Most flags are refused by the client.** Roblox 0.735 only honours an allowlist of
  local overrides. It reads the whole file, then writes
  `Denied local configuration for: <name>` into its log for every flag it throws out.
  The Flags page shows you which of yours came back refused. That is Roblox's decision,
  not a bug in the writer, and no launcher can talk it out of it. Anything the client
  refuses that has a real setting behind it lives on the Game page instead.
- **Flags are unsupported by Roblox.** The Flags page writes a file the client reads
  at startup. Roblox does not document it, and can change or ignore any value at any
  time, so a preset that stops working is Roblox renaming a flag, not RustBlox losing
  it. Roblox also wipes the version folder on update, which is why the profile is
  stored with RustBlox and rewritten on every launch.
- **The package map can go stale.** Roblox does not publish where each package
  belongs, so the mapping is maintained by hand. An unrecognised package is reported
  rather than ignored.
- **Only its own copy is used.** RustBlox looks in its own `Versions` folder and
  nowhere else. It does not go looking through `%LOCALAPPDATA%\Roblox`, Program Files
  or another launcher's folder, so an install Roblox made is never read or started.
  A copy kept somewhere else needs a custom path, which the Home page asks for when
  nothing is installed yet.

## The windows

Starting RustBlox opens a small launcher: Launch Roblox, Configure settings, and
Uninstall RustBlox. That is the whole window, and it is what you use day to day.
Each of the three replaces it with a window of its own, centred on screen.

Launch Roblox opens a compact progress window with the app icon, one line saying what
is happening, a progress bar and Cancel. It carries the whole pipeline: the update
check, the download if there is one, the configuration, and the wait for the client.
Once the client is running the window closes, and RustBlox with it. A failure leaves
the window up with the reason, Try again, and Close.

Configure settings opens the full window. Home there is a dashboard: whether the client
is running, what pressing Launch would open, the version on disk against the one Roblox
is publishing, your saved places, and a short list of things worth a look, each with the
button that deals with it. Nothing appears in that list that RustBlox cannot act on.
The rest of the window is where installs, quick launch entries, game settings,
appearance and everything else live. The sidebar there collapses to icons
with the button above the tabs. `rustblox --settings` opens it directly. Launching
from that window shows the same progress window, and once the client is running it
closes too, exactly as it does from the launcher. A failed launch comes back to the
window it started from.

Uninstall opens its own screen and asks first. It always removes
`%LOCALAPPDATA%\RustBlox\data`, which holds the Roblox copies RustBlox installed, its
logs, its saved state and its flag profile. Removing `%APPDATA%\RustBlox\config` as
well is a choice on that screen, so keeping it means a reinstall starts where you left
off. Any registered link handler is put back first, and the executable removes itself
last. A Roblox install that RustBlox did not create is never touched.

## Game settings

Roblox keeps its own settings in `%LOCALAPPDATA%\Roblox\GlobalBasicSettings_13.xml`,
and that is the only place the client reads them from, whichever folder it runs out of.
So the Game page writes that file, which is the one thing RustBlox touches that it does
not own. Because of that:

- It is off until you turn it on, and it writes nothing at all until then.
- The original is copied into the RustBlox backups folder before the first change.
- Only the values you switch on are written. Every other property in that file is left
  byte for byte as Roblox left it, and each row starts from the value Roblox already
  has, so switching a row on changes nothing until you move the control.
- Roblox rewrites the file when the client closes, so RustBlox writes the values again
  before every launch. **Keep them locked** marks the file read only afterwards, which
  stops the client putting its own values back. While that is on, settings you change
  inside Roblox stop sticking. Turning it off hands the file straight back.

This is also where the frame rate limit and the performance stats overlay live, because
the flags that used to do those two jobs are refused by the client now.

## Simple by default

Everything needed to install and play Roblox is on the Home, Game, Settings and About
pages.
Turning on **Advanced options** in Settings adds the Flags page, the Installation page
with its download controls and launch link handling, and the Advanced settings tab.
Turning it back off hides them again without changing anything they configured.

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
  app/            application state, background tasks, launch flow and sessions, toasts
  config/         settings model, validation, migration, persistence
  platform/       Windows integration behind a portable interface
    windows/      process enumeration, file version info, shell, registry
  selfupdate.rs   GitHub releases, download and in place swap
  roblox/         detection, deployment, installer, version housekeeping, launch
                  pipeline, URIs, flags, Roblox's own game settings
  uninstall.rs    removing RustBlox and the data it created
  ui/             theme, icons, app icon, widgets, pages, chrome, launcher, splash
  util/           filesystem helpers, formatting, version compare, logging
```

The layers only depend downward: `ui` reads `app`, `app` drives `roblox` and
`config`, and only `platform` contains Windows API calls. Everything that touches
the operating system sits behind a small interface with a non-Windows fallback, so
the rest of the code compiles and its tests run anywhere.

## Licence

MIT.
