# Architecture

uncoil is one always-running daemon (`uncoild`) that owns every Razer device, plus clients that ask it
for things: the `uncoil` CLI and the desktop app. Nothing but the daemon opens a device. Protocol details
live in [`PROTOCOL.md`](PROTOCOL.md); this file is about how the code is put together.

```
 uncoil (CLI)      desktop app (Tauri)                    unelevated, the logged-in user
      │                  │
      └──── \\.\pipe\uncoil ─ newline-delimited JSON ────────────────────────────────────────────
                         │
 uncoild (elevated logon task)
   pipe acceptor ─▶ client thread ─▶ Control::dispatch
                                       ├─ status / devices / capabilities: answered directly
                                       └─ device command ─▶ job queue of that device
   main loop: config reload, display fade, hot-plug        │
   one renderer thread per device ◀────────────────────────┘
     loop { send frame; wait for next frame ← jobs run here, between frames (exec::run) }
                         │
                     HID feature reports (uncoil-hid::LiveDevice)
```

## Crates and modules

| where | what | I/O |
|---|---|---|
| `crates/uncoil-core/src/proto.rs` | 90-byte report builder, `Reply` parser, the `Transport` trait | none |
| `crates/uncoil-core/src/features/` | one module per feature: `hw_effect`, `keymap`, `profile`, `dial`, `oled` — report builders and reply parsers, each unit-tested against Synapse-logged or hardware-read bytes | none |
| `crates/uncoil-core/src/device.rs` | device definitions from `devices/*.toml`, now with `features = [...]`, `[hw_effects]` and `[keymap]` | reads TOML |
| `crates/uncoil-core/src/ipc.rs` | the pipe protocol: `Request`, `Response`, `Command` and its argument structs, result types, device-name resolution, a blocking `Client` | client only |
| `crates/uncoil-core/src/effect.rs` | effects, studio layers and masks, `Frame` | none |
| `crates/uncoil-core/src/scancode.rs` | scan code to layout shape name (reactive effects) | none |
| `crates/uncoil-hid/src/keys.rs`, `audio.rs` | key press listener (Raw Input), audio peak meter (WASAPI) | Windows input / audio |
| `apps/uncoild/src/inputs.rs` | press buffer (positions only), listener start/stop, desk geometry | via `uncoil-hid` |
| `crates/uncoil-hid/src/transport.rs` | `LiveDevice`: frames, quirks, and `query()` (send + matching reply, busy/new retry) implementing `Transport` | HID |
| `apps/uncoild/src/pipe.rs` | named-pipe server | pipe |
| `apps/uncoild/src/control.rs` | request router, device registry, job queues | channels |
| `apps/uncoild/src/exec.rs` | runs one command against any `Transport`; write gating, read-back, journal | via `Transport` |
| `apps/uncoild/src/fake.rs` | a fake keyboard and mouse that answer like the real ones (tests, `--fake`) | none |
| `apps/uncoil-cli` | the `uncoil` binary | pipe |

Adding a feature is: a module in `features/` (pure, tested), a `Command` variant + args in `ipc.rs`, a
match arm in `exec.rs`, a subcommand in the CLI, and the feature name in the device TOMLs that have it.

## Device capabilities are data

Each device TOML declares what it can do:

```toml
features = ["lighting", "hw_effects", "keymap", "profiles", "dial", "oled"]

[hw_effects]
led = 0x05
effects = ["off", "static", "breathing", "spectrum", "wave", "wheel", "reactive", "starlight"]

[keymap]
get = 0x8D
set = 0x0D
keys = [ { id = 26, name = "P", led = "P", default = "key P" }, … ]
```

The daemon refuses a command whose feature the device does not declare, an effect it does not list, or a
key it does not know. `led` links each key to its matrix LED, so a GUI can draw the key map on the same
layout as the lighting. `default` is the factory normal-layer mapping (used by `keymap.reset`).

## The control pipe

