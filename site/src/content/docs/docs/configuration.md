---
title: Configuration
description: Every key in %APPDATA%\uncoil\config.json, its default, and what the daemon does with it.
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
  "openrgb_hardware_rainbow": false,
  "openrgb": { "devices": [] }
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
| `openrgb_hardware_rainbow` | boolean | `false` | Older switch for the OpenRGB hand-off; `true` means `openrgb.mode` `"hardware"` when that isn't set. See [below](#openrgb). |
| `openrgb` | object | `{ "devices": [] }` | What uncoil does with OpenRGB for the motherboard, GPU and RAM: off, a hand-off at sign-in, or live. See [below](#openrgb). |

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

### `breathing`

Fades in and out.

| Key | Default | Meaning |
|---|---|---|
| `colors` | `[]` | No colours: a new rainbow hue each breath. One: that colour. More: they take turns. Each is `[r, g, b]`. |
| `period_s` | `4` | Seconds per breath. Minimum 0.5. |

### `starlight`

Random LEDs twinkle. Every device shares one field, so the twinkles are spread across the whole desk.

| Key | Default | Meaning |
|---|---|---|
| `colors` | `[]` | Colours to pick from; none means random hues. |
| `density` | `0.15` | Share of LEDs lit at once, 0–1. |
| `twinkle_s` | `1.5` | Seconds per twinkle. Minimum 0.1. |

### `fire`

Flames rising from the front edge of the desk.

| Key | Default | Meaning |
|---|---|---|
| `speed` | `1` | 0.25–3. |
| `height` | `0.5` | Flame height as a share of the desk's depth, 0.05–1. |

### `wheel`

A rainbow turning around a centre point.

| Key | Default | Meaning |
|---|---|---|
| `period_s` | `6` | Seconds per turn. Minimum 0.5. |
| `reverse` | `false` | Turn the other way. |
| `center` | `null` | `[x, y]` in desk key units; `null` is the keyboard's centre. |

### `reactive` and `ripple`

`reactive` lights a key when it is pressed and fades it; `ripple` sends a ring across the desk from each
pressed key. While either is in use (on its own or in a Studio layer), uncoild listens for key presses and
turns each one into a desk position straight away; it never records which key it was. See
[SECURITY.md](https://github.com/justinthevoid/uncoil/blob/main/SECURITY.md#key-presses-and-audio-level-reactive-and-audio-effects).

| Key | Default | Meaning |
|---|---|---|
| `color` | `null` | `[r, g, b]`, or `null` for a new rainbow hue per press. |
| `fade_s` | `1` | Seconds a press takes to fade. |
| `speed` | `12` | `ripple` only: ring speed in key units per second. |
| `width` | `1.5` | `ripple` only: ring width in key units. |

### `audio_meter`

The desk fills left to right with the system audio peak level, green to yellow to red. uncoild reads only
Windows' peak level for the default playback device, one number, never the audio itself.

| Key | Default | Meaning |
|---|---|---|
| `sensitivity` | `1` | Multiplies the level before it is drawn. |

### `studio`

Layers of effects, bottom first. Each enabled layer is blended over the ones below it where its mask covers
an LED. Reactive, ripple, starlight and the audio meter are transparent where they are dark, so they can sit
on top of another effect.

```json
"effect": {
  "kind": "studio",
  "layers": [
    { "name": "Base", "effect": { "kind": "wave" }, "mask": { "kind": "all" } },
    { "name": "Mouse", "opacity": 0.5, "effect": { "kind": "static", "color": [255, 96, 0] },
      "mask": { "kind": "devices", "ids": ["razer-basilisk-v3-pro"] } },
    { "name": "WASD", "effect": { "kind": "reactive" },
      "mask": { "kind": "keys", "device": "razer-blackwidow-v4-pro-75", "shapes": ["W", "A", "S", "D"] } }
  ]
}
```

| Layer key | Default | Meaning |
|---|---|---|
| `name` | `""` | Shown in the app. |
| `enabled` | `true` | A disabled layer is skipped. |
| `opacity` | `1` | 0–1. |
| `effect` | required | Any effect above except `studio`. |
| `mask` | `{ "kind": "all" }` | `all`, `devices` with `ids`, `keys` with a `device` id and `shapes` (key and LED names from the device file), or `lights` with `[device, shape]` pairs on any devices, e.g. `{ "kind": "lights", "lights": [["razer-blackwidow-v4-pro-75", "W"], ["razer-basilisk-v3-pro", "Logo"]] }`. The app's Lighting page writes a whole-desk layer plus `devices` and `lights` layers when you give devices their own effect. |

Colours for `wave`, `spectrum` and the other rainbow effects come from FastLED's "rainbow" hue map rather
than a plain HSV wheel. It is tuned for real LEDs, so no band of the rainbow looks wider or brighter than the
rest.

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

A connected device the config doesn't place, beyond the first of its kind, is put next to the others of its
kind (a second keyboard below the first, another mouse to the right). Devices without lighting never appear
on the desk.

To move the mouse a little further right:

```json
"desk": { "razer-basilisk-v3-pro": { "x": 22.5, "y": 3.1 } }
```

Device ids are the `id` in each [device file](/docs/devices/). Because the wave is computed from these
positions, a correct desk layout is what makes the bands line up from one device to the next.

In [live OpenRGB mode](#live) the PC's OpenRGB devices join the desk too, with ids `openrgb:<name>` (the
device's name in lower case, other characters as `-`, for example `openrgb:asus-rog-strix-b550-f-gaming-wi-fi`;
a second device with the same name gets `-2`). Unplaced, they stack in a column left of the keyboard. To
move one, give its centre like any other device:

```json
"desk": { "openrgb:corsair-vengeance-pro-rgb": { "x": -6, "y": 2 } }
```

The app's Devices page lists them by name under "Through OpenRGB"; `uncoil --json status` gives their ids
under `openrgb.devices`.

## `openrgb`

Off by default. uncoil drives Razer devices itself; for the rest of the PC (motherboard, GPU, RAM) it can use
[OpenRGB](https://openrgb.org). `openrgb.mode` says how:

| `mode` | What happens |
|---|---|
| `"off"` | Nothing. The default. |
| `"hardware"` | At sign-in, OpenRGB runs once to put each device in `openrgb.devices` on its own built-in (hardware) effect, then exits. uncoil doesn't drive those devices. |
| `"live"` | OpenRGB keeps running in the background as a local SDK server, and uncoil sends it the desk effect, so the motherboard, RAM and GPU follow your Razer devices. |

Without `mode`, the older `"openrgb_hardware_rainbow": true` means `"hardware"`; when both are there, `mode`
wins. The app's Settings page sets `mode` for you.

Both modes need:

1. OpenRGB installed at `C:\Program Files\OpenRGB\OpenRGB.exe`. For RAM and many motherboards OpenRGB also
   needs PawnIO, its SMBus driver on Windows; see OpenRGB's own instructions.
2. The `-OpenRgb` install (`scripts\install-task.ps1 -OpenRgb`), which registers the elevated
   **uncoil-openrgb** task and the administrators-only folder OpenRGB keeps its settings in
   (`%ProgramData%\uncoil\openrgb`). RAM lighting sits on the SMBus, which needs administrator rights, so the
   unelevated daemon can't do this itself. (A daemon installed with `-Elevated` runs the hand-off or the
   server itself at start.)

Either way, if OpenRGB is already running in your session, it is closed first, and devices that another
running program lights itself are left to it (see [Programs that light PC parts](#programs-that-light-pc-parts)). An OpenRGB it can't close, such as OpenRGB's own
Windows service, makes live mode stop there (result `2`) rather than start a second server beside it: two
OpenRGBs open the same devices and the lights flicker between them. Stop that service and set it to Manual.

The task runs at sign-in, so turning a mode on, or switching between `hardware` and `live`, takes effect at
your next sign-in (with `-Elevated`, when the daemon restarts). It writes no log; its result is the task's
**Last Run Result** in Task Scheduler: `0` ran and exited cleanly, `1` turned off or nothing configured, `2`
failed (see [Troubleshooting](/docs/troubleshooting/#icue-openrgb-and-signalrgb)). In live mode the task
stays **Running** for as long as OpenRGB does.

### Hardware

```json
"openrgb": {
  "mode": "hardware",
  "devices": [
    { "match": "ASUS", "mode": "rainbow" },
    { "match": "Vengeance", "mode": "rainbow wave", "ram": true }
  ]
}
```

`openrgb.devices` lists what to hand off; there is no built-in device list, so nothing happens until you add
an entry.

| Key | Meaning |
|---|---|
| `match` | Part of the device name as OpenRGB lists it (passed to `OpenRGB.exe -d`). |
| `mode` | The OpenRGB mode to put it in (passed to `-m`). |
| `ram` | `true` for RAM on the SMBus: skipped while a program that lights RAM runs, such as iCUE. Default `false`. |

`match` and `mode` must be plain names: 1 to 64 letters, digits, spaces and `-_.()+#&:/`, not starting with
`-` or a space and not ending with a space. Anything else is skipped, so a config entry can never become an
OpenRGB option.

### Live

```json
"openrgb": {
  "mode": "live",
  "live": { "port": 6742, "exclude": ["Vengeance"] }
}
```

| Key | Default | Meaning |
|---|---|---|
| `live.port` | `6742` | The SDK server's port on `127.0.0.1`, 1024–65535. The task starts OpenRGB on it and the daemon connects to it. |
| `live.exclude` | `[]` | OpenRGB devices to leave to their own software: any whose name contains one of these, ignoring case. The app's PC page adds and removes them ("Leave it to its own software"). |

uncoil drives every device OpenRGB finds except Razer devices (anything with "Razer" in its name or vendor,
so also products like the Lian Li O11 Dynamic Razer Edition case), hidden ones, ones with no LEDs, the ones
`exclude` matches, and the ones another running program lights itself. The OpenRGB it starts has every Razer
detector turned off.

`exclude` also turns off every OpenRGB detector whose name it matches, so OpenRGB doesn't open those devices
at all and their own software can take them back: excluding `"Corsair iCUE Link System Hub"` hands the
iCUE Link fans and pump back to iCUE. Detector names are often, not always, the device's name (a motherboard
is usually found by a detector such as "ASUS Aura Motherboard"); a device with no matching detector is just
not sent frames, and keeps the colour it last had until something else sets it. When an exclusion turns a
detector off or back on, the **uncoil-openrgb** task restarts OpenRGB within a couple of seconds, so the
PC's lights pause briefly. uncoil turns back on only the detectors it turned off; ones you turned off in
OpenRGB stay off. OpenRGB lists its detectors in its settings file on its first run, so there is nothing to
match before that.

The devices appear on the [desk](#desk) as a "PC" column left of the keyboard, in the order motherboards,
RAM, GPUs, the rest, so the effect reaches them the way it reaches a mouse or a mat. Move them with `desk` if
your case sits elsewhere. Brightness, saturation, the frame rate (at most 30 for OpenRGB devices) and the
display fade apply to them too.

Things to know:

- **The SDK server has no password.** While it runs, any program on this PC can change the motherboard, RAM
  and GPU lighting through it; that is how OpenRGB's SDK works. uncoil starts it listening on `127.0.0.1`
  only, so other machines can't reach it. See
  [SECURITY.md](https://github.com/justinthevoid/uncoil/blob/main/SECURITY.md).
- **Turning live off** stops uncoil sending frames straight away, and the **uncoil-openrgb** task closes
  OpenRGB within a couple of seconds, so iCUE or the maker's app can take the devices back. Until one does,
  the PC's lights stay as they were last set. Turning live on again takes effect at your next sign-in.
- **Only uncoil's own server is driven.** If something else answers on the port (OpenRGB's Windows service,
  or an OpenRGB you started), the Settings page shows an error instead of driving it: that OpenRGB has the
  Razer devices and your exclusions on.
- The app's Settings page shows the connection: connected with the number of devices, waiting for the
  server, or the error.

### Programs that light PC parts

Some programs light parts of the PC themselves. While one of them runs, uncoil leaves it the devices it
claims, in both modes, and lights them again a few seconds after it quits:

| Program | Seen as | Leaves it |
|---|---|---|
| Corsair iCUE | `iCUE.exe` | Corsair devices, and all RAM (iCUE writes the bus every stick shares) |
| ASUS Armoury Crate | `LightingService.exe` | ASUS devices and RAM. Its lighting service runs whenever Armoury Crate is installed. |
| MSI Mystic Light | `LEDKeeper2.exe` | MSI devices |
| SignalRGB | `SignalRgb.exe` | everything |

The app's PC page lists those devices under **Run by other software**, with the program that has them. In
live mode uncoil stops sending them frames, and the **uncoil-openrgb** task restarts OpenRGB with the
detectors that find them turned off, so OpenRGB lets go of them too (the PC's other lights pause for a second
or two). When the program quits, the detectors go back on and OpenRGB restarts again. Both wait for two
looks a couple of seconds apart, so a program that is starting up causes one restart, not several.

Detectors are matched by name: one starting with the make ("Corsair …", "ASUS …"), and for RAM any with
"DRAM" in its name. A GPU detector names only its card, so a GPU from another make keeps its detector even
when its program holds it; uncoil still doesn't send it frames. A program that isn't listed here, or one
that lights more than its own make (iCUE with its motherboard plugin, say), is not noticed: use `exclude`
for those devices.

## The app's own settings

The desktop app keeps its preferences in a separate file, `%APPDATA%\uncoil\app.json`, which the engine
ignores. The app's **Tray & notifications** page edits it:

| Key | Default | Meaning |
|---|---|---|
| `close_to_tray` | `false` | Closing the window hides it in the tray instead of quitting. |
| `start_in_tray` | `false` | Start hidden in the tray when you sign in: a per-user startup entry (the `Run` key under your account) launches `uncoil-app.exe --tray`. Also keeps the app in the tray when the window closes. |
| `battery_notifications` | `true` | Notify when a wireless device's battery is low, and when charging reaches 100 %. Checked every 10 minutes while the app is open or in the tray. |
| `battery_threshold` | `20` | Percent for the first low-battery notification, 15–50; a second one comes at 10 %. |

## Other files

| Path | Written by | Contents |
|---|---|---|
| `%APPDATA%\uncoil\config.json` | you, the app | Settings (this page). |
| `%APPDATA%\uncoil\app.json` | the app | The app's own preferences ([above](#the-apps-own-settings)); the engine never reads it. |
| `%APPDATA%\uncoil\devices\*.toml` | you | Extra or overriding [device definitions](/docs/devices/#adding-a-device), read at start. Experimental unless the file sets `support`; files over 1 MB are skipped. |
| `%LOCALAPPDATA%\uncoil\status.json` | uncoild, every 2 s | Running devices, fps, retries, errors, and the daemon's own memory, CPU and size. |
| `%LOCALAPPDATA%\uncoil\uncoild.log` | uncoild | Device opens and losses, display changes, reloads, writes to device memory. Trimmed past 256 KB. |
| `%LOCALAPPDATA%\uncoil\onboard-writes.jsonl` | uncoild | One line per write to a device's own memory. `keymap reset` uses it to restore a key's value from before uncoil first wrote it. |
| `%ProgramFiles%\uncoil\uncoild.exe` | install script | The installed daemon (`install.log` next to it says what the last install did). |
| `%ProgramData%\uncoil\openrgb` | OpenRGB, the uncoil-openrgb task | OpenRGB's settings for the hand-off or the live server; administrators only. In live mode the task writes `OpenRGB.json` there (Razer detectors off, server on `127.0.0.1`). Created by `-OpenRgb` or `-Elevated` installs. |
