<!-- FAC 00 · uncoil · catalogue sheet -->

# uncoil

**A small, open-source lighting daemon for Razer peripherals on Windows. It does the part of Synapse you
actually use, in one 0.65 MB process.**

[![CI](https://github.com/justinthevoid/uncoil/actions/workflows/ci.yml/badge.svg)](https://github.com/justinthevoid/uncoil/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-f2f2f2?style=flat-square&labelColor=0b0b0b)](LICENSE)
[![Platform: Windows 10 | 11](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-f2f2f2?style=flat-square&labelColor=0b0b0b)](#install)
[![Status: pre-release](https://img.shields.io/badge/status-pre--release-d22?style=flat-square&labelColor=0b0b0b)](CHANGELOG.md)

<sub>FAC 00 &nbsp;/&nbsp; UNCOIL &nbsp;/&nbsp; LIGHTING DAEMON + DESKTOP APP &nbsp;/&nbsp; WINDOWS &nbsp;/&nbsp; 0.1.0 PRE-RELEASE</sub>

---

## The numbers

| | Razer Synapse 4 | uncoil (`uncoild`) |
|---|---|---|
| Processes | 17 | **1** |
| Memory | ~1.4 GB at start, leaking to multiple GB over days | **~3 MB** private |
| CPU, idle after startup | ~7% of a core | **<1%** of one core |
| Install size | ~500 MB | **0.65 MB**, single executable |
| Kernel drivers | yes | **none**, plain user-mode HID |

<sub>Measured on the maintainer's PC (Windows 11, BlackWidow V4 Pro 75%, Basilisk V3 Pro, Goliathus Chroma
Extended, rainbow wave running). One machine, not a benchmark; yours will differ.</sub>

No services, no account, no telemetry; the daemon makes no network connections. The optional desktop app
is opened when you want to change something and closed again. The daemon does the work.

## What it does

- **One effect across the whole desk.** Every LED has a physical position, so an angled rainbow wave flows
  from the keyboard (including its side underglow) onto the mouse and the mat as one continuous field.
- **Effects:** wave, spectrum, static, off. Brightness, saturation, speed, band width and angle.
- **Colour tuned for LEDs.** Uses FastLED's rainbow hue map, so no colour band looks wider or brighter than
  the rest.
- **Behaves like Synapse where it matters.** Lighting fades out when Windows turns the display off, dims with
  it, and comes back on wake. Unplugged devices are picked up again within seconds.
- **Leaves the firmware in charge.** Devices are kept in normal mode, so Fn keys, media keys and the volume
  dial work even when uncoil isn't running.
- **Optional:** a one-shot hand-off of motherboard, GPU and RAM RGB to their own hardware rainbow via
  [OpenRGB](https://openrgb.org), if it is installed.

## Screenshots

![The uncoil desktop app, Lighting page: a white hairline pulse plot of the desk's wave effect, effect controls and live device rows on matte black](docs/assets/app.png)

<sub>The Lighting page, running on the app's built-in demo data (engine figures and device rows are synthetic in
this capture).</sub>

## Supported devices

| Device | USB PID | Lighting | Status |
|---|---|---|---|
| Razer BlackWidow V4 Pro 75% (wired) | `1532:02B3` | per-key + 18 underglow LEDs | tested |
| Razer Basilisk V3 Pro (wired / HyperSpeed dongle) | `1532:00AA` / `1532:00AB` | scroll wheel, logo, 11-LED underglow | tested |
| Razer Goliathus Chroma Extended | `1532:0C02` | 1 zone | tested |

Adding a device is mostly a data file: USB endpoints, quirks, LED matrix and physical layout in one TOML.
See [CONTRIBUTING.md](CONTRIBUTING.md#adding-a-device) and open a
[device support request](https://github.com/justinthevoid/uncoil/issues/new?template=device_support.yml)
if you can help test.

## Install

> [!IMPORTANT]
> uncoil is pre-release. There are no published binaries yet; build from source below. The first tagged
> release will put `uncoild.exe` and an installer for the app on the
> [Releases](https://github.com/justinthevoid/uncoil/releases) page.

**Close Synapse first.** Two programs driving the same keyboard is a race neither wins. Quit Synapse and
disable its startup entry (or uninstall it). If Synapse left a device in driver mode, uncoil puts it back
in normal mode when it connects, so the dial and media keys come back.

```powershell
# from a clone, after building (see below)
powershell -ExecutionPolicy Bypass -File scripts\install-task.ps1
```

The script copies `uncoild.exe` to `%ProgramFiles%\uncoil` (admin-only, because the task runs elevated) and registers a logon task named `uncoil`.
The task runs elevated so OpenRGB can reach RAM lighting over SMBus; see [SECURITY.md](SECURITY.md) for what
that implies. It self-elevates if needed.

To remove it:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\uninstall-task.ps1   # task and binary
Remove-Item -Recurse "$env:LOCALAPPDATA\uncoil"   # status and log
Remove-Item -Recurse "$env:APPDATA\uncoil"        # your settings
```

### Files

| Path | What |
|---|---|
| `%APPDATA%\uncoil\config.json` | settings; edited by the app, hot-reloaded by the daemon |
| `%APPDATA%\uncoil\devices\*.toml` | extra or overriding device definitions |
| `%LOCALAPPDATA%\uncoil\status.json` | live device and engine status, read by the app |
| `%LOCALAPPDATA%\uncoil\uncoild.log` | daemon log, trimmed at 256 KB |

## Build from source

Requirements: Windows 10/11 x64, [Rust](https://rustup.rs) stable (1.85 or newer, MSVC toolchain with the
Visual Studio C++ build tools). For the desktop app also Node.js 22, [pnpm](https://pnpm.io) and the
WebView2 runtime (already on Windows 11); see the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```powershell
git clone https://github.com/justinthevoid/uncoil
cd uncoil

# daemon
cargo test -p uncoil-core -p uncoil-hid -p uncoild
cargo build --release -p uncoild                    # target\release\uncoild.exe

# desktop app (optional)
cd apps\uncoil
pnpm install
pnpm tauri dev                                      # live-reloading app window
pnpm tauri build                                    # release build + NSIS installer
```

`pnpm dev` alone serves the app UI in a browser against a mock with the real desk geometry, which is handy
for design work without hardware.

### Layout

```
crates/uncoil-core   protocol, colour, effects, device definitions, desk layout (pure, unit-tested)
crates/uncoil-hid    USB HID transport with per-device quirks; Windows display-power watcher
apps/uncoild         background daemon (no window)
apps/uncoil          desktop app: Tauri 2 + SvelteKit + Tailwind; its preview runs the real effect code
devices/*.toml       one data file per device: USB endpoints, quirks, LED matrix, physical layout
docs/PROTOCOL.md     how the protocol was learned, and the hardware quirks
tools/reference      Python probes and log-mining scripts used for reverse engineering
```

## How it works

Razer peripherals take one 90-byte HID feature report per command: a class, an id, up to 80 bytes of
arguments and an XOR checksum. The format is public thanks to [OpenRazer](https://github.com/openrazer/openrazer)
and [OpenRGB](https://gitlab.com/CalcProgrammer1/OpenRGB); uncoil re-implements it from those facts and from
Synapse's own logs, which record every command it sends with a name and the raw bytes.

`uncoild` places each device on a virtual desk, samples the effect at every LED's physical position, and
streams custom frames at 30 fps, respecting per-device quirks (the BlackWidow wants every reply read back,
or it quietly stops listening). The full write-up, with the expensive lessons, is in
[docs/PROTOCOL.md](docs/PROTOCOL.md).

**The Fn layer lives in the keyboard.** The BlackWidow stores its Fn layer in onboard memory. Fn+P has
Print Screen printed on the keycap but does nothing without Synapse, because that slot is empty. Writing
one key code into it makes Fn+P a native Print Screen, handled by the firmware, on any PC, with nothing
running. Today that is a reference script ([`tools/reference/obm_set_fnp.py`](tools/reference/obm_set_fnp.py));
the editor is on the roadmap.

## Roadmap

Nothing here has a date.

- **Fn-layer editor:** read and write the keyboard's onboard Hypershift layer, with a backup first.
- **Command dial and OLED:** dial functions and display control for the BlackWidow.
- **Hardware effects:** onboard firmware effects, so the desk keeps a look when uncoil isn't running.
- **Profiles:** named sets of effect and device settings.
- **More devices:** community device files and a guided, privacy-safe capture tool.
- **Per-app profiles:** switch lighting when a given program is in front.
- **Releases:** published binaries and an installer that sets up the daemon.

## Contributing

Bug reports, device data and protocol findings are all welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md);
it covers the dev setup, the device-file path and the reverse-engineering rules. The short version of those
rules: **never commit HID captures that contain keystrokes, Synapse logs or device serial numbers.**

Everyone taking part agrees to the [Code of Conduct](CODE_OF_CONDUCT.md). Security issues go through
[SECURITY.md](SECURITY.md), not public issues. Questions: [SUPPORT.md](SUPPORT.md).

## Support

uncoil is free and stays free. Starring the repository, filing a careful bug report or contributing a
device file all help more than you'd think.
<!-- Add a sponsor link here once GitHub Sponsors / Ko-fi is set up and .github/FUNDING.yml is filled in. -->

## License and trademarks

uncoil is free software under the [GNU General Public License v3.0 or later](LICENSE). Protocol facts and
some device data derive from OpenRazer and OpenRGB (both GPL-2.0); the keyboard key-mapping
commands were cross-checked against [OpenSynapse](https://github.com/A1mAssist/OpenSynapse) (MIT).

uncoil is an independent project. It is not affiliated with, endorsed by or sponsored by Razer Inc.
"Razer", "Synapse", "Chroma", "BlackWidow", "Basilisk" and "Goliathus" are trademarks of Razer Inc., used
here only to identify compatible hardware and software.

<sub>FAC 00 &nbsp;/&nbsp; END OF SHEET</sub>