`\\.\pipe\uncoil` (override with `UNCOIL_PIPE` for tests). One JSON object per line each way:

```
→ {"id":1,"cmd":"keymap.get","device":"keyboard","args":{"key":"P","layer":"fn"}}
← {"id":1,"ok":true,"result":{"profile":1,"key":26,"name":"P","layer":"hypershift","function":"key PRINT_SCREEN","description":"Print Screen"}}
← {"id":2,"ok":false,"error":"Razer Goliathus Chroma Extended does not support keymap"}
```

`device` is an id, a kind (`keyboard`, `mouse`, `mat`) or a unique part of the name. Key mappings and
firmware effects travel as the same short spec strings the CLI uses (`"key A +lctrl"`, `"razer 4"`,
`"wave left speed 40"`, `"static #ff8000"`); `uncoil_core::features` parses and prints them, so the GUI
gets the typed values for free. Spec strings instead of tagged JSON objects, and JSON values kept as text
(`serde_json::RawValue`) instead of `Value` trees, took about 75 KB off the daemon.

| cmd | args | result | touches onboard memory |
|---|---|---|---|
| `status` | | daemon status (same as `status.json`) | |
| `devices` | | connected devices, features, active firmware effect | |
| `capabilities` | `probe` (bool) | declared features, effects, keys, dial modes; with `probe`, the device's own region/effect lists | |
| `keymap.get` | `key`, `layer` (`normal`/`fn`), `profile` | `KeyMapping` | |
| `keymap.dump` | `layer`, `profile` | every key of the layer | |
| `keymap.set` | `key`, `layer`, `profile`, `function`, `write` | `{before, after, verified, unchanged}` | **yes** |
| `keymap.reset` | `key`, `layer`, `profile`, `write` | same | **yes** |
| `profile.list` | | `{max, count, ids, active}` | |
| `dial.get` | `profile` | `{profile, mode, …}` | |
| `dial.set` | `mode`, `profile`, `enabled`, `write` | before / after | **yes** |
| `oled.get` | | brightness, home screen, dim time, … | |
| `oled.set` | `brightness`, `write` | before / after | **yes** |
| `effect.hw` | `effect`, `storage` (`session`/`onboard`), `write` | `{effect, storage}` | only with `storage: onboard` |
| `effect.software` | | | |

Backups (`uncoil keymap export/import`) are built from `keymap.dump` / `keymap.set` on the client side.

## Who may talk to the daemon

The daemon runs elevated (the logon task uses "highest privileges") as the logged-in user. The pipe's
security descriptor is `D:P(A;;GA;;;<that user's SID>)S:(ML;;NW;;;ME)`: a protected DACL with a single
allow entry for that user, and a medium integrity label so the same user's *unelevated* CLI and GUI can
connect. Other users, services without that SID, and remote machines (`PIPE_REJECT_REMOTE_CLIENTS`) are
refused. `FILE_FLAG_FIRST_PIPE_INSTANCE` makes the daemon fail loudly instead of sharing the name if another
process created `\\.\pipe\uncoil` first. At most 8 clients at a time; request lines are capped at 64 KB.

## Commands run between frames

Each renderer thread waits for its next frame in `recv_timeout` on its job queue instead of `sleep`. A job
that arrives is run immediately (`exec::run`), then the thread goes back to waiting until the frame is due.
So feature traffic never interleaves with a frame upload, the keyboard's "read every reply" quirk holds,
and nothing polls: an idle daemon's control path costs no CPU (measured: 0 ms CPU over 20 s for the pipe
server with two fake devices). A key-map dump holds the keyboard's animation for about half a second.

Firmware effects replace streamed frames: after `effect.hw` the renderer stops sending frames, re-applies
the effect after the PC wakes, turns it off while the display is off, and checks every 2 s that the device
is still there. Any config change (or `effect.software`) brings back the software effect.

## Writes are explicit, read back and logged

