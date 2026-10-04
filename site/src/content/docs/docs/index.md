---
title: uncoil documentation
description: How to install, configure and extend uncoil, the small open-source lighting daemon for Razer peripherals on Windows.
tableOfContents: false
---

uncoil is a background daemon, `uncoild`, plus an optional desktop app and an `uncoil` command line. The
daemon drives the lighting on supported Razer keyboards, mice and mats from one shared effect, fades with the
display, and leaves each device's firmware in charge of its keys; through the app or the CLI it also remaps
keys and buttons and sets the dial, the screen and mouse settings in the device's own memory. It is one
1.4 MB executable that runs as you, unelevated; it used about 3 MB of RAM and under 1% of one core on the
maintainer's PC (measured on an earlier build).

uncoil is pre-release: nothing has been released yet, so installing means building from source.

Not affiliated with or endorsed by Razer Inc.

## Use

- **[Getting started](/docs/getting-started/)**: Build, install, retire Synapse without leaving the keyboard in driver mode, first run, uninstall.
- **[Configuration](/docs/configuration/)**: Every key in `%APPDATA%\uncoil\config.json`, every effect, with defaults.
- **[Troubleshooting](/docs/troubleshooting/)**: Frozen lighting, a dial that scrolls, other RGB software, display sleep, settings that won't save.
- **[FAQ](/docs/faq/)**: Licence, what still needs Synapse, what uncoil deliberately doesn't do.

## Hardware

- **[Devices](/docs/devices/)**: What is supported, what is experimental, and how to add a device with one TOML file.
- **[Protocol](/docs/protocol/)**: The HID report format, the commands uncoil uses, and the quirks that cost the most time.

## Project

- **[Architecture](/docs/architecture/)**: The daemon, the control pipe, the app and the CLI, and what's in progress.
- **[Contributing](/docs/contributing/)**: Device files, captures, code.
