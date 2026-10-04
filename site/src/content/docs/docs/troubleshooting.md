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
  same Razer device will fight uncoil for it. Quit them. uncoil looks for Synapse, Razer's Chroma SDK
  services, OpenRGB and SignalRGB itself: the app shows a notice, `uncoil status` prints it, and the log says
  `… is running and may fight uncoil over the devices`.
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
   process, an OpenRGB of your own with its Razer devices on). uncoil's notice about other programs, in the
   app or `uncoil status`, names the ones it recognises.
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
- **"another program is talking to … right now":** uncoil shares a device lock with OpenRGB (and, apparently,
  Razer's software) and waits at most 25 ms for it. A command that can't get it in two tries fails with this
  message; try again, or quit the other program. Lighting frames that can't get it
  are skipped, so heavy contention shows as stutter.
- **OpenRGB:** off by default. Set [`openrgb.mode`](/docs/configuration/#openrgb) (or use the app's Settings
  page) and install with `-OpenRgb`. In `hardware` mode the **uncoil-openrgb** task runs OpenRGB once at
  sign-in to put the devices you listed in `openrgb.devices` on their own hardware modes, then OpenRGB exits.
  In `live` mode it keeps OpenRGB running as uncoil's SDK server on `127.0.0.1`, with every Razer detector
  off, and the daemon streams the desk effect to it. Either way it closes a running OpenRGB in your session
  first. Don't run your own OpenRGB against the Razer devices as well; in testing, OpenRGB 1.0's SDK server
  accepted per-LED updates for them but never pushed them to the hardware. To turn it off, set
  `"mode": "off"`, or install again without `-OpenRgb` to remove the task. Turning live off leaves the PC's
  lights as they were last set.
- **Did it run?** The task writes no log. Open Task Scheduler, find **uncoil-openrgb** and read its status
  and **Last Run Result**. In live mode it should be **Running**. Otherwise: `0` ran and exited cleanly, `1`
  turned off or nothing configured (in hardware mode, nothing in `openrgb.devices`), `2` failed (OpenRGB is not
  installed, `%ProgramData%\uncoil\openrgb` is missing or not administrators-only, no entry was a plain name,
  `openrgb.live.port` is outside 1024–65535, or OpenRGB's server stopped with an error). Reinstalling with
  `-OpenRgb` recreates the folder. The task runs at sign-in, so a mode you just turned on starts at the next
  one.
- **Live mode shows "Waiting for OpenRGB's SDK server":** the daemon found nothing listening on
  `127.0.0.1` at `openrgb.live.port`. Check the task is running (above), and that OpenRGB is installed in
  `C:\Program Files\OpenRGB`. RAM and many motherboards also need PawnIO, OpenRGB's SMBus driver; see
  OpenRGB's instructions. A device missing from the PC column may be excluded by `openrgb.live.exclude`,
  hidden in OpenRGB, or have "Razer" in its name or vendor (those are always left to uncoil).
- **iCUE:** RAM lighting shares the SMBus with iCUE, so uncoil skips the RAM, in both OpenRGB modes, while
  iCUE is running (live mode checks when it connects to OpenRGB). Corsair devices themselves are iCUE's
  business.

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
timer, scroll wheel) are refused in these cases, with a message saying why:

- **`check_failed`:** the device is experimental, or the feature isn't confirmed on it yet, and the read-only
  check didn't pass: the device didn't answer the way its file expects. `uncoil check <device>` shows what was
  read. Reads and lighting still work. Please [report it](https://github.com/justinthevoid/uncoil/issues/new?template=device_report.yml).
  A device file you added to `%APPDATA%\uncoil\devices` counts as experimental unless it sets `support`.
- **`left_click_guard`:** the change would leave the mouse with no button that left-clicks. Map another button
  to left click first.
- **`not_supported`:** the device file doesn't list that feature.
- **"another program is talking to … right now":** another program held the shared device lock. Try again
  (see [iCUE, OpenRGB and SignalRGB](#icue-openrgb-and-signalrgb)).
- On the command line, a command that writes device memory is refused without `--write` (`keymap import`
  without it shows what differs from the file instead).

## Still stuck

Open an issue on [GitHub](https://github.com/justinthevoid/uncoil/issues) with the last 50 lines of
`uncoild.log`, your `status.json`, and the device models. The log contains no keystrokes or personal data,
but read it before posting anyway.