Every command that writes onboard memory is refused unless the request says `"write": true` (CLI:
`--write`). When it runs, `exec` reads the current value, skips the write if it already matches, writes,
reads back, and reports `before`, `after` and `verified`. Each write gets a line in `uncoild.log`
(`ONBOARD WRITE …`) and a JSON line in `%LOCALAPPDATA%\uncoil\onboard-writes.jsonl`. `keymap.reset`
restores the value from before uncoil's *first* write to that key (from the journal), falling back to the
factory default in the device file.

## Effects, layers and inputs

Effects live in `crates/uncoil-core/src/effect.rs` and are pure functions of (desk position, time):
`Effect::at_with(t, sat, val, &Inputs)` freezes the effect into a `Frame`, and
`Frame::color_led(device_id, shape_name, x, y)` colours one LED (`color_at(x, y)` remains for callers that
don't know the LED). Every effect yields a colour and an alpha. A plain effect is composited over black.
`studio` stacks layers bottom (index 0) to top; each enabled layer whose mask (`all`, `devices`, or `keys` by
desk shape name) covers the LED blends over the result: `out = mix(out, rgb, alpha * opacity)`. Alpha is 1
for the area effects, the intensity for reactive, ripple and starlight, and lit/unlit for the audio meter.

`Inputs` carries what a pure function can't know: recent key presses as `(x, y, t)`, the audio peak level,
the desk bounds (union of device bodies; fire uses its depth, the audio meter its width) and the keyboard's
centre (the wheel's default pivot). Starlight and fire need no state: they hash the LED's position and a time
bucket, or sample value noise of (x, y, t). `apps/uncoil/src/lib/effect.ts` is a line-for-line TypeScript
port (bit-exact hashes) used by the browser mock; the GUI's `preview_frame` runs the Rust engine with
simulated presses and audio.

In the daemon (`apps/uncoild/src/inputs.rs`), the main loop starts and stops two listeners on every config
(re)load, only while the effect needs them (`Effect::uses_keys`, `uses_audio`):

- **Keys:** keyboard Raw Input on a message-only window (`uncoil-hid::keys`). The callback maps the scan code
  to a layout shape name (`uncoil-core::scancode`) and the name to its desk position on the placed keyboard,
  then keeps only `(x, y, t)` in a ring buffer (64 entries, 5 s). Auto-repeat is ignored. **Privacy is a
  hard rule:** which key was pressed is never logged, stored or sent; see [`SECURITY.md`](../SECURITY.md).
- **Audio:** the default render endpoint's WASAPI peak meter, polled at frame rate on its own thread
  (`uncoil-hid::audio`, three COM calls through hand-written vtables rather than the `windows` crate). No
  samples are captured.

## Testing without hardware

- `uncoil-core`: every report builder is asserted against bytes from Synapse's logs, OpenRazer's captures
  or the 2026-10-03 hardware reads; spec strings round-trip; device TOMLs validate.
- `uncoild`: `exec` runs against `FakeDevice` (answers like the real keyboard and mouse, records every
  report); `control` tests drive fake device threads through the real router; `pipe` tests start the real
  named-pipe server in-process and talk to it with `ipc::Client`, including a full stack test.
- `cargo build -p uncoild --features fake` then `uncoild --fake` serves fake devices on
  `\\.\pipe\uncoil-fake` (or `UNCOIL_PIPE`), so the CLI and GUI can be exercised with no device and next
  to a running daemon: `uncoil --pipe \\.\pipe\uncoil-fake devices`.

## Footprint

Release `uncoild.exe`: 0.66 MB before the control channel, about 0.91 MB with it (serde for the requests,
the five feature modules, the pipe server). The CLI is a separate 0.6 MB binary that only runs when used.
The layered effects, key listener and audio meter added about 82 KB (0.93 MB to 1.02 MB, measured
2026-10-03); most of it is serde for the new effect, layer and mask types.
