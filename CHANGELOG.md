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
- **More effects:** breathing, starlight, fire (flames rising from the front of the desk), wheel, reactive
  (keys light when pressed), ripple (rings spread from each press) and an audio meter that fills the desk
  with the system volume peak. Starlight and fire are deterministic per LED position, so every device
  shares one field with no per-LED state.
- **Studio:** stack effects as layers with opacity and masks (whole desk, chosen devices, or chosen keys
  and LEDs). Reactive, ripple, starlight and the audio meter are transparent where unlit.
- **Key presses and audio stay private.** The key listener runs only while reactive or ripple is in use,
  turns each press into a desk position immediately, and never logs or stores which key it was. The audio
  meter reads only Windows' peak level, never samples. See `SECURITY.md`.
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
- **Desktop app** (Tauri 2, SvelteKit, Tailwind; design in `DESIGN.md`): your desk drawn to scale and lit
  with the live effect from the engine's real effect code, effect and display controls, device status, and
  a browser mock for UI work without hardware.
  - **Keys:** the keyboard drawn as solid keycaps from its real geometry, normal and Fn layers, remap any
    key to a key (with modifiers), a mouse button or nothing from a searchable list; restore the original.
    Mouse buttons too.
  - **Dial & screen:** the command dial's mode, OLED brightness and readout, firmware effects per device,
    onboard profile slots.
  - Every onboard write takes a second, explicit confirmation and reports the read-back.
- **Protocol documentation:** `docs/PROTOCOL.md`, a 30-command catalog mined from Synapse's own logs, the
  BlackWidow key-id table, and the onboard key-map commands (verified by mapping Fn+P to Print Screen in
  the keyboard's own memory).
- **Control pipe and `uncoil` CLI.** The daemon now serves `\\.\pipe\uncoil` (newline-delimited JSON,
  current user only) and stays the single owner of device I/O: commands are queued per device and run
  between frames. The new `uncoil` command line covers status, devices, capabilities, onboard key maps
  (get, dump, set, reset, TOML export/import of the Fn layer), profiles, the OLED command dial, OLED
  settings and firmware lighting effects. Onboard writes need `--write`, print before/after, are read
  back, logged and journaled. Design: `docs/ARCHITECTURE.md`.
- **Feature modules** in `uncoil-core` (`features::{hw_effect, keymap, profile, dial, oled}`) and the
  shared `ipc` types for the GUI; device files declare `features`, `[hw_effects]` and `[keymap]` (79 keys
  of the BlackWidow V4 Pro 75%, 13 Basilisk V3 Pro buttons).
- **Protocol documentation:** firmware-effect layout and per-device support, mouse button map, function-id
  data layouts, profiles, command-dial modes and OLED getters, from a second pass over Synapse's logs and
  read-only hardware probes (`tools/reference/readonly_probe.py`).
- **Reverse-engineering tools** in `tools/reference` (log miners, read-only probes, capture and analysis).
- Project scaffolding: CI, release workflow, issue and pull request templates, contributing guide, security
  policy, code of conduct.

### Changed

- The release daemon grew from 0.66 MB to about 0.93 MB for the control pipe and feature modules.
- `scripts/install-task.ps1` installs the elevated daemon to `%ProgramFiles%\uncoil` instead of
  `%LOCALAPPDATA%\uncoil\bin`, so no unelevated process can replace it; `scripts/uninstall-task.ps1` added.

[Unreleased]: https://github.com/justinthevoid/uncoil/commits/main
