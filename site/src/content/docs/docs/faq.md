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
[OpenRGB](https://gitlab.com/CalcProgrammer1/OpenRGB), both GPL-2.0.

## How light is it, really?

Measured on the maintainer's PC (a Ryzen desktop with a BlackWidow V4 Pro 75%, Basilisk V3 Pro and Goliathus
Chroma Extended):

| | Razer Synapse 4 | uncoild |
|---|---|---|
| Processes | 17 | 1 |
| Memory | ~1.4 GB at start, leaking to several GB over days | ~3 MB |
| CPU, idle animation | ~7% of one core | under 1% of one core |
| On disk | ~500 MB | 651 KB, one executable |
| Kernel drivers | yes | none |

That is one machine. uncoild measures itself and writes the numbers to `%LOCALAPPDATA%\uncoil\status.json`,
so you can check yours.

## What still needs Synapse?

What uncoil doesn't do yet:

- macros and DPI settings;
- custom images on the BlackWidow's dial screen;
- firmware updates;
- devices that aren't on the [list](/docs/devices/).

Key and button remapping (normal and Fn layers) works from the app and the CLI. Only one remap, Fn+P to Print
Screen, has been checked on a real keyboard so far; see the
[protocol](/docs/protocol/#onboard-key-mappings-the-fn-layer-lives-in-the-keyboard).

Anything already stored in a device's own memory stays there after Synapse is uninstalled. If you need one of
the above occasionally, you can install Synapse, make the change, quit it properly, and uninstall it again.

## Will it support my device?

If it is a Razer device that OpenRGB or OpenRazer already knows, probably, and adding it is mostly a
[data file](/docs/devices/#adding-a-device). Anything else is out of scope: uncoil drives Razer devices, and
hands the rest of the PC to OpenRGB once at start.

## Does it collect anything?

No. There is no account, no telemetry, no network access and no updater. It reads its config and device files,
writes a status file and a small log under your profile, and talks to USB devices.

## Why is the logon task elevated?

Only for the optional OpenRGB hand-off, which needs administrator rights to reach RAM lighting over SMBus.
uncoild's own device access is plain user-mode HID.

## Windows only?

Yes, Windows 10 and 11. On Linux, [OpenRazer](https://openrazer.github.io/) already does this job well.

## Why "uncoil"?

It unwinds the things you didn't ask to have running. Make of the snake what you like.
