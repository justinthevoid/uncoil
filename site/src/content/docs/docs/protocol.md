---
title: Protocol
description: How uncoil talks to Razer devices, how that was learned from Synapse's own logs, and the hardware quirks that cost the most time.
fac: FAC 204
---

:::note[Source]
This page is a copy of [`docs/PROTOCOL.md`](https://github.com/justinthevoid/uncoil/blob/main/docs/PROTOCOL.md);
the repository version is canonical. Everything here was learned from public prior art, Razer's own logs on
the maintainer's machine, and the hardware itself.
:::

What uncoil knows about talking to Razer peripherals, how it was learned, and the device quirks that
cost the most time. Machine-readable catalog of every command Razer's own software sent to the
BlackWidow V4 Pro 75%: [`protocol-catalog.json`](https://github.com/justinthevoid/uncoil/blob/main/docs/protocol-catalog.json).

## How this was learned

1. **Prior art.** The report format is public thanks to [OpenRazer](https://github.com/openrazer/openrazer)
   and [OpenRGB](https://gitlab.com/CalcProgrammer1/OpenRGB). uncoil re-implements it from those facts.
2. **Razer's own logs.** Synapse 4 logs every command it sends, with a human-readable name *and* the raw
   bytes, under `%LOCALAPPDATA%\Razer\RazerAppEngine\User Data\Logs`. Mining 130 MB of those logs
   ([`tools/reference/mine_synapse_logs.py`](https://github.com/justinthevoid/uncoil/blob/main/tools/reference/mine_synapse_logs.py)) produced the
   catalog above: 30 commands and 116 distinct input events, no USB sniffer required.
3. **Live probing** of real hardware with small Python scripts (`tools/reference/`), reading back status
   codes and asking a human what the LEDs actually did.

## Transport

One 90-byte **feature report**, written with report id `0` (91 bytes through hidapi).

| offset | field | notes |
|---:|---|---|
| 0 | status | `0x00` new, `0x01` busy, `0x02` ok, `0x03` fail, `0x04` timeout / no answer, `0x05` unsupported |
| 1 | transaction id | per device: `0x1F` keyboard + mouse, `0x3F` Goliathus mat |
| 2–3 | remaining packets | 0 |
| 4 | protocol type | 0 |
| 5 | data size | number of argument bytes |
| 6 | command class | |
| 7 | command id | high bit set = "get" |
| 8–87 | arguments | up to 80 bytes |
| 88 | CRC | XOR of bytes 2–87 |
| 89 | reserved | |

Each device answers on one specific HID collection (interface + usage page + usage); see [Devices](/docs/devices/).

## Commands used by uncoil

| class/id | name | args |
|---|---|---|
| `00/04` | Set Device Mode | `[mode, 0]` — `0x00` normal (firmware runs Fn, media keys, dial, DPI buttons), `0x03` driver (host must) |
| `00/84` | Get Device Mode | |
| `0F/02` | Set effect (extended matrix) | `[storage, led, effect, …]` — `0x08` custom frame, `0x04` wave `[dir, speed]` |
| `0F/03` | Custom frame row | `[0, 0, row, start_col, stop_col, r,g,b, …]` |
| `17/00` | Set OLED command dial active mode | `[profile, mode, display_order, enabled_functions]` |
| `17/03` | Set OLED brightness | `[percent]` |

See the catalog for battery, firmware, OLED home screen, screensaver, macro storage, etc.

## Input reports (keyboard, vendor collection)

| report | meaning |
|---|---|
| `2` | media keys |
| `4` | "Razer keys": up to 7 held Razer key codes. Fn = `1`; dial click `82`; next `83`; prev `84`; play/pause `85`; dial `96` |
| `5` | hardware events `[5, event, value…]`: `81` OLED mode (0 normal, 1 home, 2 command dial, 3 driver, 4 Fn, 5 off, 6 BLE pairing, 7 low battery), `82` OLED menu, `85` OLED low power, `49` battery (`value*100/255` %), `9` wireless connect/disconnect, `57` scroll free/notch |

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
  side of the board. [`devices/razer-blackwidow-v4-pro-75.toml`](https://github.com/justinthevoid/uncoil/blob/main/devices/razer-blackwidow-v4-pro-75.toml) maps every slot to its physical position.
- **The HyperSpeed dongle answers `0x04`** while the mouse is on its cable; treat that as "not here yet".
- **OpenRGB 1.0's SDK server** accepted per-LED updates for these devices but never pushed them to the
  hardware, leaving animations frozen; uncoil drives Razer devices directly.

## Onboard key mappings (the Fn layer lives in the keyboard)

The BlackWidow stores a two-layer key map per profile in its own memory: **Normal** and **Hypershift**.
On this keyboard Hypershift *is* the Fn layer, so "Synapse-only" Fn combinations are simply empty or
"send a Razer key to the host" entries. Write a real key code into the Hypershift layer and the firmware
handles the combination itself: no driver mode, no daemon, works on any PC.

| class/id | name | args → reply |
|---|---|---|
| `05/8A` | max profiles | → `[5]` |
| `05/80` | profile count | → `[n]` |
| `05/81` | profile ids | → `[count, ids…]` |
| `02/8D` | **get key mapping** | `[profile, key, layer]` → `[profile, key, layer, fn, len, data…]` |
| `02/0D` | **set key mapping** | `[profile, key, layer, fn, len, data…]` |
| `02/8F` | bulk get | `[profile, key, layer]` → packed 8-byte records |

- `layer`: 0 Normal, 1 Hypershift (Fn).
- `fn` (function id, shared with Razer mice): 0 off, 1 mouse button, 2 **key code** `[modifier bits, HID usage]`,
  7 profile, 10 media key, 12 Hypershift/mode key, 17 **Razer key** `[code]` (reported to the host in report 4), …
- `key`: Razer key ids, e.g. 26 = P, 76 = Delete, 120–123 = F9–F12, 124 = Print Screen
  (full table: [`blackwidow-keyids.json`](https://github.com/justinthevoid/uncoil/blob/main/docs/blackwidow-keyids.json), mined from Synapse's logs).

Verified on hardware (2026-10-03): Fn+Delete reads `17 [76]` (exactly the `04 01 4C` seen in a driver-mode
capture), Fn+F9 reads `17 [4]`, and Fn+P read `2 [0, 0]` — an empty key code, which is why Fn+P (Print Screen on
the keycap) does nothing without Synapse. Writing `02/0D [1, 26, 1, 2, 2, 0, 0x46]` made Fn+P a native Print Screen.
Tool: [`tools/reference/obm_set_fnp.py`](https://github.com/justinthevoid/uncoil/blob/main/tools/reference/obm_set_fnp.py). The mouse equivalent (`02/8C` / `02/0C`, Basilisk/Viper) is documented by
[OpenSynapse](https://github.com/A1mAssist/OpenSynapse) (MIT); the keyboard variant answers `0x05` to those.

## Driver mode, measured

With the keyboard in driver mode (`00/04 [3]`), the dial still sends ordinary consumer volume codes (`E9`/`EA`,
press = mute `E2`) and the firmware still runs its own Fn functions. Only Hypershift entries of type
"Razer key" are handed to the host, as report 4 (`04 01 <code>`: Fn held + code). A host-side Fn layer is
therefore only needed for actions the firmware can't express as a key code.

## Still to map

- Writing dial (command dial) functions and OLED images.
- Media-key function data layout (`fn` 10) for the keyboard.
