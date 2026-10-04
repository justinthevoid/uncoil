---
title: FAQ
description: Who makes uncoil, the licence, what still needs Synapse, and what uncoil deliberately leaves out.
---

## Is uncoil made by Razer?

No. It is an independent open-source project, not affiliated with, endorsed by or sponsored by Razer Inc.
"Razer", "Synapse" and "Chroma" are trademarks of Razer Inc., used here only to say which hardware and software
uncoil works with.

## What is the licence?

GPL-3.0-or-later. Free to use, study, change and share; changes you distribute stay under the same licence.
Protocol facts and some device data derive from [OpenRazer](https://github.com/openrazer/openrazer) and
[OpenRGB](https://gitlab.com/CalcProgrammer1/OpenRGB), both GPL-2.0-or-later.

## How light is it, really?

Measured on the maintainer's PC (a Ryzen desktop with a BlackWidow V4 Pro 75%, Basilisk V3 Pro and Goliathus
Chroma Extended):

| | Razer Synapse 4 | uncoild |
|---|---|---|
| Processes | 17 | 1 |
| Memory | ~1.4 GB at start, leaking to several GB over days | ~3 MB |
| CPU, idle animation | ~7% of one core | under 1% of one core |
| On disk | ~500 MB | 1.3 MB, one executable |
| Kernel drivers | yes | none |

That is one machine. Memory and CPU were measured on an earlier build; the size is the build of 2026-10-03
(1,257,472 bytes).
uncoild measures itself and writes the numbers to `%LOCALAPPDATA%\uncoil\status.json` (`uncoil status` prints
them), so you can check yours.

## What still needs Synapse?

What uncoil doesn't do yet:

- macros;
- what each command-dial mode does, and custom images on the BlackWidow's dial screen (uncoil sets the active
  mode and the screen's brightness);
- switching onboard profiles;
- firmware updates;
- devices that aren't on the [list](/docs/devices/).

Key and button remapping (normal and Fn layers) works from the app and the CLI. Only one remap, Fn+P to Print
Screen, has been checked on a real keyboard so far; see the
[protocol](/docs/protocol/#onboard-key-mappings-the-fn-layer-lives-in-the-keyboard). DPI, DPI stages, poll
rate, battery and the sleep timer are there too, from OpenRazer's documented commands, but not yet confirmed on
real hardware: uncoil reads each one first and won't change it unless the read makes sense.

Anything already stored in a device's own memory stays there after Synapse is uninstalled. If you need one of
the above occasionally, you can install Synapse, make the change, quit it properly, and uninstall it again.

## Will it support my device?

If it is a Razer device that OpenRGB or OpenRazer already knows, probably, and adding it is mostly a
[data file](/docs/devices/#adding-a-device). 29 such devices already have an
[experimental](/docs/devices/#experimental) file waiting for someone with the hardware to confirm it. Anything else is out of scope: uncoil drives Razer devices, and
can hand the rest of the PC to OpenRGB once at logon if you set that up.

## Does it collect anything?

No. There is no account, no telemetry, no network access and no updater. It reads its config and device files,
writes a status file, a small log and a record of writes to device memory under your profile, talks to USB
devices, and answers the app and the CLI over a local named pipe that only your user account can open.

## Does it need administrator rights?

Once, to install: the executable goes to `%ProgramFiles%\uncoil`, where only administrators can write. The
daemon itself runs as you, unelevated; its device access is plain user-mode HID. Only the optional OpenRGB
hand-off runs elevated, from its own one-shot task (`install-task.ps1 -OpenRgb`), because RAM lighting sits on
the SMBus, which needs administrator rights. `-Elevated` runs the whole daemon elevated, as a fallback for a
PC where it cannot open its devices otherwise.

## Windows only?

Yes, Windows 10 and 11. On Linux, [OpenRazer](https://openrazer.github.io/) already does this job well.

## Why "uncoil"?

It unwinds the things you didn't ask to have running. Make of the snake what you like.
