## What and why

<!-- What this changes, and the problem it solves. Link the issue: "Closes #123". -->

## Type

- [ ] Bug fix
- [ ] Device definition (new device or correction)
- [ ] Protocol finding / docs
- [ ] Feature
- [ ] Desktop app / design
- [ ] Build, CI or tooling

## Tested on

<!-- Hardware and Windows version. "Unit tests only" is a fine answer for pure changes. -->

- Devices (name, connection, PID):
- Windows:

## Checklist

- [ ] `cargo fmt --all --check`, clippy (`-D warnings`) and `cargo test` pass locally
- [ ] For app changes: `pnpm check` and `pnpm build` pass; states read without colour; reduced motion respected
- [ ] For device files: tested on real hardware, the wave travels the right way, `BUILTIN` and the README table updated
- [ ] Nothing writes a device's onboard memory without the user explicitly asking for it
- [ ] No HID captures with keystrokes, Synapse logs, serial numbers or usernames in the diff
- [ ] `CHANGELOG.md` updated under `[Unreleased]` if users will notice the change
- [ ] I agree to license this contribution under GPL-3.0-or-later
