# uncoil

**A small, open-source lighting daemon for Razer peripherals on Windows. It does the part of Synapse you
actually use, in one 1.3 MB process.**

[![CI](https://github.com/justinthevoid/uncoil/actions/workflows/ci.yml/badge.svg)](https://github.com/justinthevoid/uncoil/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-f2f2f2?style=flat-square&labelColor=0b0b0b)](LICENSE)
[![Platform: Windows 10 | 11](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-f2f2f2?style=flat-square&labelColor=0b0b0b)](#install)
[![Status: pre-release](https://img.shields.io/badge/status-pre--release-d22?style=flat-square&labelColor=0b0b0b)](CHANGELOG.md)

---

## The numbers

| | Razer Synapse 4 | uncoil (`uncoild`) |
|---|---|---|
| Processes | 17 | **1** |
| Memory | ~1.4 GB at start, leaking to multiple GB over days | **~3 MB** private |
| CPU, idle after startup | ~7% of a core | **<1%** of one core |
| Install size | ~500 MB | **1.3 MB**, single executable |
| Kernel drivers | yes | **none**, plain user-mode HID |

<sub>Measured on the maintainer's PC (Windows 11, BlackWidow V4 Pro 75%, Basilisk V3 Pro, Goliathus Chroma
Extended, rainbow wave running); memory and CPU on an earlier build, size on the build of 2026-10-03
(1,257,472 bytes). One machine, not a benchmark; yours will differ. The daemon reports its own memory, CPU
and size in `status.json`, so you can check yours.</sub>

No services, no account, no telemetry; the daemon makes no network connections. The optional desktop app
and the `uncoil` command line talk to it over a local named pipe that only your user account can open. The
app is opened when you want to change something and closed again. The daemon does the work.

## What it does

- **One effect across the whole desk.** Every LED has a physical position, so an angled rainbow wave flows
  from the keyboard (including its side underglow) onto the mouse and the mat as one continuous field.
- **The Razer quick effects:** wave, spectrum, breathing, static, starlight, fire, wheel, reactive, ripple and
  an audio meter, each with its own settings, plus brightness and saturation.
- **Studio.** Stack effects as layers with opacity, and limit each layer to the whole desk, chosen devices or
  keys you paint. Reactive and ripple only ever learn where a key is, never which key it was.
- **Keys and buttons.** Remap keys on the normal and Fn layers and the mouse's buttons, set the command dial
  mode and the screen's brightness, and save a device's own (firmware) effect. These write to the device's own
  memory, so they keep working without uncoil. Every such write needs an explicit confirmation (`--write` on
  the command line) and is logged; settings that can be read back are, and the result says so.
- **Mouse settings:** DPI, DPI stages, polling rate, battery level, sleep timer and low-battery warning on
  mice whose device file lists them, using OpenRazer's shared mouse commands. These are not yet confirmed on
  the Basilisk V3 Pro, so uncoil reads each value first and only changes it if the read makes sense.
- **Colour tuned for LEDs.** Uses FastLED's rainbow hue map, so no colour band looks wider or brighter than
  the rest.
- **Behaves like Synapse where it matters.** Lighting fades out when Windows turns the display off, dims with
  it, and comes back on wake. Unplugged devices are picked up again within seconds.
- **Leaves the firmware in charge.** Devices are kept in normal mode, so Fn keys, media keys and the volume
  dial work even when uncoil isn't running.
- **A command line,** `uncoil`, for status, devices, key maps (with TOML backups), the dial, the screen,
  firmware effects, DPI, polling rate and power. Run `uncoil help` for the full list.
- **Optional, off by default:** a one-shot hand-off of motherboard, GPU and RAM RGB to their own hardware
  modes via [OpenRGB](https://openrgb.org), if it is installed at `C:\Program Files\OpenRGB`. You list the
  devices and modes in the config (`openrgb.devices`; there are no built-in names) and install with
  `-OpenRgb`, which adds a small elevated task that runs it once at logon. It closes a running OpenRGB in
  your session first. See [configuration](site/src/content/docs/docs/configuration.md#openrgb_hardware_rainbow).

## Screenshots

![The uncoil desktop app, Lighting page: the desk drawn to scale and lit with the wave, a gallery of effect cards, and the wave's settings in the right-hand panel](docs/assets/app.png)

<sub>The Lighting page, running on the app's built-in demo data (engine figures and device rows are synthetic in
this capture).</sub>

## Supported devices

| Device | USB PID | Lighting | Status |
|---|---|---|---|
| Razer BlackWidow V4 Pro 75% (wired) | `1532:02B3` | per-key + 18 underglow LEDs | tested |
| Razer Basilisk V3 Pro (wired / HyperSpeed dongle) | `1532:00AA` / `1532:00AB` | scroll wheel, logo, 11-LED underglow | tested |
| Razer Goliathus Chroma Extended | `1532:0C02` | 1 zone | tested |

### Experimental

These are set up from OpenRazer and OpenRGB data, and nobody has confirmed them on real hardware yet. Before
uncoil changes anything stored on one of them (key and button mappings, DPI stages, polling rate, sleep timer),
it reads a few settings first to make sure the device answers the way it expects; nothing is written until that
check passes. If you own one,
[tell us whether it works](https://github.com/justinthevoid/uncoil/issues/new?template=device_report.yml).
Each one is a file in [`devices/experimental/`](devices/experimental/), with the source of every value in comments.

- **Keyboards:** BlackWidow V3, V3 Pro, V3 Tenkeyless, V4, V4 Pro, V4 X, V4 75%; Huntsman V2, V2 Tenkeyless,
  V3 Pro, V3 Pro Tenkeyless, Mini; Ornata V3
- **Mice:** Basilisk V3, V3 35K, V3 X HyperSpeed; Cobra, Cobra Pro; DeathAdder V2, V3, V3 Pro; Naga V2 Pro;
  Viper Mini, V2 Pro, V3 Pro
- **Mats and accessories:** Firefly V2, Strider Chroma, Base Station V2 Chroma, Mouse Dock Pro

Adding a device is mostly a data file: USB endpoints, quirks, LED matrix and physical layout in one TOML.
See [CONTRIBUTING.md](CONTRIBUTING.md#adding-a-device) and open a
[device support request](https://github.com/justinthevoid/uncoil/issues/new?template=device_support.yml)
if you can help test.

## Install

> [!IMPORTANT]
> uncoil is pre-release. Nothing has been released and there are no published binaries yet; build from
> source below. The release workflow is set up so that a tagged release attaches `uncoild.exe`, the `uncoil`
> CLI, the install scripts and an installer for the app to a GitHub release.

**Close Synapse first.** Two programs driving the same keyboard is a race neither wins. Quit Synapse and
disable its startup entry (or uninstall it). If Synapse left a device in driver mode, uncoil puts it back
in normal mode when it connects, so the dial and media keys come back.

```powershell
# from a clone, after building (see below)
powershell -ExecutionPolicy Bypass -File scripts\install-task.ps1
```

The daemon runs as you, **unelevated**. The script asks for one UAC prompt only to copy `uncoild.exe` to
`%ProgramFiles%\uncoil`, where only administrators can write (so no program running as you can swap it); it
checks the copy's SHA-256, registers a logon task named `uncoil` and starts it. Two options:

- `-OpenRgb` also registers `uncoil-openrgb`, an elevated task that runs once at logon to hand motherboard,
  GPU and RAM lighting to OpenRGB (RAM sits on the SMBus, which needs administrator rights), and creates the
  admin-only folder `%ProgramData%\uncoil\openrgb` for OpenRGB's settings. It does nothing until you turn
  the hand-off on in the config.
- `-Elevated` runs the daemon itself elevated, as older installs did. It is a fallback for a PC where the
  daemon cannot open its devices unelevated (none seen so far).

See [SECURITY.md](SECURITY.md) for what runs elevated and why. The script installs only the daemon: the
`uncoil` CLI (`target\release\uncoil.exe`) runs from wherever you put it, and the desktop app has its own
installer.

To remove it:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\uninstall-task.ps1   # both tasks, the binary, %ProgramData%\uncoil
Remove-Item -Recurse "$env:LOCALAPPDATA\uncoil"   # status, log and the onboard-write journal
Remove-Item -Recurse "$env:APPDATA\uncoil"        # your settings and extra device files
```

The uninstaller removes nothing in your user profile; the last two lines are for a clean slate.

Anything uncoil wrote into a device's own memory (key maps, a saved firmware effect, DPI stages) stays there.

### Files

| Path | What |
|---|---|
| `%APPDATA%\uncoil\config.json` | settings; edited by the app, hot-reloaded by the daemon |
| `%APPDATA%\uncoil\devices\*.toml` | extra or overriding device definitions, read at start |
| `%LOCALAPPDATA%\uncoil\status.json` | live device and engine status, read by the app |
| `%LOCALAPPDATA%\uncoil\uncoild.log` | daemon log, trimmed at 256 KB |
| `%LOCALAPPDATA%\uncoil\onboard-writes.jsonl` | one line per write to a device's memory; `keymap reset` restores from it |
| `%ProgramFiles%\uncoil\uncoild.exe` | the installed daemon (and `install.log`, what the last install did) |
| `%ProgramData%\uncoil\openrgb` | OpenRGB's settings for the hand-off, administrators only (`-OpenRgb` or `-Elevated` installs) |

## Build from source

Requirements: Windows 10/11 x64, [Rust](https://rustup.rs) stable (1.85 or newer, MSVC toolchain with the
Visual Studio C++ build tools). For the desktop app also Node.js 22, [pnpm](https://pnpm.io) and the
WebView2 runtime (already on Windows 11); see the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```powershell
git clone https://github.com/justinthevoid/uncoil
cd uncoil

# daemon and CLI
cargo test -p uncoil-core -p uncoil-hid -p uncoild -p uncoil-cli
cargo build --release -p uncoild -p uncoil-cli      # target\release\uncoild.exe, target\release\uncoil.exe

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
crates/uncoil-core    protocol, colour, effects, device definitions, desk layout, pipe types (pure, unit-tested)
crates/uncoil-hid     USB HID transport with per-device quirks; display-power watcher; key and audio listeners
apps/uncoild          background daemon (no window); serves the control pipe \\.\pipe\uncoil
apps/uncoil-cli       the `uncoil` command line, a client of that pipe
apps/uncoil           desktop app: Tauri 2 + SvelteKit + Tailwind; its preview runs the real effect code
devices/*.toml        one data file per device: USB endpoints, quirks, LED matrix, physical layout
devices/experimental  experimental device files, generated by tools/devices/ from OpenRazer / OpenRGB data
docs/PROTOCOL.md      how the protocol was learned, and the hardware quirks
docs/ARCHITECTURE.md  how the daemon, the pipe and its clients fit together
tools/reference       Python probes and log-mining scripts used for reverse engineering
```

## How it works

Razer peripherals take one 90-byte HID feature report per command: a class, an id, up to 80 bytes of
arguments and an XOR checksum. The format is public thanks to [OpenRazer](https://github.com/openrazer/openrazer)
and [OpenRGB](https://gitlab.com/CalcProgrammer1/OpenRGB); uncoil re-implements it from those facts and from
Synapse's own logs, which record every command it sends with a name and the raw bytes.

`uncoild` places each device on a virtual desk, samples the effect at every LED's physical position, and
streams custom frames (30 fps by default), respecting per-device quirks (the BlackWidow wants every reply
read back, or it quietly stops listening). Other commands from the app or the CLI run on the same device
thread between frames. The full write-up, with the expensive lessons, is in
[docs/PROTOCOL.md](docs/PROTOCOL.md); how the code fits together is in
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

**The Fn layer lives in the keyboard.** The BlackWidow stores its Fn layer in onboard memory. Fn+P has
Print Screen printed on the keycap but does nothing without Synapse, because that slot is empty. Writing
one key code into it makes Fn+P a native Print Screen, handled by the firmware, on any PC, with nothing
running: `uncoil keymap set keyboard P key PRINT_SCREEN --layer fn --write`, or the app's Keys page.

## Roadmap

Nothing here has a date.

- **Command dial and OLED:** the dial's per-mode functions and custom images for the BlackWidow's screen
  (today: the active dial mode and the screen's brightness).
- **Profiles:** named sets of effect and device settings, and switching onboard profiles.
- **More devices:** confirming the experimental ones, community device files and a guided, privacy-safe
  capture tool.
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
some device data derive from OpenRazer and OpenRGB (both GPL-2.0-or-later); the keyboard key-mapping
commands were cross-checked against [OpenSynapse](https://github.com/A1mAssist/OpenSynapse) (MIT).

uncoil is an independent project. It is not affiliated with, endorsed by or sponsored by Razer Inc.
"Razer", "Synapse", "Chroma", "HyperSpeed" and the device names used here ("BlackWidow", "Basilisk",
"Goliathus" and the rest) are trademarks of Razer Inc., used only to identify compatible hardware and
software.

