---
title: Contributing
description: Where help is most useful (device files, captures, protocol facts) and where the contribution guide lives.
fac: FAC 207
---

The full guide, including how to build, test and format, lives in
[`CONTRIBUTING.md`](https://github.com/justinthevoid/uncoil/blob/main/CONTRIBUTING.md) in the repository.
This page is the short version.

## Most useful right now

1. **Device definitions.** If you own a Razer device that isn't [supported](/docs/devices/), a working
   `devices/*.toml` is the single most valuable contribution. [Devices](/docs/devices/#adding-a-device)
   explains the format and where the facts come from.
2. **Protocol facts.** Anything on the [still to map](/docs/protocol/#still-to-map) list: dial functions,
   OLED images, the media-key data layout. Write down what you sent, what came back and what the device did.
3. **Testing on other machines.** The resource numbers on this site come from one PC. Measurements from yours
   (from `status.json`, which the daemon fills in itself) are welcome in an issue.

## Ground rules

- **Data over code for devices.** If a device needs code, explain why the data file couldn't express it.
- **The hardware should keep working without uncoil.** Prefer writing to the device (normal mode, onboard
  memory) over intercepting input in software.
- **Earn every byte.** Anything added to the daemon runs all day on someone's PC; it has to justify its cost.
- **Never pretend to be Razer.** No Razer, Synapse or Chroma marks in names, icons or UI.
- **HID captures can contain keystrokes.** Never commit raw captures; the repository ignores `*capture*.log`
  for that reason. Share summaries instead.

## Building

```powershell
cargo test --workspace --exclude uncoil-gui     # core, transport, daemon
cargo build --release -p uncoild                # target\release\uncoild.exe
```

The desktop app lives in `apps/uncoil` (Tauri + SvelteKit, pnpm). This website and these docs live in `site/`
(Astro + Starlight, pnpm); see `site/README.md`.

## Licence

uncoil is GPL-3.0-or-later. By contributing you agree your contribution is licensed the same way. Protocol
facts and some device data derive from OpenRazer and OpenRGB (GPL-2.0); credit sources in the file or commit.
