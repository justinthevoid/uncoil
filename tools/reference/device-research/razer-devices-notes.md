# Razer devices research: notes (2026-10-03)

Companion to `razer-devices.json` (30 devices, 40 USB PIDs; transaction id: 24 agree, 10 one source, 6 conflict). Facts only, transcribed from:

- OpenRazer `master` @ `a84cd0ae` (GPL-2.0): `driver/razer{kbd,mouse,accessory}_driver.{c,h}`, `razerchromacommon.c`,
  `razercommon.{c,h}`, `daemon/openrazer_daemon/hardware/{keyboards,mouse,mouse_mat,accessory,device_base}.py`
- OpenRGB `master` @ `df3024be` (GPL-2.0-or-later): `Controllers/RazerController/RazerDevices.{cpp,h}`,
  `RazerControllerDetect.cpp`, `RazerController/RazerController.{cpp,h}`
- USB-IF HID Usage Tables 1.12, Keyboard/Keypad page, column "Ref: Typical AT-101 Position" (for Q1)

Line numbers in the JSON refer to those commits. Each `usb[]` entry keeps the raw per-source facts under `sources`
(OpenRGB struct, detector line, OpenRazer daemon class + METHODS, and a per-function transaction-id table parsed
from the kernel driver). `transaction_id` is only filled when both sources agree or only one has the device;
otherwise it is `null` and `transaction_id_candidates` lists both. Mouse button counts/ids are `null` everywhere:
neither project maps buttons.

Devices: BlackWidow V3 / V3 Pro / V3 TKL / V4 / V4 Pro / V4 X / V4 75%, Huntsman V2 / V2 TKL / V3 Pro / V3 Pro TKL /
Mini, Ornata V3; DeathAdder V3 / V3 Pro / V2, Viper V2 Pro / V3 Pro / Mini, Basilisk V3 / V3 X HyperSpeed / V3 35K,
Cobra / Cobra Pro, Naga V2 Pro; Firefly V2, Strider Chroma, Base Station V2 Chroma, Mouse Dock Pro, Chroma
Addressable RGB Controller. All requested devices were found;
DeathAdder V3 / V3 Pro, Viper V2 Pro and Viper V3 Pro have no RGB, so OpenRGB does not list them (OpenRazer only).

## Q1. Are the class-0x02 remap key ids a generic scheme?

**Yes for key positions, very likely across Razer keyboards; not across device classes.**

