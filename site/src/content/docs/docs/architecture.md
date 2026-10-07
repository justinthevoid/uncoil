---
title: Architecture
description: The daemon, the control pipe, the desktop app and the CLI, the files between them, and what is still in progress.
---

uncoil is split so that the part that runs all day is as small as it can be, and the parts with a UI only
run while you use them. The repository's
[`docs/ARCHITECTURE.md`](https://github.com/justinthevoid/uncoil/blob/main/docs/ARCHITECTURE.md) has the
full version, module by module.

```
  uncoil (app) ──┐  writes config.json            reads, hot-reloads
                 ├────────────────────────────────────────────┐
  uncoil (CLI) ──┤                                            │
                 └── \\.\pipe\uncoil (JSON lines) ───── uncoild (daemon) ──── USB HID ──── devices
  uncoil (app) ◀──── status.json, every 2 s ──────────────────┘
```

## The daemon: `uncoild`

One process, no window, no console, no service, no kernel driver. It runs as the logged-in user, unelevated.
On the maintainer's PC it is a 1.4 MB executable (1,402,880 bytes, 2026-10-07) that used about 3 MB of private memory and
under 1% of one core while animating (memory and CPU measured on an earlier build).

- **Main loop** (every 33 ms; every 250 ms while the lights are faded out): watches the config file's
  modification time and reloads it, follows the display state and steps the fade level, rescans for new or
  replugged devices every 5 seconds (and, at the same time, for other programs that drive the same devices),
  and writes the status file every 2 seconds.
- **One thread per device** renders the shared effect at the configured `fps`: it evaluates the effect at each
  LED's desk position and sends the frame row by row. Device positions are recomputed only when the config
  or the set of connected devices changes. When a device stops answering, its thread ends and the next rescan
  picks it up again.
- **The shared device lock:** every request to a Razer device runs under the named lock OpenRGB uses
  (`Global\RazerLinkReadWriteGuardMutex`; Razer's own software appears to share it), so two programs never mix
  their reports on one device. uncoil waits at most 25 ms for it: a frame is skipped, and a command is tried
  once more and then fails with "another program is talking to … right now".
- **Conflict notice:** at every rescan the daemon compares running programs' image names (nothing else)
  against a short list: Razer Synapse, the Razer Chroma SDK services, OpenRGB (unless it is uncoil's own live
  server) and SignalRGB. Matches go into the status file as `conflicts`, are logged once, and the app shows a
  notice.
- **The control pipe** `\\.\pipe\uncoil` serves the app and the CLI. Commands for a device (key maps, the
  dial, the screen, firmware effects, DPI, poll rate, power, read-only checks) are queued to that device's
  thread and run between two frames, so the daemon stays the only thing that talks to the hardware. Only the
  logged-in user can connect, and only from this PC; the app and CLI in turn refuse a pipe served by a
  process running as anyone else. Connections idle for 5 minutes are closed.
- **Display watcher:** a hidden message window registered for Windows' `GUID_CONSOLE_DISPLAY_STATE`
  notifications (off, dimmed, on). On wake every device is re-prepared, since some reset during sleep.
- **Key and audio listeners**, only while an effect needs them (reactive and ripple; the audio meter). Key
  presses become desk positions on the spot; which key it was is never kept.
- **Single instance:** the control pipe doubles as the lock. If the pipe name is already taken (another
  uncoild, or anything else), the daemon logs it and exits before opening a device.
- **Device files are untrusted input:** each is checked (Razer vendor id, known commands and transaction ids,
  size limits, plain ids and names) before use, and one you add yourself is experimental unless it sets
  `support`.
- **Optional OpenRGB** (off by default, `openrgb.mode`). OpenRGB itself runs from a separate elevated task,
  **uncoil-openrgb** (`install-task.ps1 -OpenRgb`), which runs `uncoild --openrgb-once` at logon. In
  `hardware` mode one OpenRGB CLI run puts the devices listed in the config on their hardware modes, then both
  exit. In `live` mode the task starts OpenRGB as an SDK server on `127.0.0.1` (every Razer detector off) and
  keeps it running, and the daemon's own OpenRGB thread connects to it with a small built-in client
  (`crates/uncoil-openrgb`), at most 30 frames a second, on its own thread so a slow OpenRGB never holds up the
  Razer devices. The motherboard, RAM and GPU join the desk as a "PC" column left of the keyboard: each
  device's OpenRGB zones become points (a single LED), vertical strips (a linear zone) or grids (a matrix
  zone), with ids `openrgb:<name>` that `config.desk` can move. See
  [Configuration](/docs/configuration/#openrgb).
- **Self-measurement:** the daemon samples its own private memory and CPU time and publishes them in the
  status file, so the footprint claim can be checked on any machine.

On connect, every device is switched to **normal mode**, so the firmware keeps running the Fn layer, media
keys and the dial. uncoil supplies colours, and writes to a device's own memory only when you ask it to.

## The pieces

| Path | What it is |
|---|---|
| `crates/uncoil-core` | Pure and unit-tested: the protocol (report building, checksum), colour, effects, device definitions, desk layout (including OpenRGB devices built from their zones), the feature commands (key maps, dial, OLED, firmware effects, DPI, poll rate, power, scroll wheel, device info), the pipe's message types, config and status types. |
| `crates/uncoil-hid` | USB HID transport via hidapi, with the per-device quirks (reading every reply, retrying `busy`) and the shared Razer device lock; the Windows display-power watcher; the key and audio listeners. |
| `crates/uncoil-openrgb` | A small OpenRGB SDK client for live mode: std only, connects to `127.0.0.1` only, treats the server as untrusted input. |
| `apps/uncoild` | The daemon: the loop above, the control pipe, read-only checks, logging, self-measurement, conflict detection, the OpenRGB hand-off or server and the live OpenRGB client. |
| `apps/uncoil-cli` | The `uncoil` command line, a client of the pipe. |
| `apps/uncoil` | The desktop app: Tauri 2, SvelteKit and Tailwind. |
| `devices/*.toml`, `devices/experimental/*.toml` | One data file per device, all compiled into the daemon. |
| `tools/reference`, `tools/devices`, `tools/art` | The Python probes and log miners used for reverse engineering; the generator for the experimental device files; the device drawings, traced from product photos. |

## The desktop app

The app is a Tauri window around a small Svelte front end. It reads and writes the same `config.json`, reads
`status.json` to show whether the engine is running and what it costs, and sends everything else (key maps,
the dial, DPI, checks) through the pipe, the same way the CLI does. Its live preview of the desk runs the
engine's own Rust effect code, so it shows what the hardware will. Run in a plain browser for UI work, it falls
back to a line-for-line TypeScript port of the effect maths and a mock daemon; this website's desk uses the
same port.

The app is meant to be opened, used and closed. Nothing depends on it running. If you want, it can stay in
the tray instead (an effect menu, Open and Quit), start there with Windows, and send battery notifications
for wireless devices, checked every 10 minutes while it runs. Only one copy runs at a time. Those preferences
live in the app's own `%APPDATA%\uncoil\app.json`, which the daemon never reads. In live OpenRGB mode the
preview draws the PC's OpenRGB devices too, from the zones the daemon reports in the status file.

## The command line

`uncoil` sends one request over the pipe and prints the answer (`--json` for the raw reply). Anything that
writes a device's own memory needs `--write`, prints the value before and after, and is read back where the
device allows. `uncoil help` lists every command.

```powershell
uncoil devices
uncoil keymap get keyboard P --layer fn
uncoil dpi mouse
uncoil scroll mouse       # tactile or free spin, acceleration, Smart Reel
uncoil info keyboard      # firmware version, layout and colour
uncoil check mouse        # the read-only checks, nothing is written
```

## The status file

`%LOCALAPPDATA%\uncoil\status.json`, rewritten every 2 seconds (values here are an example):

```json
{
  "pid": 14872,
  "version": "0.1.0",
  "started_unix": 1791000000,
  "updated_unix": 1791011820,
  "display": "on",
  "level": 1.0,
  "devices": [
    { "id": "razer-blackwidow-v4-pro-75", "name": "Razer BlackWidow V4 Pro 75%", "product_id": 691,
      "connection": "wired", "fps": 18.1, "busy_retries": 12, "errors": 0 }
  ],
  "memory_bytes": 3145728,
  "cpu_percent": 0.8,
  "exe_bytes": 1402880,
  "unknown_devices": [],
  "conflicts": [],
  "openrgb": { "state": "off", "detail": null, "devices": [], "ours": false }
}
```

`level` is the current fade multiplier (0 when the display is off). `busy_retries` counts reports the device
answered `busy` to and that were sent again; `errors` counts the ones that failed outright.
`unknown_devices` lists Razer devices on the bus that no device file knows, by product ID. `conflicts` lists
other programs seen driving the same devices (`{app, detail}`). `openrgb` is the live OpenRGB connection:
`state` is `off`, `waiting`, `connected` or `error`, `detail` says why in plain words, `devices` lists what it
drives (`{id, name, leds, zones}`), and `ours` says whether the running OpenRGB is uncoil's own server.

## In progress

- Confirming the [experimental devices](/docs/devices/#experimental), and on the Basilisk V3 Pro the power
  commands (its low-battery byte reads an unexplained `0x4C`) and scroll acceleration; DPI, stages, poll
  rate, scroll mode and Smart Reel are confirmed (2026-10-07).
- What each command-dial mode does, and OLED images for the BlackWidow, which need more capture first. See
  [Protocol](/docs/protocol/#still-to-map).
- Named profiles, per-app profiles, and an installer for the daemon.

None of these are released; this page will say when they are.
