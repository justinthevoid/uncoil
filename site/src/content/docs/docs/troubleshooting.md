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
  `%LOCALAPPDATA%\uncoil\bin\uncoild.exe` directly to rule out the task.
- **Is something else holding the devices?** Synapse, OpenRGB, SignalRGB or another lighting app driving the
  same Razer device will fight uncoil for it. Quit them.
- **Is the device supported?** Only the [listed devices](/docs/devices/) are driven. A missing line like
  `opened Razer Basilisk V3 Pro (00AA, wired)` in the log means uncoil didn't find a matching USB endpoint.
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
- **OpenRGB:** uncoil only uses it once, at start, to put non-Razer hardware (motherboard, GPU, RAM) on its own
  hardware rainbow, then OpenRGB exits. Don't also run OpenRGB's SDK server against the Razer devices; in
  testing, OpenRGB 1.0's SDK server accepted per-LED updates for them but never pushed them to the hardware.
  To turn the hand-off off, set `"openrgb_hardware_rainbow": false` and restart the task.
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

## Still stuck

Open an issue on [GitHub](https://github.com/justinthevoid/uncoil/issues) with the last 50 lines of
`uncoild.log`, your `status.json`, and the device models. The log contains no keystrokes or personal data,
but read it before posting anyway.
