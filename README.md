# uncoil

**A tiny, open-source lighting and device daemon for Razer peripherals on Windows — what Razer Synapse does
for your RGB, without the half-gigabyte of Electron.**

| | Razer Synapse 4 | uncoil |
|---|---|---|
| Processes | 17 (Electron app engine + services) | 1 (`uncoild`) |
| Memory | ~1.4 GB at start, leaks to many GB over days | **~3 MB** private |
| CPU (idle animation) | ~7% of a core, spikes at startup | **<1%** of a core |
| Install | ~500 MB | **0.65 MB** single executable |
| Kernel drivers | yes | none — plain user-mode HID |

*Measured on a Ryzen desktop with a BlackWidow V4 Pro 75%, Basilisk V3 Pro and Goliathus Chroma Extended.*

## What it does

- **One effect across the whole desk.** Every LED has a physical position, so an angled rainbow wave flows
  from the keyboard (including its side underglow) onto the mouse and mat as one continuous field.
- **LED-tuned colour.** Uses FastLED's rainbow hue map so no colour band looks wider or brighter than the rest.
- **Behaves like Synapse where it matters:** lighting fades out when Windows turns the display off, dims with
  it, and comes back when it wakes; devices that were unplugged are picked up again within seconds.
- **Leaves the keyboard's firmware in charge** of Fn keys, media keys and the dial (normal mode), so they keep
  working even if uncoil isn't running.
- **Optional:** one-shot hand-off of motherboard / GPU / RAM RGB to their own hardware rainbow via OpenRGB.

## Layout

```
crates/uncoil-core   protocol, colour, effects, device definitions, desk layout (pure, unit-tested)
crates/uncoil-hid    USB HID transport with per-device quirks; Windows display-power watcher
apps/uncoild         background daemon (no window)
apps/uncoil          desktop app — Tauri + SvelteKit + Tailwind; its preview runs the real effect code
devices/*.toml       one data file per device: USB endpoints, quirks, LED matrix, physical layout
docs/PROTOCOL.md     how the protocol was learned, and the hardware quirks
tools/reference      the Python probes and capture scripts used for reverse engineering
```

## Supported devices

| Device | Lighting | Notes |
|---|---|---|
| Razer BlackWidow V4 Pro 75% (wired) | per-key + 18 underglow LEDs | |
| Razer Basilisk V3 Pro (wired / HyperSpeed) | wheel, logo, 11-LED underglow | |
| Razer Goliathus Chroma Extended | 1 zone | |

Adding a device is mostly a data file — see [`devices/`](devices/) and [docs/PROTOCOL.md](docs/PROTOCOL.md).

## Build

```powershell
cargo test --workspace --exclude uncoil-gui
cargo build --release -p uncoild               # target\release\uncoild.exe
powershell -ExecutionPolicy Bypass -File scripts\install-task.ps1   # run at logon
```

Config: `%APPDATA%\uncoil\config.json` (hot-reloaded). Status for the GUI: `%LOCALAPPDATA%\uncoil\status.json`.

## Roadmap

- Host-side Fn layer, custom dial functions and OLED control for the BlackWidow (needs driver-mode capture)
- More devices via community data files and a guided capture tool
- Per-app profiles

## License

GPL-3.0-or-later. Protocol facts and some device data derive from OpenRazer and OpenRGB (both GPL-2.0).
Not affiliated with or endorsed by Razer Inc. "Razer", "Synapse" and "Chroma" are trademarks of Razer Inc.
