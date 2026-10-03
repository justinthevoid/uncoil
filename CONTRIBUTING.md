# Contributing to uncoil

Thanks for looking. uncoil is small on purpose, and contributions that keep it that way are the most
welcome: device files, protocol findings, careful bug reports, and code that earns its bytes.

By taking part you agree to the [Code of Conduct](CODE_OF_CONDUCT.md). Security problems go to
[SECURITY.md](SECURITY.md), never to a public issue.

## Ways to help

- **Test on your hardware** and report what works and what doesn't (use the bug report form; it asks for the
  details that matter).
- **Add a device.** Usually one TOML file. See [Adding a device](#adding-a-device).
- **Map more of the protocol.** Dial functions, OLED images and media-key data are still open; see
  "Still to map" in [docs/PROTOCOL.md](docs/PROTOCOL.md).
- **Fix or improve code.** For anything larger than a bug fix, open an issue first so we can agree on the
  shape before you spend an evening on it.

## Development setup

You need Windows 10/11 x64 for anything that touches hardware. The pure crates (`uncoil-core`) build and
test anywhere Rust does.

| Tool | Version | For |
|---|---|---|
| [Rust](https://rustup.rs) | stable, 1.85+ (MSVC toolchain), with `clippy` and `rustfmt` | everything |
| Visual Studio Build Tools | "Desktop development with C++" | linking on Windows |
| Node.js | 22 | desktop app |
| [pnpm](https://pnpm.io) | current major (CI uses 12) | desktop app |
| WebView2 runtime | preinstalled on Windows 11 | desktop app |
| Python | 3.10+ with `hidapi`, `pillow`, `pynput` | reference tools only |

The full Tauri list is at <https://v2.tauri.app/start/prerequisites/>.

### Workspace layout

```
Cargo.toml                 workspace: uncoil-core, uncoil-hid, uncoild, uncoil-gui
crates/uncoil-core         protocol, colour, effects, device definitions, desk layout. Pure; no I/O to devices.
crates/uncoil-hid          HID transport (hidapi), per-device quirks, Windows display-power watcher
apps/uncoild               the daemon: no window, started at logon by scripts/install-task.ps1
apps/uncoil                desktop app: SvelteKit + Tailwind front end
apps/uncoil/src-tauri      its Tauri shell; Cargo package name `uncoil-gui`
devices/*.toml             device definitions, compiled into the binaries
docs/PROTOCOL.md           protocol notes and quirks; protocol-catalog.json, blackwidow-keyids.json
tools/reference/           Python probes and log miners used for reverse engineering
scripts/install-task.ps1   installs the daemon as an elevated logon task
```

### Everyday commands

From the repository root:

```powershell
cargo fmt --all                                             # format (CI runs --check)
cargo clippy -p uncoil-core -p uncoil-hid -p uncoild -- -D warnings
cargo test   -p uncoil-core -p uncoil-hid -p uncoild
cargo build  --release -p uncoild                          # target\release\uncoild.exe
```

The GUI crate needs the front end built first, because Tauri embeds `apps/uncoil/build`:

```powershell
cd apps\uncoil
pnpm install
pnpm check            # svelte-check + TypeScript
pnpm build            # static front end into build/
pnpm tauri dev        # app window with hot reload
cd ..\..
cargo test -p uncoil-gui                                    # checks the browser mock matches the real desk
$env:UNCOIL_UPDATE_MOCK=1; cargo test -p uncoil-gui         # regenerate the mock after layout changes
```

`pnpm dev` on its own serves the UI in a browser at <http://localhost:1420> against a mock in
`src/lib/mock/`, which is the fastest loop for UI work and needs no hardware.

To run your build of the daemon instead of the installed one, stop the task first
(`Stop-ScheduledTask uncoil; Stop-Process -Name uncoild`) and then run `target\release\uncoild.exe`. It
logs to `%LOCALAPPDATA%\uncoil\uncoild.log`.

## Code style

- `rustfmt.toml` sets a 120-column width. Run `cargo fmt` before committing.
- Clippy must be clean with `-D warnings`.
- Keep `uncoil-core` pure: no HID, no filesystem side effects outside `config.rs`, unit tests for anything
  numeric (colour, layout, effects).
- Device-specific behaviour belongs in a device file's `[quirks]` table, not in an `if pid == ...` branch.
- The daemon runs all day. Allocation in the frame loop, new threads and new dependencies need a reason.
  Release builds are size-optimised; check `uncoild.exe` doesn't grow without cause.

## Commits and pull requests

- One logical change per commit. Imperative subject line under ~72 characters ("Add Basilisk V3 X device
  file", "Fix underglow order on the BlackWidow"), then a blank line and a body that says why.
- If an AI assistant wrote a meaningful part of the change, say so with a `Co-Authored-By:` trailer.
- Pull requests: fill in the template, link the issue, and say which hardware you tested on. CI must pass.
- By contributing you agree your work is licensed under GPL-3.0-or-later, the project licence.

## Adding a device

Device support is data, not code. A device file describes:

- `id`, `name`, `kind` (`keyboard`, `mouse`, `mousemat`, `headset`, `other`) and `vendor_id` (`0x1532`).
- One `[[usb]]` entry per connection (wired, dongle, ...): `product_id`, the HID `interface`, `usage_page`
  and `usage` of the collection that accepts feature reports, and the `transaction_id`.
- `[quirks]`: `ack_every_report` (read the reply after each report), `custom_mode_once` (send the
  custom-frame effect once, not every frame).
- `[matrix]`: rows, columns and the LED name at every slot (`""` for an empty slot).
- `[layout]`: physical positions, either `type = "keyboard"` rows in key units or `type = "points"`.

Copy the closest existing file in [`devices/`](devices/) and adapt it.

1. **Find the endpoint.** Look the product up in [OpenRazer](https://github.com/openrazer/openrazer) or
   [OpenRGB](https://gitlab.com/CalcProgrammer1/OpenRGB) first; most Razer devices are already there with
   their PID, matrix size and transaction id. Credit the source in a comment at the top of the file.
2. **Test without rebuilding.** Drop the file into `%APPDATA%\uncoil\devices\`. The daemon loads every
   `*.toml` there at start, and a file with the same `id` as a built-in overrides it. Restart the daemon and
   check the log.
3. **Verify the layout with your eyes.** Run a slow wave and confirm it travels across the device in the
   right direction and that no LED shows a colour from the wrong side. Note in a comment what you verified
   and when (the existing files do this).
4. **Make it built in.** Move the file into `devices/`, add it to the `BUILTIN` list in
   `crates/uncoil-core/src/device.rs`, update the count in `builtins_parse_and_validate`, add a row to the
   README's device table, and run the tests.
5. Open a pull request with the device name, PID(s), firmware version if you know it, and what you tested.

Only list connections you have actually tested. Known-but-untested PIDs go in a comment, as the BlackWidow
file does for its wireless variant.

## Reverse engineering

What's in [docs/PROTOCOL.md](docs/PROTOCOL.md) came from three sources: public prior art (OpenRazer,
OpenRGB, OpenSynapse), Synapse's own logs, and careful live probing. New findings should land there, with
how they were verified.

### Hardware safety

- **Read before you write.** Use "get" commands (high bit set on the command id) to learn what a device
  holds before setting anything, and record the original bytes so you can restore them.
- **Onboard memory is persistent.** Key mappings, profiles and macros survive power cycles and move with
  the device to other PCs. Never write them from a script you haven't read, never in a loop, and always ship
  a `--restore` path (see `obm_set_fnp.py`).
- **Restore normal mode.** Anything that sets driver mode (`00/04 [3]`) must put the device back in normal
  mode (`00/04 [0]`) on every exit path, including Ctrl+C, or the dial and media keys stay dead until a
  power cycle. `drv_capture.py` shows the pattern.
- **One writer at a time.** Stop the daemon before running a probe (`Stop-ScheduledTask uncoil;
  Stop-Process -Name uncoild`). Interleaved feature reports confuse both sides, and the BlackWidow is
  particular about reply ordering. Quit Synapse too.
- Unknown command ids can do unknown things. Scanning is for read-only ids only, as `obm_scan.py` does.

### Privacy: what never gets committed

- **HID captures that contain keystrokes.** Driver-mode captures (`drv_capture.py`) log every key you
  press, including passwords typed while the capture runs. Keep captures local. If a finding depends on
  one, reduce it to the specific reports that matter and check by eye that nothing else survived. Capture
  logs are ignored by `.gitignore` (`*capture*.log`, `*.pcap`, `*.pcapng`); don't force-add them.
- **Synapse logs**, raw or excerpted beyond the specific command bytes you're documenting. They contain
  your Windows username, paths, account identifiers and device serials.
- **Serial numbers**, Razer account IDs, Windows usernames and machine names, in any file, screenshot or
  issue. Replace them with `<serial>`, `<user>` and so on.
- Derived, anonymised artefacts (like `docs/protocol-catalog.json`) are fine and very welcome.

When doing a capture, close password managers and anything you'd type a secret into, type only the marker
letters and the keys you are testing, and stop the capture as soon as you have what you need.

### Using `tools/reference`

These are the scripts the protocol was learned with: small, single-purpose and Windows-only. Install the
dependencies with `pip install hidapi pillow pynput` and run them from that folder.

| Script | Writes to device? | What it does |
|---|---|---|
| `mine_synapse_logs.py` | no | builds `protocol-catalog.json` from Synapse's logs on this PC |
| `mine_keyids.py`, `mine_keymap.py` | no | extract key ids and onboard key-map structure from Synapse's logs |
| `obm_probe.py` | no | read-only dump of profiles and a few key mappings |
| `obm_scan.py` | no | scans read-only command ids in class `0x02` |
| `obm_set_fnp.py` | **yes, onboard memory** | maps Fn+P to Print Screen; `--restore` undoes it |
| `drv_capture.py` | mode change (restored) | driver-mode capture of input reports **and keystrokes**; see privacy above |
| `analyze_capture.py` | no | summarises a `drv_capture.py` log by section markers |
| `razer_wave.py` | yes, lighting only | sets the hardware wave effect with a custom speed |
| `rgb_effect.py`, `rgb_layout.py`, `preview.py` | lighting only | the Python prototype of the desk wave, and a GIF preview |

The PIDs and interfaces in these scripts are hard-coded for the maintainer's BlackWidow V4 Pro 75%. Adapt
them for your device, and re-read the safety rules before running anything marked "yes".

## Design

The desktop app and site follow a written design direction: [PRODUCT.md](PRODUCT.md) (who it's for, voice,
principles) and the surface brief in [`.impeccable/surfaces/`](.impeccable/surfaces/). UI changes should
fit that world (matte black, white hairlines, colour used as code) and keep every state readable without
colour and calm under reduced motion.
