# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

(The desktop app is a Tauri webview on Windows; the marketing site and docs are web. Design language is web, not native Windows.)

## Users

Enthusiasts first, polished enough for everyone else.

- **Primary:** Windows power users who own Razer peripherals and are fed up with Razer Synapse's resource use. They are comfortable with config files, OpenRGB, GitHub, and reading a protocol write-up, and some will contribute device definitions or reverse-engineering.
- **Secondary:** everyday Razer owners (gamers, creators) who want their lighting and keys to keep working without Synapse. For them it must work after install without touching a file.

Job: get the device features they paid for (lighting, Fn layer, media controls) back, without a heavyweight vendor app running all day.

## Product Purpose

uncoil replaces the parts of Razer Synapse people actually use with a tiny background daemon and an optional desktop app. Success means Synapse can be uninstalled, the user's desk looks and works as before or better, and nothing they didn't ask for runs on their PC.

## Positioning

**Lightness leads.** Measured on the maintainer's PC: Synapse 4 ran 17 processes at ~1.4 GB RAM at start and leaked to multiple GB over days; uncoil's daemon is one 1.4 MB executable using ~3 MB RAM and under 1% of one core (memory and CPU measured on an earlier build). It runs as the user, unelevated. No services, no account, no telemetry, no kernel drivers.

Supporting points (secondary, in this order):
1. One effect across the whole desk, using each LED's real physical position (keyboard keys and underglow, mouse, mat).
2. Hardware ownership: features written into the device's own memory keep working without uncoil (e.g. Fn+P → Print Screen on the BlackWidow), and the protocol is openly documented.

## Operating Context

- Windows 10/11 desktop. The daemon starts at logon via a scheduled task and runs headless; the app is opened occasionally to change effects or check device status, then closed (or, if the user chooses, kept in the tray for its effect menu and battery notifications).
- Config at `%APPDATA%\uncoil\config.json` (hot-reloaded); status at `%LOCALAPPDATA%\uncoil\status.json`.
- Lighting fades when Windows turns the display off, dims with it, and returns on wake.
- Contributors work from `devices/*.toml`, `docs/PROTOCOL.md`, and the Python tools in `tools/reference`.

## Capabilities and Constraints

- Supported devices today: Razer BlackWidow V4 Pro 75% (wired), Basilisk V3 Pro (wired/HyperSpeed), Goliathus Chroma Extended. More via data files.
- Effects: wave, spectrum, breathing, static, starlight, fire, wheel, reactive, ripple, audio meter, off; brightness and saturation; a different effect per device; Studio layers.
- Device firmware stays in normal mode so Fn/media/dial keep working even if uncoil isn't running.
- Optional OpenRGB for non-Razer RGB (motherboard, GPU, RAM), off by default, from its own elevated logon task only if installed with `-OpenRgb`: either a one-shot hand-off to the devices' hardware modes, or live, where OpenRGB runs as a local server and those devices follow the desk effect.
- Built: key and button remapping (normal and Fn / Hypershift layers), dial mode and OLED brightness, mouse DPI, poll rate, power and scroll settings, a PC page for OpenRGB lighting. Undecided: per-app profiles, a daemon installer (the app has an NSIS installer), distribution beyond GitHub Releases.
- License GPL-3.0-or-later. Not affiliated with Razer; must not use Razer/Synapse/Chroma marks in the product name or logo.

## Brand Commitments

- Name: **uncoil** (lowercase). A quiet nod to snakes/Razer and to "unwinding" bloat, without trademarks.
- Voice: dry, precise, a little wry. Measured claims backed by numbers, understatement over hype, at most one subtle snake/uncoil joke per surface.
- Standalone brand (not tied to the maintainer's other identities).
- Existing asset: app icon (brass spiral on graphite, `apps/uncoil/src-tauri/icons`). Provisional, not binding.

## Evidence on Hand

- Measured resource numbers above (one machine; label as such).
- `docs/PROTOCOL.md`: real reverse-engineering write-up (Synapse log mining, quirks, onboard key maps).
- A real phone video of the desk wave running (user has it; not yet in the repo).
- No testimonials, users, download counts, or benchmarks across machines exist. Do not fabricate any.

## Product Principles

1. **Earn every byte.** Anything that runs all day must justify its cost; the app may be richer because it is closed most of the time.
2. **The hardware should keep working without us.** Prefer writing to the device over intercepting in software.
3. **Show, then claim.** Numbers and live previews over adjectives.
4. **Data over code for devices.** Supporting a device should be a file, not a fork.
5. **Never pretend to be Razer.**

## Accessibility & Inclusion

- Lighting is decorative; every state the app shows must also be readable without colour (devices, connection, errors).
- Respect reduced motion: the live preview and transitions must calm down or pause.
- Keyboard-operable controls with visible focus.
