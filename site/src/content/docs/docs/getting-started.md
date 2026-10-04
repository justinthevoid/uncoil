---
title: Getting started
description: Build uncoild, install it as a logon task, retire Synapse without leaving devices in driver mode, check it is running, and remove it again.
---

uncoil has no installer yet. Installing it means putting one executable somewhere and asking Windows to
start it at logon. This page does both, and covers the one step that can go wrong: retiring Synapse.

## What you need

- Windows 10 or 11.
- At least one [supported or experimental device](/docs/devices/), on its cable or dongle.
- PowerShell (part of Windows) and administrator rights once, to copy the executable to `%ProgramFiles%`.
  The daemon itself runs as you, unelevated.
- Until there is a release: Git and [Rust](https://rustup.rs) with the Visual Studio C++ build tools.

## 1. Get uncoild

uncoil is pre-release and nothing has been released yet, so for now you build it from source with a stable
Rust toolchain (1.85 or newer, MSVC target):

```powershell
git clone https://github.com/justinthevoid/uncoil
cd uncoil
cargo build --release -p uncoild -p uncoil-cli   # -> target\release\uncoild.exe and uncoil.exe
```

Once there is a release, its GitHub release page will carry `uncoild.exe`, the `uncoil.exe` command line,
`install-task.ps1`, `uninstall-task.ps1` and an installer for the desktop app.

`uncoild --version` prints the version and exits; it is otherwise silent. It has no window and no console.
`uncoil.exe` is the optional command line; it talks to the running daemon and needs no installing.

## 2. Retire Synapse safely

Synapse switches devices into **driver mode**, where the host is expected to handle some of the keyboard's
functions. If Synapse is killed (Task Manager, a crash, a forced uninstall) instead of quitting, it never
switches them back, and the BlackWidow's dial scrolls instead of changing the volume and the media keys go
dead until the keyboard is power-cycled.

In order:

1. **Quit Synapse** from its tray icon, so it gets the chance to restore the devices.
2. **Uninstall it** from Windows Settings, under Apps.
3. **Start uncoil** (next step). On connect, uncoil puts every device it drives back into normal mode,
   where the firmware runs the Fn keys, media keys, dial and DPI buttons itself.
4. If the dial still scrolls or the media keys do nothing, **unplug the keyboard and plug it back in**.
   That always clears driver mode.

:::caution[Don't run both]
Synapse and uncoil write to the same LEDs. With both running, the lighting flickers between the two
and the keyboard may be put back into driver mode. Pick one.
:::

uncoil notices when Synapse, Razer's Chroma SDK services, OpenRGB or SignalRGB is running (by process name,
every 5 seconds), notes it once in its log and shows a notice in the app and in `uncoil status`. It also takes
the same device lock as OpenRGB and, apparently, Razer's software, so their reports never get mixed up with
uncoil's on one device; when another program holds it, uncoil skips a frame, or a command fails with "another
program is talking to … right now" and can simply be tried again.

## 3. Run it at logon

From the source checkout, after building:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\install-task.ps1
```

With no arguments it installs `target\release\uncoild.exe`. To install an `uncoild.exe` from somewhere else
(for example a release download, once there is one), pass it with `-Exe .\uncoild.exe`.

The script asks for elevation (one UAC prompt), then:

- stops any running `uncoild` in your session and copies the executable to `%ProgramFiles%\uncoil\uncoild.exe`,
  a folder only administrators can write, so no program running as you can swap it; it checks the copy's
  SHA-256 against the original;
- registers a scheduled task named **uncoil** that starts it at your logon (after a 5 second delay),
  **unelevated**, with no time limit, restarting it up to five times a minute apart if it exits;
- starts the task straight away.

What it did is written to `%ProgramFiles%\uncoil\install.log` and shown at the end.

uncoild talks to Razer devices as ordinary user-mode HID and needs no administrator rights. Two options
change that:

- **`-OpenRgb`** also registers **uncoil-openrgb**, an elevated task that runs `uncoild --openrgb-once` at
  logon (after 10 seconds) for [motherboard, GPU and RAM lighting through OpenRGB](/docs/configuration/#openrgb),
  which needs administrator rights to reach RAM lighting over SMBus. In `hardware` mode it hands the devices
  over once and exits; in `live` mode it keeps OpenRGB running as a local server (Task Scheduler shows it
  **Running**) and the daemon sends it the desk effect. It also creates `%ProgramData%\uncoil\openrgb`, an
  administrators-only folder for OpenRGB's settings. The task does nothing until `openrgb.mode` is set in the
  config (the app's Settings page does that), and it first runs at your next sign-in. OpenRGB itself must be
  installed in `C:\Program Files\OpenRGB`, with PawnIO if OpenRGB needs it for your RAM or motherboard.
  Running the script again without `-OpenRgb` removes the task.
- **`-Elevated`** runs the daemon itself elevated, as installs before this option did. It is a fallback for
  a PC where uncoild cannot open its devices unelevated; none has been seen so far.

```powershell
powershell -ExecutionPolicy Bypass -File scripts\install-task.ps1 -OpenRgb
```

## 4. First run

Within a second the desk should show the default effect: a rainbow wave at 35°, one colour cycle every
14 seconds, one full rainbow per 26 keys of width, crossing from the keyboard to the mouse and mat.

To check on it from PowerShell:

```powershell
Get-ScheduledTask uncoil | Select-Object State      # Running
Get-Process uncoild                                  # one process
Get-Content "$env:LOCALAPPDATA\uncoil\status.json"   # devices, fps, memory, refreshed every 2 s
Get-Content "$env:LOCALAPPDATA\uncoil\uncoild.log" -Tail 20
```

The log records each device as it is opened (`opened Razer BlackWidow V4 Pro 75% (02B3, wired)`), display
changes and config reloads.

The command line asks the daemon directly:

```powershell
.\target\release\uncoil.exe status    # version, display state, the daemon's own memory and CPU, each device's fps
.\target\release\uncoil.exe devices   # connected devices and what each supports
.\target\release\uncoil.exe help      # every command
```

## Change settings

Either open the desktop app, or edit `%APPDATA%\uncoil\config.json` in any text editor. uncoild notices the
file changing and applies it within a frame or two; there is nothing to restart. Without a config file it
runs the defaults. Every key is listed under [Configuration](/docs/configuration/).

The app doesn't need to stay open. If you'd like it to, its **Tray & notifications** page can keep it in the
tray when the window closes (the tray icon switches the effect and opens the window), and start it in the
tray with Windows: that adds a per-user startup entry (the `Run` key under your account) launching
`uncoil-app.exe --tray`, and turning it off removes the entry. While the app runs, it checks wireless
devices' batteries every 10 minutes and notifies you once at the level you pick (20 % by default), once more
at 10 %, and once when charging reaches 100 %. Only one copy of the app runs; starting it again brings up
the open window. These preferences live in `%APPDATA%\uncoil\app.json`, apart from the engine's config.

Developers: notifications from a dev build (`pnpm tauri dev`) show under PowerShell's name and icon, because
the app isn't installed.

## Update

Run the install script again with the new `uncoild.exe`. It stops the running copy before replacing it. Pass
the same options as before: without `-OpenRgb`, an existing **uncoil-openrgb** task is removed.

## Uninstall

From the source checkout (or the folder with a release's `uninstall-task.ps1`, once there is one):

```powershell
powershell -ExecutionPolicy Bypass -File scripts\uninstall-task.ps1   # tasks and executable (asks for elevation)
Remove-Item -Recurse "$env:LOCALAPPDATA\uncoil"   # status file, log and the onboard-write journal
Remove-Item -Recurse "$env:APPDATA\uncoil"        # your settings, the app's settings and any extra device files
```

The uninstall script removes both tasks (**uncoil** and **uncoil-openrgb**; stopping the latter also ends
an OpenRGB server it started in live mode), `%ProgramFiles%\uncoil` and OpenRGB's settings folder
`%ProgramData%\uncoil`. It deletes nothing in your user profile: the two
`Remove-Item` lines are for a clean slate, and you run them yourself.

uncoil leaves devices in normal mode, so the keys, dial and media controls keep working afterwards. Anything
written to a device's own memory (key remaps, a saved firmware effect, DPI stages) stays there until
something rewrites it. To put a remapped key back first, use `uncoil keymap reset` or the app's Restore
original, before deleting `%LOCALAPPDATA%\uncoil`: the journal there remembers what each key did before
uncoil first wrote it.
