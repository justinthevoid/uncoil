---
title: Protocol
description: How uncoil talks to Razer devices, how that was learned from Synapse's own logs, and the hardware quirks that cost the most time.
---

:::note[Source]
This page is a copy of [`docs/PROTOCOL.md`](https://github.com/justinthevoid/uncoil/blob/main/docs/PROTOCOL.md);
the repository version is canonical. Everything here was learned from public prior art, Razer's own logs on
the maintainer's machine, and the hardware itself.
:::

What uncoil knows about talking to Razer peripherals, how it was learned, and the device quirks that
cost the most time. Machine-readable catalog of every command Razer's own software sent to the
BlackWidow V4 Pro 75%: [`protocol-catalog.json`](https://github.com/justinthevoid/uncoil/blob/main/docs/protocol-catalog.json). How uncoil's daemon uses all of
this: [`ARCHITECTURE.md`](https://github.com/justinthevoid/uncoil/blob/main/docs/ARCHITECTURE.md).

## How this was learned

1. **Prior art.** The report format is public thanks to [OpenRazer](https://github.com/openrazer/openrazer)
   and [OpenRGB](https://gitlab.com/CalcProgrammer1/OpenRGB). uncoil re-implements it from those facts.
   Onboard key mapping and profile commands come from [OpenSynapse](https://github.com/A1mAssist/OpenSynapse)
   (MIT, `ViperObmProtocol.cs`).
2. **Razer's own logs.** Synapse 4 logs every command it sends, with a human-readable name *and* the raw
   bytes, under `%LOCALAPPDATA%\Razer\RazerAppEngine\User Data\Logs`. Mining 130 MB of those logs
   ([`tools/reference/mine_synapse_logs.py`](https://github.com/justinthevoid/uncoil/blob/main/tools/reference/mine_synapse_logs.py)) produced the
   catalog above: 30 commands and 116 distinct input events, no USB sniffer required. A second pass over
   all ~600 MB (2026-10-03) added the command-dial mode table, the mouse button table and the
   function-data samples quoted below.
3. **Live probing** of real hardware with small Python scripts (`tools/reference/`), reading back status
   codes and asking a human what the LEDs actually did. Everything marked *read 2026-10-03* was read with
   [`readonly_probe.py`](https://github.com/justinthevoid/uncoil/blob/main/tools/reference/readonly_probe.py), which refuses to send any command without
   the "get" bit, on the maintainer's BlackWidow V4 Pro 75% (`02B3`), Basilisk V3 Pro (`00AA`) and
   Goliathus Chroma Extended (`0C02`).

Status words used below: **verified** (sent by uncoil or its scripts and confirmed on hardware),
**logged** (bytes seen in Synapse's logs), **read** (a read-only command answered on hardware),
**prior art** (OpenRazer / OpenRGB / OpenSynapse), **inferred** (consistent with the above, not tested).

## Transport

One 90-byte **feature report**, written with report id `0` (91 bytes through hidapi).

| offset | field | notes |
|---:|---|---|
| 0 | status | `0x00` new, `0x01` busy, `0x02` ok, `0x03` fail, `0x04` timeout / no answer, `0x05` unsupported |
| 1 | transaction id | per device: `0x1F` keyboard + mouse, `0x3F` Goliathus mat (Synapse itself uses a rolling counter); some firmwares want another id for some command groups, see below |
| 2–3 | remaining packets | 0 |
| 4 | protocol type | 0 |
| 5 | data size | number of argument bytes; getters often announce more (`0x50`) than they send |
| 6 | command class | |
| 7 | command id | high bit set = "get" (read-only) |
| 8–87 | arguments | up to 80 bytes |
| 88 | CRC | XOR of bytes 2–87 |
| 89 | reserved | |

Each device answers on one specific HID collection (interface + usage page + usage); see [Devices](/docs/devices/).
Some keyboards moved their collection in a firmware update (BlackWidow V4 / V4 Pro: page `0x01` usage
`0x00` before 1.5, `0x0C` / `0x01` after, same interface 3, per OpenRGB), so a device file can accept more
than one (`alt_usages`).

**Transaction ids per command group** (prior art, OpenRazer): a few devices use different ids for
different commands, e.g. Viper Mini lighting `0x3F` but DPI / poll `0xFF`; BlackWidow V3 custom-frame rows
`0x1F` but effects `0x3F`; the Basilisk V3 Pro, Viper V2 Pro and Cobra Pro send only the low-battery
threshold pair (`07/01`, `07/81`) with `0xFF`. Device files say so in `[usb.transaction_ids]` (groups in
[`ARCHITECTURE.md`](https://github.com/justinthevoid/uncoil/blob/main/docs/ARCHITECTURE.md#transaction-ids-per-command-group)). Wireless receivers also need a
longer pause before the reply (OpenRazer: 31 ms for HyperSpeed mice, 59.9 ms for Viper-class receivers,
4.9 ms for wireless keyboards), `reply_wait_us` in the device file.
A reply echoes class and id. Right after a request the device may still show `busy`, `new` or the
previous command's reply; uncoil re-reads with growing pauses and only then re-sends.

Synapse's logs contain a second, older command table for the same keyboard (`rzDevice30`, 2025 to May
2026) in which several OLED getters sit at different ids. The current firmware/Synapse pair uses the
`rzDevice25` table, which is the one documented here and confirmed by the reads.

## Commands used by uncoil

| class/id | name | args → reply | status |
|---|---|---|---|
| `00/04` | Set Device Mode | `[mode, 0]` — `0x00` normal (firmware runs Fn, media keys, dial, DPI buttons), `0x03` driver | verified, logged |
| `00/84` | Get Device Mode | → `[mode, 0]` (`[0, 0]` on all three devices) | read |
| `0F/02` | Set effect (extended matrix) | see [Firmware effects](#firmware-effects) | verified (custom frame, wave) |
| `0F/03` | Custom frame row | `[0, 0, row, start_col, stop_col, r,g,b, …]` | verified |
| `0F/80` | Lighting regions | → 5-byte records `[led, 25, 3, rows, cols]` | read |
| `0F/81` | Effects of a region | `[led]` → `[led, effect ids…]` | read |
| `0F/84` | Get brightness | `[storage, led]` → `[storage, led, 0-255]` (keyboard backlight: 114) | read |
| `02/8D` `02/0D` | Get / set key mapping (keyboard) | see [Onboard key mappings](#onboard-key-mappings-the-fn-layer-lives-in-the-keyboard) | verified |
| `02/8C` `02/0C` `02/84` | Get / set button mapping, list buttons (mouse) | | read / prior art |
| `05/80` `05/81` `05/84` `05/8A` | Profiles | see [Profiles](#profiles) | read |
| `17/00` `17/80` | Set / get command dial mode | see [Command dial](#command-dial-blackwidow-v4-pro-75) | logged / read |
| `17/03` | Set OLED brightness | `[percent]` | logged |
| `17/82`–`17/96` | OLED getters | see [OLED](#oled-display) | logged, read |

See the catalog for firmware, macro storage etc., and [Shared mouse commands](#shared-mouse-commands) for
DPI, poll rate and power.

## Shared mouse commands

From OpenRazer (`razerchromacommon.c` and `razermouse_driver.c` at a84cd0ae; facts transcribed into
`tools/reference/device-research/`). **From OpenRazer, unverified on uncoil's devices:** none of these has
been sent to or read from the Basilisk V3 Pro yet, so its device file lists them as `unverified` and uncoil
reads each value back (read-only check) before changing it.

| class/id | name | args → reply | notes |
|---|---|---|---|
| `04/05` | set DPI | `[storage, x_hi, x_lo, y_hi, y_lo, 0, 0]` (size 7) | storage `0` (NOSTORE) on almost every modern mouse: the DPI lasts until the DPI button changes it; `1` (VARSTORE) on Naga V2 Pro, Naga Pro and a few older mice |
| `04/85` | get DPI | `[storage]` (size 7) → `[storage, x_hi, x_lo, y_hi, y_lo]` | |
| `04/06` | set DPI stages | `[1, active, count, count × [index, x_hi, x_lo, y_hi, y_lo, 0, 0]]` (size `0x26`) | always VARSTORE (stored); active is 1-based, each record's index 0-based; at most 5 stages |
| `04/86` | get DPI stages | `[1]` (size `0x26`) → same layout | |
| `00/05` / `00/85` | poll rate, classic | `[code]` (size 1): `1` = 1000, `2` = 500, `8` = 125 Hz | most mice |
| `00/40` / `00/C0` | poll rate, HyperPolling | `[arg, code]` (size 2): `0x01` 8000, `0x02` 4000, `0x04` 2000, `0x08` 1000, `0x10` 500, `0x20` 250, `0x40` 125 Hz; reply code in argument 1 | DeathAdder V3, Viper V3 Pro wireless, Viper 8K, Viper Mini SE, DeathAdder V4 Pro, the HyperPolling dongle, and (among uncoil's experimental files) the BlackWidow V4 / V4 Pro / V4 75% and Huntsman V2 / V2 Tenkeyless keyboards. OpenRazer sends the set twice to some of these mice, argument `0` then `1` (`set_twice` in the device file) |
| `07/80` | battery | size 2 → `[_, 0–255]` | shown as a percentage |
| `07/84` | charging | size 2 → `[_, 0/1]` | |
| `07/03` / `07/83` | sleep timer | `[secs_hi, secs_lo]` | 60–900 s, no storage byte |
| `07/01` / `07/81` | low-battery threshold | `[raw]` (size 1) | `0x0C`–`0x3F` of 255 (about 5–25 %); transaction id `0xFF` on the Basilisk V3 Pro |

uncoil clamps DPI to the device file's `[dpi]` range, the sleep timer to 60–900 s and the threshold to
`0x0C`–`0x3F` before sending.

## Firmware effects

`0F/02 Set effect` uses one argument layout for every effect (OpenRGB
`razer_create_mode_effect_extended_matrix_report`; the byte captures in OpenRazer's `razerchromacommon.c`
agree, and uncoil's tests assert them byte for byte):

| arg | meaning |
|---:|---|
| 0 | storage: `0` this session only, `1` save to onboard memory (OpenRazer `VARSTORE`) |
| 1 | LED / region: `0` whole device, `1` scroll wheel, `4` logo, `5` keyboard backlight, `10` underglow |
| 2 | effect: `0` off, `1` static, `2` breathing, `3` spectrum, `4` wave, `5` reactive, `6` ripple, `7` starlight, `8` custom frame, `9` fire, `10` wheel |
| 3 | flags: wave/wheel direction (OpenRGB: `1` right, `2` left; older devices `0`/`1`), or colour count for breathing / starlight |
| 4 | rate: wave speed (lower = faster, firmware default `0x28`), reactive duration `1..4`, starlight `1..3` |
| 5 | colour count |
| 6… | colours, 3 bytes each |

Data size is `6 + 3 × colours`. OpenRazer writes starlight with flags `0`; OpenRGB (newer) uses the colour
count, which uncoil follows. Note storage `1` is an onboard write; uncoil defaults to `0`.

What each device runs:

| device | LED id | lists itself (`0F/81`, read 2026-10-03) | uncoil offers | source |
|---|---|---|---|---|
| BlackWidow V4 Pro 75% | `5` (`0F/80`: one 6×18 region) | 0–9 | off, static, breathing, spectrum, wave, wheel, reactive, starlight | OpenRazer (V4 family, incl. wheel) |
| Basilisk V3 Pro | `0` (regions 1, 4, 10 = wheel, logo, 11-LED strip) | 0, 1, 2, 3, 5, 8 per region | off, static, breathing, spectrum, wave, reactive | OpenRazer adds wave; OpenRGB notes the list is not exhaustive |
| Goliathus Chroma Extended | `0` (region 4, 1×1) | 0, 1, 2, 3, 5, 8 | off, static, breathing, spectrum | reactive needs input the mat lacks |

Firmware effects are not yet verified on hardware by uncoil (the custom-frame and wave paths are). The
first `uncoil effect hw` on each device is the test; it is session-only and `uncoil effect software`
(or any config change) undoes it.

## Input reports (keyboard, vendor collection)

| report | meaning |
|---|---|
| `2` | media keys |
| `4` | "Razer keys": up to 7 held Razer key codes. Fn = `1`; dial click `82`; next `83`; prev `84`; play/pause `85`; dial `96` |
| `5` | hardware events `[5, event, value…]`: `81` OLED mode (0 normal, 1 home, 2 command dial, 3 driver, 4 Fn, 5 off, 6 BLE pairing, 7 low battery), `82` OLED menu (5 = home screen change), `85` OLED low power (1 enter, 0 exit), `49` battery (`value*100/255` %), `9` wireless connect (3) / disconnect (2) for mouse (1) or keyboard (2), `12` external power removed, `57` scroll notch (0) / free spin (1), `2` DPI change `[x_hi, x_lo, y_hi, y_lo]` (mouse) |

## Quirks (the expensive lessons)

- **Driver mode sticks.** Synapse leaves devices in driver mode (`0x03`). Kill Synapse without restoring
  them and the BlackWidow's dial scrolls instead of changing volume and the media keys go dead until the
  device is power-cycled. uncoil puts every device in normal mode on connect.
- **Custom-frame effect: send once.** Re-sending `0F/02 0x08` every frame makes the BlackWidow freeze on the
  first frame. The mouse and mat don't care.
- **The keyboard needs every reply read.** Stream frame rows without reading the reply and the BlackWidow
  silently stops applying them after a while (all statuses still report ok). Reading the reply after each
  row (~4 ms) fixes it; occasional `busy` replies are retried. Mouse and mat are fine fire-and-forget.
- **Underglow LEDs live in odd matrix slots.** The BlackWidow V4 Pro 75%'s 18 underglow LEDs are packed into
  spare cells of its 6×18 matrix: three *left* underglow LEDs sit at top-row columns 14–16, three *right*
  ones around the spacebar. Treat the matrix as a grid and the side lighting shows colours from the wrong
  side of the board. `devices/razer-blackwidow-v4-pro-75.toml` maps every slot to its physical position.
- **The HyperSpeed dongle answers `0x04`** while the mouse is on its cable; treat that as "not here yet".
- **OpenRGB 1.0's SDK server** accepted per-LED updates for these devices but never pushed them to the
  hardware, leaving animations frozen; uncoil drives Razer devices directly.
- **Mice echo layer 0** in key-mapping replies even for Hypershift requests (OpenSynapse saw the same on the
  Viper), and the Basilisk stores its DPI-clutch mapping as `len 1` followed by all five data bytes.

## Onboard key mappings (the Fn layer lives in the keyboard)

The BlackWidow stores a two-layer key map per profile in its own memory: **Normal** and **Hypershift**.
On this keyboard Hypershift *is* the Fn layer, so "Synapse-only" Fn combinations are simply empty or
"send a Razer key to the host" entries. Write a real key code into the Hypershift layer and the firmware
handles the combination itself: no driver mode, no daemon, works on any PC.

| class/id | name | args → reply | status |
|---|---|---|---|
| `02/8D` | **get key mapping** (keyboard) | `[profile, key, layer]` → `[profile, key, layer, fn, len, data…]` | verified |
| `02/0D` | **set key mapping** (keyboard) | `[profile, key, layer, fn, len, data…]` (data size byte `0x50`) | verified |
| `02/8C` / `02/0C` | the same for mice | reply always 10 bytes: data padded to 5 | read / prior art |
| `02/84` | list button ids (mouse) | → `[count, ids…]`; keyboard answers `0x05` | read |
| `02/8F` | bulk get (keyboard) | `[profile, key, layer]` → `[profile, key]` echo + nine 8-byte records `[key, fn, len, data(5)]` | read, not understood |

`02/8F` answered with the normal-layer records for keys 1–9 whatever start key and layer were asked
(read 2026-10-03), so uncoil reads layers key by key with `02/8D` (79 reads, about half a second).

- `layer`: 0 Normal, 1 Hypershift (Fn).
- `key`: Razer key ids, e.g. 26 = P, 76 = Delete, 120–123 = F9–F12, 124 = Print Screen. The 79 keys of
  the 75% board, with their LED names and factory normal-layer mappings, are in
  `devices/razer-blackwidow-v4-pro-75.toml` (`[keymap]`); Synapse's full id table is
  [`blackwidow-keyids.json`](https://github.com/justinthevoid/uncoil/blob/main/docs/blackwidow-keyids.json). Left Windows and Fn have no id there.
- The ids are the IBM PC/AT key positions the USB HID Usage Tables print as "Typical AT-101 Position"
  (checked against HUT 1.12: A = 31, P = 26, Esc = 110, F1 = 112, Print Screen = 124, Space = 61, …; every
  id in `blackwidow-keyids.json` matches), so the table very likely holds for other Razer keyboards too
  (inferred). The scheme predicts **Left GUI = 127, Right GUI = 128** (untested; probably the missing Left
  Windows id). Keys with no AT-101 position (Fn, macro keys, the dial, media keys) use Razer ids that are
  not documented anywhere. Mouse button ids are a different numbering (52 is tilt left on the Basilisk, not
  a key).
- Basilisk V3 Pro button ids (`02/84`, read): 1 left, 2 right, 3 wheel click, 4 back, 5 forward, 9/10 wheel
  up/down, 52/53 wheel tilt left/right, 14 profile button (`DKM_SB_01`), 15 DPI clutch (`DKM_SB_02`),
  96 DPI button (`DKM_SB_03`), 106 scroll-mode button (`DKM_M_106`). Names in parentheses are Synapse's.

`fn` (function id, shared with Razer mice; names are Synapse's `fnIdEnum`):

| fn | name | data | status |
|---:|---|---|---|
| 0 | Off | — | prior art |
| 1 | ButtonCode | `[button]`: 1 left, 2 right, 3 middle, 4 back, 5 forward, 9/10 wheel up/down, 104/105 tilt left/right | read, logged |
| 2 | KeyCode | `[modifier bits, HID usage]`; bits 1 LCtrl, 2 LShift, 4 LAlt, 8 LWin, 16 RCtrl, 32 RShift, 64 RAlt, 128 RWin; modifier keys are `[bit, 0]` | verified |
| 3, 4, 5, 15 | MacroTypeI–IV | macro reference; Synapse logged `MacroTypeII [0, 250]` | logged, not mapped |
| 6 | DPI | `[6]` cycle up, `[5, x_hi, x_lo, y_hi, y_lo]` clutch (400×400 logged); OpenSynapse also accepts `[1]`, `[2]`, `[7]` (stage up / down, cycle down: inferred) | read, logged |
| 7 | Profile | `[4]` cycle up (the only value logged) | read, logged |
| 8 | Lighting | not mapped | |
| 9 | PowerKeys | `[0x82]` system sleep (HID generic desktop usage) | logged |
| 10 | MediaKeys | 2 bytes (OpenSynapse); keyboard layout not mapped | prior art |
| 11 | DoubleClick | `[1]` | prior art |
| 12 | ModeButtonKey (Hypershift) | `[1]` | prior art |
| 13 | TurboModeKey | 4 bytes, not mapped | prior art |
| 14 | TurboModeButton | `[button, interval_hi, interval_lo]`, interval in ms (`[104, 0, 20]` = tilt left at 50/s) | logged |
| 16 | Controller | not mapped | |
| 17 | RazerKey | `[code]`, reported to the host in input report 4: 3 game mode, 4 macro record, 8/9 backlight up/down, 11 low-power mode, 76 sleep | verified, logged |
| 18 | WindowsShortcutsKey (keyboard) / ScrollWheelMode (Basilisk, `[1]` = notch/free-spin toggle) | | read, logged |

Verified on hardware (2026-10-03): Fn+Delete reads `17 [76]` (exactly the `04 01 4C` seen in a driver-mode
capture), Fn+F9 reads `17 [4]`, Fn+Space reads `2 [0, 44]` (the key itself), and Fn+P read `2 [0, 0]` — an
empty key code, which is why Fn+P (Print Screen on the keycap) does nothing without Synapse. Writing
`02/0D [1, 26, 1, 2, 2, 0, 0x46]` made Fn+P a native Print Screen, and it still reads `2 [0, 0x46]`.
Tool: `tools/reference/obm_set_fnp.py`; the daemon equivalent is
`uncoil keymap set keyboard P key PRINT_SCREEN --layer fn --write`.

## Profiles

| class/id | name | BlackWidow (read) | Basilisk (read) |
|---|---|---|---|
| `05/8A` | max profiles | `[5]` | `[5]` |
| `05/80` | profile count | `[1]` | `[1]` |
| `05/81` | profile ids `[count, ids…]` | `[1, 1]` (size 80) | `[5, 1]` (size 2) |
| `05/84` | **probably** the active profile | `[1]` | `[1]` |

With one profile on each device `05/84` cannot be told apart from "first profile id"; treat it as unconfirmed.
On the keyboard `05/82`, `05/83`, `05/85`–`05/87` and `05/89` answer unsupported and `05/88` fails.
Switching profiles was not seen in Synapse's logs or OpenSynapse; by convention it would be `05/04 [id]`,
which uncoil does not send.

## Command dial (BlackWidow V4 Pro 75%)

`17/00 Set OLED Command dial active mode [profile, mode, display_order, enabled_functions]` (logged:
`{"profileId":1,"mode":0,"modeEnum":"VOLUME","displayOrder":1,"totalEnabledFunctions":6}` ↔ `[1, 0, 1, 6]`).
`display_order` is the 1-based position of the mode among the enabled modes in id order, and
`enabled_functions` how many are enabled (logged: TRACK_SELECTOR `[1, 1, 2, 6]`, OLED_BRIGHTNESS `[1, 2, 3, 7]`
with SCROLL_VERTICAL enabled). `17/80 [profile]` reads it back (read 2026-10-03: `[1, 0, 0, 0]`: VOLUME;
the firmware does not keep the order and total). The older `rzDevice30` stack sent a 3-byte variant
`[1, 2, 6]`.

Modes, with what Synapse binds to turning right / pressing / turning left (Synapse `commandDial.modes`):

| id | mode | right | press | left | Synapse default |
|---:|---|---|---|---|---|
| 0 | VOLUME | VolumeUp | MuteVolume | VolumeDown | on |
| 1 | TRACK_SELECTOR | NextTrack | Play | PrevTrack | on |
| 2 | OLED_BRIGHTNESS | OLED brightness up | toggle | down | on |
| 3 | LIGHTNING_BRIGHTNESS (sic; backlight) | BrightnessUp | BrightnessToggle | BrightnessDown | on |
| 4 | SWITCH_APPS | SwitchAppRight | CycleApps | SwitchAppLeft | on |
| 5 | ZOOM | OfficeZoomIn | OfficeZoomReset | OfficeZoomOut | on |
| 6 | TRACK_JOGGING | TrackJoggingForward | Play | TrackJoggingBackward | off |
| 7 | SCROLL_VERTICAL | ScrollUp | — | ScrollDown | off |
| 8 | SCROLL_HORIZONTAL | ScrollRight | — | ScrollLeft | off |

Known: in normal mode the dial sends consumer volume codes (`E9`/`EA`, press = mute `E2`), also in driver
mode; dial click / next / prev / play are Razer keys 82–85 in report 4; while the dial menu is on screen the
keyboard reports OLED mode 2 "Command_Dial_Mode" (event `81`). **Unknown:** whether the firmware performs the non-volume modes
itself once `17/00` selects them in normal mode, or only shows them (Synapse performs them in driver mode),
and whether the enabled-mode list is stored anywhere in the device. `uncoil dial set` is the experiment;
it is an onboard write and needs `--write`.

## OLED display

Getters (current table, logged with names; values read 2026-10-03 on the maintainer's keyboard):

| class/id | name | size | reply |
|---|---|---:|---|
| `17/82` | home screen | 7 | `[type, index…]`: type 0 ANIMATION (gif index), 1 IMAGE (image index); read `[0, 1]` |
| `17/83` | brightness | 1 | percent; read 100 |
| `17/84` | time to home screen | 1 | `1` = TIME_5S (the only value seen) |
| `17/85` | language | 1 | `0` = ENGLISH |
| `17/86` | time to dim | 1 | minutes; read 1 |
| `17/8D` | home screen active item | 2 | `[item, item]`: 0 ANIMATION, 3 KEYBOARD_INFO, 7 OFF |
| `17/8F` | image GUID | 22 | `[type, index, guid(20)]`; type 2 has slots 0–5, type 9 slots 0–9; all GUIDs zero |
| `17/92` | low battery warning | 1 | percent; read 20 |
| `17/93` | OLED low power mode | 1 | `0` = INACTIVE |
| `17/94` | home screen animation state | 6 | one enable flag per built-in animation |
| `17/95` | home screen image state | 10 | one enable flag per image slot |
| `17/96` | screensaver option | 2 | `[mode, item]` |

Setters seen in the logs: `17/03 [percent]` (brightness, every slider step), `17/17 Set OLED Menu Chroma
Effect [profile, 14]` and `17/18 Set Chroma Brightness Status [profile, region, status]` (meaning not
mapped). The other setters very likely mirror the getters without the high bit; uncoil does not send
unobserved writes. Synapse keeps `timeToHomeScreenSeconds`, `timeToDimMinutes`, the low-battery warning and
the screensaver in its own `oledSettings`; how (or whether) those reach the device is not in the logs.

**Future work:** OLED image / animation upload. The slots and their GUIDs are readable, but the upload
command, image format and chunking never appear in the logs.

## Still to map

- Hardware verification of firmware effects (`0F/02`) on all three devices, and of `dial set` / `oled set`.
- Profile switching and confirmation of `05/84`.
- The shared mouse commands (DPI, stages, poll rate, battery, sleep timer, low-battery threshold) on the
  Basilisk V3 Pro: `uncoil check mouse` reads them all without writing anything.
- Every file in `devices/experimental/` (built from OpenRazer / OpenRGB data).
- Media-key (`fn` 10), macro, lighting and Windows-shortcut data layouts on the keyboard.
- The `02/8F` bulk-read paging, and the factory Hypershift defaults for keys Synapse never wrote.
- OLED setters beyond brightness; OLED image upload.
