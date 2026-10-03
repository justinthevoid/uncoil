---
title: Configuration
description: Every key in %APPDATA%\uncoil\config.json, its default, and what the daemon does with it.
fac: FAC 202
---

uncoil reads one JSON file: `%APPDATA%\uncoil\config.json`. The desktop app writes it; you can edit it by
hand. uncoild watches the file's modification time and reloads it on change, so edits apply immediately.

:::note[Missing keys use defaults]
Every key is optional. Leave one out and its default applies. A file that isn't valid JSON (a stray comma, a
misspelt effect kind) is ignored as a whole and the defaults apply instead, so if the lighting snaps back to
the default wave after an edit, check the file's syntax first.
:::

## The default file

This is what uncoil runs when there is no config file at all:

```json title="%APPDATA%\uncoil\config.json"
{
  "effect": { "kind": "wave", "angle_deg": 35, "period_s": 14, "wavelength": 26, "reverse": false },
  "brightness": 1.0,
  "saturation": 1.0,
  "fps": 30,
  "display": { "off_when_display_off": true, "dim_level": 0.35, "fade_s": 1.2 },
  "desk": {},
  "openrgb_hardware_rainbow": true
}
```

## Top-level keys

| Key | Type | Default | Meaning |
|---|---|---|---|
| `effect` | object | wave | The effect every device samples. See [Effects](#effects). |
| `brightness` | number, 0–1 | `1.0` | Overall brightness. |
| `saturation` | number, 0–1 | `1.0` | Colour saturation; `0` is white light at the effect's brightness. |
| `fps` | integer | `30` | Frames per second sent to each device, clamped to 5–60. |
| `display` | object | see below | How lighting follows the display. See [Display](#display). |
| `desk` | object | `{}` | Where each device sits on the desk. See [Desk](#desk). |
| `openrgb_hardware_rainbow` | boolean | `true` | Hand non-Razer RGB to OpenRGB once at start. See [below](#openrgb_hardware_rainbow). |

## Effects

Effects are pure functions of desk position and time. Every device samples the same field, which is why a
wave crosses from the keyboard onto the mouse and mat without a seam. `kind` selects the effect.

### `wave`

Rainbow bands travelling across the desk at an angle.

| Key | Default | Meaning |
|---|---|---|
| `angle_deg` | `35` | Direction of travel. `0` sweeps left to right, `90` back to front. |
| `period_s` | `14` | Seconds for one full colour cycle to pass a point. Higher is slower. Minimum 0.5. |
| `wavelength` | `26` | Width of one full rainbow, in key units (1u = 19.05 mm). Minimum 1. |
| `reverse` | `false` | Run the wave the other way. |

```json
"effect": { "kind": "wave", "angle_deg": 0, "period_s": 8, "wavelength": 18 }
```

### `spectrum`

The whole desk cycles through the rainbow in unison.

| Key | Default | Meaning |
|---|---|---|
| `period_s` | `14` | Seconds per full cycle. Minimum 0.5. |

### `static`

One colour everywhere. `color` is required, as `[red, green, blue]` from 0 to 255; `brightness` still
applies.

```json
"effect": { "kind": "static", "color": [255, 96, 0] }
```

### `off`

All LEDs dark. The devices stay connected and keep their normal-mode functions.

```json
"effect": { "kind": "off" }
```

Colours for `wave` and `spectrum` come from FastLED's "rainbow" hue map rather than a plain HSV wheel. It is
tuned for real LEDs, so no band of the rainbow looks wider or brighter than the rest.

## Display

uncoil listens for Windows' console display state, the same signal Synapse uses.

| Key | Default | Meaning |
|---|---|---|
| `off_when_display_off` | `true` | Fade the lighting out when Windows turns the display off. |
| `dim_level` | `0.35` | Brightness multiplier while Windows has dimmed the display (0–1). |
| `fade_s` | `1.2` | Seconds for each fade. |

Once faded out, uncoil sends a few black frames and then idles; the devices hold the last frame. On wake it
re-prepares every device (some reset during sleep) and fades back in.

## Desk

Where each device sits, keyed by device id, in key units: `x` to the right, `y` toward you. For a keyboard
the point is the top-left of the key area; for every other device it is the device's centre. Devices not
listed use their defaults:

| Device | Default `x` | Default `y` |
|---|---|---|
| keyboard | `0` | `0` |
| mouse | `20.75` | `3.1` |
| mouse mat | `11.125` | `3.375` |

To move the mouse a little further right:

```json
"desk": { "razer-basilisk-v3-pro": { "x": 22.5, "y": 3.1 } }
```

Device ids are the `id` in each [device file](/docs/devices/). Because the wave is computed from these
positions, a correct desk layout is what makes the bands line up from one device to the next.

## `openrgb_hardware_rainbow`

When `true` and OpenRGB is installed at `C:\Program Files\OpenRGB\OpenRGB.exe`, uncoild runs it once at
start to put the motherboard, GPU and RAM on their own built-in rainbow effects, then OpenRGB exits. uncoil
does not drive those devices itself. If iCUE is running, the RAM is skipped, because RAM lighting shares the
SMBus with iCUE and writing it from two programs at once is a bad idea. The device names it targets are set in
`apps/uncoild/src/openrgb.rs` today.

This key is read at start only; restart the task after changing it.

## Other files

| Path | Written by | Contents |
|---|---|---|
| `%APPDATA%\uncoil\config.json` | you, the app | Settings (this page). |
| `%APPDATA%\uncoil\devices\*.toml` | you | Extra or overriding [device definitions](/docs/devices/#adding-a-device), read at start. |
| `%LOCALAPPDATA%\uncoil\status.json` | uncoild, every 2 s | Running devices, fps, retries, errors, and the daemon's own memory, CPU and size. |
| `%LOCALAPPDATA%\uncoil\uncoild.log` | uncoild | Device opens and losses, display changes, reloads. Trimmed past 256 KB. |
| `%LOCALAPPDATA%\uncoil\bin\uncoild.exe` | install script | The installed daemon. |
