---
title: Architecture
description: The daemon, the desktop app, the files between them, and the pieces still in progress.
fac: FAC 205
---

uncoil is split so that the part that runs all day is as small as it can be, and the part with a UI only
runs while you look at it.

```
                    %APPDATA%\uncoil\config.json
            writes ┌───────────────────────────────┐ reads, hot-reloads
  uncoil (app) ────┤                               ├──── uncoild (daemon) ──── USB HID ──── devices
            reads  └───────────────────────────────┘ writes every 2 s
                    %LOCALAPPDATA%\uncoil\status.json
```

## The daemon: `uncoild`

One process, no window, no console, no service, no kernel driver. On the maintainer's PC it is a 651 KB
executable using about 3 MB of private memory and under 1% of one core while animating.

- **Main loop** (every 33 ms; every 250 ms while the lights are faded out): watches the config file's
  modification time and reloads it, follows the display state and steps the fade level, rescans for new or
  replugged devices every 5 seconds, and writes the status file every 2 seconds.
- **One thread per device** renders the shared effect at the configured `fps`: it evaluates the effect at each
  LED's desk position and sends the frame row by row. Device positions are recomputed only when the config
  changes. When a device stops answering, its thread ends and the next rescan picks it up again.
- **Display watcher:** a hidden message window registered for Windows' `GUID_CONSOLE_DISPLAY_STATE`
  notifications (off, dimmed, on). On wake every device is re-prepared, since some reset during sleep.
- **Single instance:** a named mutex stops a second copy from starting.
- **Optional OpenRGB hand-off:** at start, one OpenRGB CLI run puts non-Razer RGB on its hardware rainbow,
  then OpenRGB exits. See [Configuration](/docs/configuration/#openrgb_hardware_rainbow).
- **Self-measurement:** the daemon samples its own private memory and CPU time and publishes them in the
  status file, so the footprint claim can be checked on any machine.

On connect, every device is switched to **normal mode**, so the firmware keeps running the Fn layer, media
keys and the dial. uncoil only ever supplies colours.

## The crates

| Path | What it is |
|---|---|
| `crates/uncoil-core` | Pure and unit-tested: the protocol (report building, CRC), colour, effects, device definitions, desk layout, config and status types. |
| `crates/uncoil-hid` | USB HID transport via hidapi, with the per-device quirks (reading every reply, retrying `busy`), and the Windows display-power watcher. |
| `apps/uncoild` | The daemon: the loop above, logging, self-measurement, the OpenRGB hand-off. |
| `apps/uncoil` | The desktop app: Tauri 2, SvelteKit and Tailwind. |
| `devices/*.toml` | One data file per device, compiled into the daemon. |
| `tools/reference` | The Python probes and log miners used for reverse engineering. |

## The desktop app

The app is a Tauri window around a small Svelte front end. It reads and writes the same `config.json`, reads
`status.json` to show which devices are live and what the daemon costs, and draws a live preview of the
effect. The preview runs a line-for-line TypeScript port of the engine's colour maths, guarded by a test in
the app's Rust side, so it shows exactly what the hardware will. (This website's pulse plot uses the same port.)

The app is meant to be opened, used and closed. Nothing depends on it running.

## The status file

`%LOCALAPPDATA%\uncoil\status.json`, rewritten every 2 seconds:

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
      "connection": "wired", "fps": 29.8, "busy_retries": 12, "errors": 0 }
  ],
  "memory_bytes": 3145728,
  "cpu_percent": 0.8,
  "exe_bytes": 666624
}
```

`level` is the current fade multiplier (0 when the display is off). `busy_retries` counts reports the device
answered `busy` to and that were sent again; `errors` counts the ones that failed outright.

## In progress

- **A control channel.** A named pipe so the app and scripts can talk to the running daemon directly
  (apply a setting, ask for state) instead of only through files.
- **A command-line tool** on top of that pipe, for switching effects from scripts, shortcuts and stream decks.
- Host-side Fn layer, dial functions and OLED images for the BlackWidow, which need more driver-mode capture
  first. See [Protocol](/docs/protocol/#still-to-map).
- Per-app profiles, and an installer.

None of these are released; this page will say when they are.
