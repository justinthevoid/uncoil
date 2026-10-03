# CLAUDE.md

Guidance for AI coding agents (Claude Code and others) working in this repository. Humans: see
[CONTRIBUTING.md](CONTRIBUTING.md), which this file assumes.

uncoil is a small Windows daemon (`uncoild`) plus an optional Tauri desktop app that drives Razer
peripheral lighting directly over HID, replacing Razer Synapse. It is pre-release, GPL-3.0-or-later, and
**not affiliated with Razer**.

## Layout

```
crates/uncoil-core         pure logic: report format (proto.rs), colour, effects, device defs, desk layout, config
crates/uncoil-hid          hidapi transport, per-device quirks, display-power watcher
apps/uncoild               daemon: main loop, log.rs, openrgb.rs (one-shot hand-off), selfstat.rs
apps/uncoil                SvelteKit + Tailwind front end (Svelte 5 runes); src/lib/mock = browser mock
apps/uncoil/src-tauri      Tauri shell, Cargo package `uncoil-gui`
devices/*.toml             device definitions, include_str!'d into uncoil-core (BUILTIN in device.rs)
docs/PROTOCOL.md           the protocol and every hardware quirk; read before touching transport or devices
tools/reference/*.py       reverse-engineering scripts; some WRITE TO HARDWARE (see below)
scripts/install-task.ps1   installs the daemon as an elevated logon task
```

Runtime files: `%APPDATA%\uncoil\config.json` (hot-reloaded), `%APPDATA%\uncoil\devices\*.toml` (user
device overrides), `%LOCALAPPDATA%\uncoil\status.json`, `%LOCALAPPDATA%\uncoil\uncoild.log`.

## Commands

```powershell
cargo fmt --all --check
cargo clippy -p uncoil-core -p uncoil-hid -p uncoild -- -D warnings
cargo test   -p uncoil-core -p uncoil-hid -p uncoild
cargo build  --release -p uncoild

# GUI: the front end must be built before the uncoil-gui crate compiles
pnpm --dir apps/uncoil install
pnpm --dir apps/uncoil check
pnpm --dir apps/uncoil build
cargo test -p uncoil-gui                 # mock-vs-real desk check; UNCOIL_UPDATE_MOCK=1 regenerates it
pnpm --dir apps/uncoil dev               # UI in a browser on :1420 against the mock (launch config: uncoil-ui)
```

CI (`.github/workflows/ci.yml`) runs fmt, clippy, tests and both builds on `windows-latest`. Keep it green.

## Hardware safety rules

These are hard rules. The maintainer's devices are real, and some mistakes persist across power cycles.

1. **Never write a device's onboard memory without explicit consent from the user in this conversation**,
   given for that specific write. That covers key mappings (`02/0D`), profiles, macros, dial functions,
   OLED images, and anything else that persists. A plan, a TODO, an issue or a comment in a file is not
   consent. Before any write: read the current value with the matching "get" command, show it, record it,
   and have a restore command ready.
2. **Read-only first.** "Get" commands have the high bit set on the command id. Probing unknown ids is
   allowed only for read-only ids; never scan "set" ids.
3. **Always restore normal mode.** Anything that sends `00/04 [3]` (driver mode) must send `00/04 [0]` on
   every exit path, including errors and Ctrl+C. A device left in driver mode loses its dial and media keys
   until power-cycled.
4. **One writer at a time.** Do not run raw HID probes or `tools/reference` scripts while `uncoild` is
   streaming frames. There is no pause command; stop it first
   (`Stop-ScheduledTask uncoil; Stop-Process -Name uncoild`) and tell the user you did, then restart it
   afterwards (`Start-ScheduledTask uncoil`). Synapse must not be running either.
5. **Respect the quirks** in docs/PROTOCOL.md when changing transport or effects:
   - BlackWidow: read the reply after every report (`ack_every_report`), or it silently stops applying
     frames while still reporting "ok". Retry on `busy`.
   - Send the custom-frame effect (`0F/02 0x08`) once, not per frame (`custom_mode_once`), or the
     keyboard freezes on the first frame.
   - Underglow LEDs live in odd matrix slots; positions come from the device file, never from grid order.
   - The HyperSpeed dongle answers `0x04` while the mouse is on its cable: treat it as "not here yet",
     not an error.
   - Transaction ids differ per device (`0x1F` keyboard and mouse, `0x3F` mat).
6. Don't install or uninstall the logon task, kill Synapse, or change Windows settings unless asked.

## Privacy rules

- Never commit HID captures that contain keystrokes (`drv_capture.py` logs every key typed), raw Synapse
  logs, device serial numbers, Razer account ids, or Windows usernames and machine names. `.gitignore`
  covers common capture names; don't force-add around it.
- When quoting logs or captures in docs, issues or commit messages, reduce them to the relevant bytes and
  replace identifiers with `<serial>`, `<user>`.
- Screenshots for docs must not show notifications, names or other windows.

## Product and design

- Product context, users, voice and principles: [PRODUCT.md](PRODUCT.md). Voice is dry, precise and a
  little wry; lightness leads; claims are measured and labelled ("measured on the maintainer's PC").
  Never invent users, testimonials, download counts or benchmarks.
- Never imply Razer affiliation. Don't use Razer, Synapse or Chroma marks in product names, logos or
  decoration; naming devices to identify compatibility is fine.
- UI work uses the impeccable workflow. Read, in order: `PRODUCT.md`, `DESIGN.md` (written by the finish
  review; may not exist yet), `.impeccable/surfaces/apps-uncoil.md` (the direction contract: Factory
  Records catalogue identity, matte black `#0b0b0b`, plot-white `#f2f2f2` hairlines, zero radius, FAC
  catalogue numbers, colour used only as code). `.impeccable/config.json` is tracked; runtime state, mocks
  and decisions under `.impeccable/` are gitignored.
- Every state must read without colour; respect `prefers-reduced-motion`; keyboard-operable with visible
  focus.
- Principles that decide arguments: earn every byte (the daemon runs all day), the hardware should keep
  working without us, data over code for devices.

## Code conventions

- Rust 2021, MSRV 1.85, `rustfmt.toml` at 120 columns, clippy clean with `-D warnings`.
- `uncoil-core` stays pure and unit-tested. Device-specific behaviour goes in a device file's `[quirks]`.
- No new dependencies in `uncoild` without a reason stated in the commit; watch the release binary size
  (~0.65 MB).
- Adding a device: TOML in `devices/`, entry in `BUILTIN` (`crates/uncoil-core/src/device.rs`), bump the
  count in `builtins_parse_and_validate`, README device table.
- Protocol findings go in `docs/PROTOCOL.md` with how they were verified and the date.

## Commits

- Only commit when the user asks. Never push, tag, change repository settings or create GitHub resources
  unless asked.
- Imperative subject under ~72 characters, blank line, body explaining why. One logical change per commit.
- End every commit message written with AI help with the trailer the session specifies, for example:

  ```
  Co-Authored-By: Claude <noreply@anthropic.com>
  ```

- Update `CHANGELOG.md` under `[Unreleased]` for user-visible changes.
