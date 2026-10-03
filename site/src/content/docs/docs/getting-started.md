---
title: Getting started
description: Install uncoild as a logon task, retire Synapse without leaving devices in driver mode, check it is running, and remove it again.
fac: FAC 201
---

uncoil has no installer yet. Installing it means putting one executable somewhere and asking Windows to
start it at logon. This page does both, and covers the one step that can go wrong: retiring Synapse.

## What you need

- Windows 10 or 11.
- At least one [supported device](/docs/devices/), on its cable or dongle.
- PowerShell (part of Windows) and administrator rights once, to register the logon task.

## 1. Get uncoild

Download `uncoild.exe` and `install-task.ps1` from the
[GitHub Releases page](https://github.com/justinthevoid/uncoil/releases).

Or build it from source with a stable Rust toolchain (MSVC target):

```powershell
git clone https://github.com/justinthevoid/uncoil
cd uncoil
cargo build --release -p uncoild      # -> target\release\uncoild.exe
```

`uncoild --version` prints the version and exits; it is otherwise silent. It has no window and no console.

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

## 3. Run it at logon

From the folder with the two downloaded files:

```powershell
powershell -ExecutionPolicy Bypass -File install-task.ps1 -Exe .\uncoild.exe
```

From a source checkout, `scripts\install-task.ps1` with no arguments installs `target\release\uncoild.exe`.

The script asks for elevation (one UAC prompt), then:

- stops any running `uncoild` and copies the executable to `%ProgramFiles%\uncoil\uncoild.exe`, a folder only
  administrators can write (the task runs elevated, so its binary must not be swappable by other programs);
- registers a scheduled task named **uncoil** that starts it at your logon (after a 5 second delay), with no
  time limit, restarting it up to five times a minute apart if it exits;
- starts the task straight away.

The task runs elevated only so the optional [OpenRGB hand-off](/docs/configuration/#openrgb_hardware_rainbow)
can reach RAM lighting over SMBus. uncoild itself talks to Razer devices as ordinary user-mode HID.

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

## Change settings

Either open the desktop app, or edit `%APPDATA%\uncoil\config.json` in any text editor. uncoild notices the
file changing and applies it within a frame or two; there is nothing to restart. Without a config file it
runs the defaults. Every key is listed under [Configuration](/docs/configuration/).

## Update

Run the install script again with the new `uncoild.exe`. It stops the running copy before replacing it.

## Uninstall

```powershell
powershell -ExecutionPolicy Bypass -File uninstall-task.ps1   # task and executable (asks for elevation)
Remove-Item -Recurse "$env:LOCALAPPDATA\uncoil"            # status file and log
Remove-Item -Recurse "$env:APPDATA\uncoil"                 # your settings and any extra device files
```

uncoil leaves devices in normal mode, so the keys, dial and media controls keep working afterwards. Key
remaps written to a keyboard's onboard memory stay there until something rewrites them.
