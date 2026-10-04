---
title: uncoil documentation
description: How to install, configure and extend uncoil, the small open-source lighting daemon for Razer peripherals on Windows.
tableOfContents: false
---

uncoil is a background daemon, `uncoild`, plus an optional desktop app. The daemon drives the lighting on
supported Razer keyboards, mice and mats from one shared effect, fades with the display, and leaves each
device's firmware in charge of its keys. It is one 1.2 MB executable that used about 3 MB of RAM and under
1% of one core on the maintainer's PC.

Not affiliated with or endorsed by Razer Inc.

## Use

- **[Getting started](/docs/getting-started/)**: Install, retire Synapse without leaving the keyboard in driver mode, first run, uninstall.
- **[Configuration](/docs/configuration/)**: Every key in `%APPDATA%\uncoil\config.json`, with defaults.
- **[Troubleshooting](/docs/troubleshooting/)**: Frozen lighting, a dial that scrolls, other RGB software, display sleep.
- **[FAQ](/docs/faq/)**: Licence, what still needs Synapse, what uncoil deliberately doesn't do.

## Hardware

- **[Devices](/docs/devices/)**: What is supported, and how to add a device with one TOML file.
- **[Protocol](/docs/protocol/)**: The HID report format, the commands uncoil uses, and the quirks that cost the most time.

## Project

- **[Architecture](/docs/architecture/)**: The daemon, the app, the files between them, and what's in progress.
- **[Contributing](/docs/contributing/)**: Device files, captures, code.
