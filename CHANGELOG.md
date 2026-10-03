# Changelog

All notable changes to uncoil are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project will follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html) from its first release. Until 1.0, minor versions
may change config and device-file formats; such changes are called out under **Changed**.

## [Unreleased]

Everything so far. Nothing has been released yet.

### Added

- **`uncoild`, the daemon.** One headless process that drives Razer lighting directly over user-mode HID:
  no services, no kernel drivers, no network. Installed as a per-user logon task by
  `scripts/install-task.ps1`.
- **Desk-wide effects.** Wave (angle, speed, band width, direction), spectrum, static and off, with
  brightness and saturation. Effects are sampled at each LED's physical position, so one wave crosses
  keyboard, underglow, mouse and mat continuously. FastLED rainbow hue map for even-looking colour bands.
- **Display-aware lighting.** Fades out when Windows turns the display off, dims with it, returns on wake.
- **Hot-plug.** Unplugged and replugged devices are picked up again within seconds.
- **Normal-mode restore.** Devices are put back in normal mode on connect, which revives the dial and
  media keys after Synapse leaves them in driver mode.
- **Device definitions as data.** One TOML file per device (USB endpoints, quirks, LED matrix, physical
  layout), compiled in, with user overrides from `%APPDATA%\uncoil\devices`.
- **Supported devices:** Razer BlackWidow V4 Pro 75% (wired; per-key plus 18 underglow LEDs), Basilisk V3
  Pro (wired and HyperSpeed dongle), Goliathus Chroma Extended.
- **Hot-reloaded config** at `%APPDATA%\uncoil\config.json`; live status at
  `%LOCALAPPDATA%\uncoil\status.json`; a small self-trimming log at `%LOCALAPPDATA%\uncoil\uncoild.log`.
- **Optional OpenRGB hand-off:** one CLI run at start puts motherboard, GPU and RAM RGB on their own
  hardware rainbow.
- **Desktop app** (Tauri 2, SvelteKit, Tailwind): live desk preview drawn by the engine's real effect code,
  effect and display controls, device status; a browser mock for UI work without hardware.
- **Protocol documentation:** `docs/PROTOCOL.md`, a 30-command catalog mined from Synapse's own logs, the
  BlackWidow key-id table, and the onboard key-map commands (verified by mapping Fn+P to Print Screen in
  the keyboard's own memory).
- **Reverse-engineering tools** in `tools/reference` (log miners, read-only probes, capture and analysis).
- Project scaffolding: CI, release workflow, issue and pull request templates, contributing guide, security
  policy, code of conduct.

### In progress

- Desktop app redesign in the catalogue direction described in `.impeccable/surfaces/apps-uncoil.md`.

[Unreleased]: https://github.com/justinthevoid/uncoil/commits/main