- The ids are the IBM PC/AT key-position numbers that the HID Usage Tables print as "Typical AT-101 Position".
  Checked against HUT 1.12: a=31, P=26, Esc=110, F1=112, F12=123, Print Screen=124, Scroll Lock=125, Pause=126,
  Insert=75, Delete=76, Home=80, End=81, PgUp=85, PgDn=86, arrows L/U/D/R=79/83/84/89, Num Lock=90, KP/=95,
  KP1=93, KP0=99, `=1, Backspace=15, Tab=16, Enter=43, LShift=44, RShift=57, LCtrl=58, LAlt=60, Space=61, RAlt=62,
  RCtrl=64, Non-US \=45, Application=129. Every one matches `docs/blackwidow-keyids.json`.
- So `blackwidow-keyids.json` is effectively a generic table (Synapse's own names, AT-101 numbering), not a
  BlackWidow-matrix artefact. It is missing ids the AT-101 scheme predicts: **Left GUI = 127, Right GUI = 128**
  (the "Left Windows has no id" gap in the 75% TOML is very likely 127; untested). Japanese/Korean keys use
  Razer-specific ids 150–154; 42 (Non-US #) and 45 (Non-US \) are AT-101/102 positions, and 14 (Yen) / 56 (Ro)
  appear to follow the IBM 106-key numbering (HUT lists N/A for those, so this part is inferred).
- Against / limits: keys with no AT-101 position (Fn, M1–M6 macro keys, command dial, media keys, Hypershift keys,
  analog/profile keys) must use Razer extensions that are not documented anywhere; only the BW V4 Pro 75% was
  verified. Neither OpenRazer nor OpenRGB implements 02/0D / 02/8D, so there is no second source for the command
  ids on other keyboards. Mouse button ids (Basilisk: 1–5, 9, 10, 52, 53, 14, 15, 96, 106) are a different
  namespace (52 = tilt-left on the mouse, M on a keyboard).
- Practical rule: reuse the AT-101 table for any Razer keyboard, read the factory map with 02/8D for keys 1–154 to
  discover which ids a board answers, and treat extra keys as per-device until read on hardware.

## Q2. Shared mouse commands (OpenRazer `razerchromacommon.c`, used by `razermouse_driver.c`)

| feature | set | get | args | notes |
|---|---|---|---|---|
| DPI | `04/05` size 7 | `04/85` size 7 | `[storage, x_hi, x_lo, y_hi, y_lo, 0, 0]` | storage **NOSTORE (0)** for almost every modern mouse; **VARSTORE (1)** only for Naga X / Naga Left-Handed 2020 / Naga Pro / **Naga V2 Pro** / Orochi 2013 / Imperator / Mamba Elite / Atheris (razermouse_driver.c ~2517–2533). Get uses NOSTORE except Orochi 2013 / Imperator. |
| DPI stages | `04/06` size 0x26 | `04/86` size 0x26 | `[VARSTORE, active(1-based), count, count × [stage_idx, x_hi, x_lo, y_hi, y_lo, 0, 0]]` | always VARSTORE; driver caps count at 5 (`RAZER_MOUSE_MAX_DPI_STAGES`, razermouse_driver.h:137) |
| Poll rate (classic) | `00/05` size 1 | `00/85` | `[code]`: 0x01 = 1000, 0x02 = 500, 0x08 = 125 Hz | most mice; daemon default list 125/500/1000 (device_base.py:115) |
| Poll rate (HyperPolling) | `00/40` size 2 | `00/C0` | `[arg, code]`: 0x01 = 8000, 0x02 = 4000, 0x04 = 2000, 0x08 = 1000, 0x10 = 500, 0x20 = 250, 0x40 = 125 | DeathAdder V3, Viper V3 Pro wireless, Viper 8K, Viper Mini SE, DA V4 Pro, HyperPolling dongle; HyperPolling keyboards (BW V4 / V4 Pro / V4 75%, Huntsman V2 / V2 TKL). For DA V3 and Viper V3 Pro wireless OpenRazer sends it twice, `arg` 0x00 then 0x01 |
| Battery level | — | `07/80` size 2 | reply `args[1]` = 0–255 | scale to % |
| Charging | — | `07/84` size 2 | reply `args[1]` = 0/1 | |
| Idle / sleep timer | `07/03` size 2 | `07/83` | `[secs_hi, secs_lo]`, clamped 60–900 s | no storage byte |
| Low-battery threshold | `07/01` size 1 | `07/81` | `[raw]`, clamped 0x0C–0x3F (~5–25 % of 255) | **sent with 0xFF** on Basilisk V3 Pro (uncoil's own mouse), Viper V2 Pro, Cobra Pro; 0x1F elsewhere |
| Scroll mode / accel / smart reel | `02/14`, `02/16`, `02/17` size 2 | `02/94`, `02/96`, `02/97` | `[VARSTORE, value]` | Basilisk V3 family |
| Device mode | `00/04` | `00/84` | `[mode, param]` | |
| Firmware / serial | — | `00/81` / `00/82` (size 0x16) | | |
| Brightness | `0F/04` size 3 | `0F/84` | `[VARSTORE, led, 0–255]` | per LED id |
| Active profile | — | `05/84` size 1 | reply `[profile]` | OpenRazer razerkbd_driver.c:689 uses it as the storage byte for 0F/02 effects on Huntsman V3 Pro 8KHz: on profile devices the "storage" byte selects a profile, VARSTORE = profile 1. Confirms uncoil's "probably active profile". |

VARSTORE = 0x01, NOSTORE = 0x00 (razercommon.h:33-34). All lighting effects in both projects are written with
VARSTORE; custom frames with NOSTORE.

## Q3. Devices whose transport/protocol differs

- **Feature-report interface is not always 0.** Keyboards answer on interface 2 or 3 (OpenRazer
  `razer_get_report_params` wIndex agrees with OpenRGB's detector interface on every keyboard here except the
  BW V3 Pro dongle). Basilisk V3 and V3 35K use interface 3; every other mouse/mat/accessory here uses 0.
- **BlackWidow V4 / V4 Pro: HID collection changed at firmware 1.5** (page 0x01 / usage 0x00 before, 0x0C / 0x01
  after; same interface 3) — OpenRGB registers both. The V4 75% is registered only as 0x01/0x00 while uncoil's
  V4 Pro 75% answers on 0x0C/0x01. Match on interface (and try both usages), not on a fixed usage.
- **Wireless transaction ids.** BW V3 Pro dongle (0x025C): OpenRazer 0x9F, OpenRGB 0x3F (and OpenRGB's other
  HyperSpeed keyboards use 0x9F). Wireless keyboards need a 4.9 ms reply wait; HyperSpeed mice 31 ms
  (`RAZER_NEW_MOUSE_RECEIVER_WAIT_US`), Viper-class receivers / Viper V3 Pro wireless 59.9 ms; wired devices 600 µs.
- **Mixed transaction ids within one device (OpenRazer).** BW V3 / V3 TKL / V3 Pro wired: custom-frame rows 0x1F,
  effects 0x3F. Viper Mini: lighting 0x3F, DPI/poll 0xFF. Cobra: lighting 0x1F, breathing 0x3F, DPI/poll 0xFF.
  Basilisk V3: DPI stages 0xFF. Firefly V2 / Strider / ARGB controller: lighting 0x3F, device mode + serial 0xFF.
  Many keyboards send device mode and game/macro LED commands with 0xFF. uncoil's single `transaction_id` per PID
  will not cover these; a per-command override (or retry with 0xFF on status 0x05/no answer) is needed.
- **Huntsman V2 / V2 TKL / V3 Pro / V3 Pro TKL:** OpenRazer 0x1F, OpenRGB 0x3F. Since Synapse uses a rolling
  counter (PROTOCOL.md), many firmwares may ignore the value; unverified.
- **Chroma Addressable RGB Controller** uses two interfaces: 0 for normal 90-byte commands, 1 for a ~320-byte ARGB
  frame report (report id 0x04 for channels 0–4, 0x84 above; header `[channel, channel, 0, last_idx]` + up to 315
  colour bytes). Both projects agree.
- **Mouse Dock Pro** uses transaction id 0xFF (both sources).
- **Bluetooth PIDs** (BW V3 Pro 0x025B, Basilisk V3 Pro 0x00AC): OpenRGB's detectors are commented out, OpenRazer
  has none. Treat as unsupported.
- OpenRazer only uses the "standard" (03) matrix commands for older devices; every device in this list uses the
  extended (0F) set in both projects.

## Disagreements (also in each device's `conflicts[]`)

- Matrix size: BW V4 75% 6x18 (OpenRGB) vs 6x16 (OpenRazer); Huntsman V2 TKL 6x17 vs 6x18; Huntsman V3 Pro TKL
  6x19 vs 6x22; Basilisk V3 35K 1x11 vs 1x13; Naga V2 Pro 1x2 vs 1x3; Mouse Dock Pro 1x9 vs 1x8; DeathAdder V2 1x2
  vs 1x1 (+ logo/scroll zones). Resolve on hardware with `0F/80` (regions → rows/cols), which both uncoil and
  OpenRGB already read.
- Per-LED vs zones: Cobra Pro (OpenRGB 1x11 per-LED, OpenRazer zones only), Cobra and Basilisk V3 X HyperSpeed
  (OpenRGB 1x1, OpenRazer no matrix).
- Effect LED id: Viper Mini and Cobra — OpenRazer logo 0x04, OpenRGB 0x05.
- DeathAdder V2 (0x0084) transaction id: OpenRazer 0x3F, OpenRGB 0x1F.
- OpenRGB naming slips: 0x007C/0x007D are listed as "DeathAdder V2 (Wired/Wireless)" but are the DeathAdder V2 Pro
  (its own PID macro names them V2_PRO); Naga V2 Pro is "Naga Pro V2".

## Other findings relevant to uncoil

- OpenRazer has no class for the BlackWidow V4 Pro 75% (0x02B3/0x02B4) at this commit; OpenRGB is the only prior art.
- OpenRGB queries regions (`0F/80`) and per-region effects (`0F/81`) at runtime and only falls back to PID lists
  (wave fallback, breathing-forced-off for the Basilisk V3 family). Doing the same read-only probe on first connect
  would settle most matrix/effect disagreements without hard-coding.
- OpenRazer's "ripple" is a daemon software effect, not firmware.

## Licensing

Only identifiers, sizes, numbers and names were transcribed (PIDs, class/id bytes, argument layouts, matrix sizes,
table names, line references). No code or comments were copied. The parsing helpers used to extract them
(`parse_or.py`, `parse_rgb.py`, `parse_drv.py`, `build_devices.py`, `finalize.py`) are in the same scratchpad.
