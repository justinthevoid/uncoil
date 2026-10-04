---
title: Troubleshooting
description: Frozen keyboard lighting, a dial that scrolls instead of changing volume, conflicts with iCUE or OpenRGB, and lighting around display sleep.
---

Start with the log. `%LOCALAPPDATA%\uncoil\uncoild.log` records every device opened or lost, display state
changes and config reloads, with a Unix timestamp on each line:

```powershell
Get-Content "$env:LOCALAPPDATA\uncoil\uncoild.log" -Tail 30
```

`%LOCALAPPDATA%\uncoil\status.json` shows what is running right now: each device's fps, `busy` retries and
errors.

## Nothing lights up

- **Is it running?** `Get-Process uncoild`. If not, `Start-ScheduledTask uncoil`, or run
  `%ProgramFiles%\uncoil\uncoild.exe` directly to rule out the task.
- **Is something else holding the devices?** Synapse, OpenRGB, SignalRGB or another lighting app driving the
  same Razer device will fight uncoil for it. Quit them.
- **Is the device supported?** Only the [listed devices](/docs/devices/), supported and experimental, are
  driven. A missing line like `opened Razer Basilisk V3 Pro (00AA, wired)` in the log means uncoil didn't find
  a matching USB endpoint. A Razer device uncoil has no file for is logged once by product ID and listed by
  `uncoil status`; that is a good start for a [device support request](https://github.com/justinthevoid/uncoil/issues/new?template=device_support.yml).
- **Is the effect `off`, or `brightness` 0?** Check `%APPDATA%\uncoil\config.json`.

## Keyboard lighting froze on one frame

Two firmware quirks on the BlackWidow V4 Pro 75% cause exactly this, and uncoil works around both:

- Re-sending the custom-frame effect command every frame freezes the keyboard on the first frame. uncoil sends
  it once per connection.
- If frame rows are streamed without reading each reply, the keyboard silently stops applying them after a
  while, even though every status says "ok". uncoil reads every reply for this keyboard.

If you see a freeze anyway:

1. Make sure no other program is also writing to the keyboard (another lighting app, a leftover Synapse
   process, OpenRGB's SDK server).
2. Restart the task: `Stop-ScheduledTask uncoil; Start-ScheduledTask uncoil`.
3. Unplug and replug the keyboard; uncoil picks it up again within 5 seconds.

A growing `errors` count for the device in `status.json` points at the USB connection (hub, front-panel port,
cable) rather than uncoil.

## The dial scrolls instead of changing volume

The keyboard is in **driver mode**, where the host is expected to handle some keys. Synapse puts devices in
driver mode, and if it is killed rather than quit it never switches them back; the dial then scrolls and the
media keys go dead until the keyboard is power-cycled.

uncoil switches every device it drives back to normal mode when it connects, so this normally fixes itself as
soon as uncoild starts. If it doesn't:

1. Check Synapse really is gone: no `Razer` processes in Task Manager.
2. Unplug the keyboard and plug it back in. That always clears driver mode, with or without uncoil.

The same applies to Fn shortcuts that stopped working: in normal mode the firmware runs them itself.

## iCUE, OpenRGB and SignalRGB

- **Razer devices:** let one program drive them. uncoil talks to them directly over HID; two programs writing
  frames at once looks like flicker or a freeze.
- **OpenRGB:** the hand-off is off by default. When turned on (and installed with `-OpenRgb`), the
  **uncoil-openrgb** task runs OpenRGB once at logon to put the devices you listed in
  [`openrgb.devices`](/docs/configuration/#openrgb_hardware_rainbow) on their own hardware modes, then
  OpenRGB exits. It closes a running OpenRGB in your session first. Don't also run OpenRGB's SDK server
  against the Razer devices; in testing, OpenRGB 1.0's SDK server accepted per-LED updates for them but never
  pushed them to the hardware. To turn the hand-off off, set `"openrgb_hardware_rainbow": false`, or install
  again without `-OpenRgb` to remove the task.
- **Did the hand-off run?** The task writes no log. Open Task Scheduler, find **uncoil-openrgb** and read its
  **Last Run Result**: `0` done, `1` turned off or nothing in `openrgb.devices`, `2` failed (OpenRGB is not
  installed, `%ProgramData%\uncoil\openrgb` is missing or not administrators-only, or no entry was a plain
  name). Reinstalling with `-OpenRgb` recreates the folder.
- **iCUE:** RAM lighting shares the SMBus with iCUE, so uncoil skips the RAM in the OpenRGB hand-off while
  iCUE is running. Corsair devices themselves are iCUE's business.

## Lighting around display sleep

By design, lighting fades out when Windows turns the display off, dims to `dim_level` (35% by default) when
Windows dims it, and fades back in on wake. That is the behaviour of
[`display`](/docs/configuration/#display) in the config.

- **Lights stay on with the display off:** check `"off_when_display_off": true`. uncoil follows the
  *display* state, not the screensaver or the lock screen, so a screensaver on a display that is still on
  keeps the lights on.
- **Lights don't come back after sleep:** some devices reset while the PC sleeps. uncoil re-prepares every
  device on wake; if one stays dark, the log will say `re-prepare … after wake failed`. Replugging it, or
  restarting the task, brings it back.
- **The fade is too slow or too fast:** change `fade_s`.

## The mouse doesn't light up on its dongle

The Basilisk V3 Pro's HyperSpeed dongle answers "no answer" while the mouse is connected by cable, and
uncoil waits for it. Unplug the cable and the mouse is picked up on the dongle within 5 seconds.

## The app or `uncoil` can't reach the daemon

Both talk to uncoild over the named pipe `\\.\pipe\uncoil`. If they say the daemon is unreachable:

- Check it is running (`Get-Process uncoild`).
- Look for `control pipe … unavailable (…): another uncoild, or another program, holds it; exiting` in the
  log. The pipe is also uncoild's single-instance lock: something else (usually another uncoild, for example
  one started by hand next to the task) already holds `\\.\pipe\uncoil`, so this copy exited before opening
  any device. Close the other one (`Get-Process uncoild`) and restart the task.
- **`too many clients`:** uncoild serves at most 8 connections at once. Close a few `uncoil` commands or app
  windows and retry. A connection that sends nothing for 5 minutes is closed by the daemon.
- **`something else is serving the uncoil pipe`:** the process holding `\\.\pipe\uncoil` does not run as your
  user, so the app and CLI refuse to talk to it. Find it and stop it, then restart the task.
- The pipe only accepts your own user account, from this PC.

## A setting won't save

Changes stored in a device's own memory (key remaps, the dial, the screen, DPI stages, poll rate, sleep
timer) are refused in these cases, with a message saying why:

- **`check_failed`:** the device is experimental, or the feature isn't confirmed on it yet, and the read-only
  check didn't pass: the device didn't answer the way its file expects. `uncoil check <device>` shows what was
  read. Reads and lighting still work. Please [report it](https://github.com/justinthevoid/uncoil/issues/new?template=device_report.yml).
  A device file you added to `%APPDATA%\uncoil\devices` counts as experimental unless it sets `support`.
- **`left_click_guard`:** the change would leave the mouse with no button that left-clicks. Map another button
  to left click first.
- **`not_supported`:** the device file doesn't list that feature.
- On the command line, a command that writes device memory is refused without `--write` (`keymap import`
  without it shows what differs from the file instead).

## Still stuck

Open an issue on [GitHub](https://github.com/justinthevoid/uncoil/issues) with the last 50 lines of
`uncoild.log`, your `status.json`, and the device models. The log contains no keystrokes or personal data,
but read it before posting anyway.
