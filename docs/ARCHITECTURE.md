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
| `crates/uncoil-core/src/features/` | one module per feature: `hw_effect`, `keymap`, `profile`, `dial`, `oled`, `performance` (DPI, stages, poll rate), `power` — report builders and reply parsers, each unit-tested against Synapse-logged, hardware-read or OpenRazer-documented bytes | none |
| `crates/uncoil-core/src/device.rs` | device definitions from `devices/*.toml` and `devices/experimental/*.toml` (all embedded by `build.rs`), with `support`, `features`, per-group transaction ids and the feature sections | reads TOML |
| `crates/uncoil-core/src/layout.rs` | LED positions on the desk; which devices the desk shows and where unplaced ones go (`desk_devices`, `arrange`) | none |
| `crates/uncoil-core/src/ipc.rs` | the pipe protocol: `Request`, `Response`, `Command` and its argument structs, result types, device-name resolution, a blocking `Client` | client only |
| `crates/uncoil-core/src/effect.rs` | effects, studio layers and masks, `Frame` | none |
| `crates/uncoil-core/src/color.rs` | `Rgb`, the FastLED rainbow hue map | none |
| `crates/uncoil-core/src/config.rs` | `Config` (`config.json`) and `Status` (`status.json`) | reads / writes those files |
| `crates/uncoil-core/src/scancode.rs` | scan code to layout shape name (reactive effects) | none |
| `crates/uncoil-hid/src/display.rs` | display power state (`GUID_CONSOLE_DISPLAY_STATE`) on a hidden window | Windows |
| `crates/uncoil-hid/src/keys.rs`, `audio.rs` | key press listener (Raw Input), audio peak meter (WASAPI) | Windows input / audio |
| `apps/uncoild/src/inputs.rs` | press buffer (positions only), listener start/stop, desk geometry | via `uncoil-hid` |
| `crates/uncoil-hid/src/transport.rs` | `LiveDevice`: frames, quirks, and `query()` (send + matching reply, busy/new retry) implementing `Transport` | HID |
| `apps/uncoild/src/main.rs` | main loop (config reload, display fade, rescan, status), one renderer thread per device, single-instance mutex, `--fake` | everything above |
| `apps/uncoild/src/log.rs`, `selfstat.rs`, `openrgb.rs` | the log (trimmed past 256 KB), the daemon's own memory / CPU / size, the one-shot OpenRGB hand-off | files, process launch |
| `apps/uncoild/src/pipe.rs` | named-pipe server | pipe |
| `apps/uncoild/src/control.rs` | request router, device registry, job queues | channels |
| `apps/uncoild/src/exec.rs` | runs one command against any `Transport`; write gating, read-back, journal, left-click guard | via `Transport` |
| `apps/uncoild/src/checks.rs` | read-only checks per feature, cached per connection; gate writes on experimental devices | via `Transport` |
| `apps/uncoild/src/fake.rs` | a fake keyboard, mouse and experimental DeathAdder V3 Pro that answer like real ones (tests, `--fake`) | none |
| `apps/uncoil-cli` | the `uncoil` binary | pipe |
| `apps/uncoil/src-tauri` | the desktop app's shell (`uncoil-gui`): config read/write, `preview_frame` with the real engine, a `daemon` bridge to the pipe | config, status, pipe |

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

The daemon refuses a command whose feature the device does not declare (error code `not_supported`), an
effect it does not list, or a key it does not know. `led` links each key to its matrix LED, so a GUI can
draw the key map on the same layout as the lighting. `default` is the factory normal-layer mapping (used by
`keymap.reset`).

More sections, each required by its feature: `[dpi]` (`min`, `max`, `storage`, `stages_max`), `[poll_rate]`
(`kind` = `classic` or `hyperpolling`, `rates`, `set_twice`) and `[power]` (`battery`, `idle`,
`low_battery`). `matrix` and `layout` are needed only with `lighting` or `hw_effects`; a device without them
(a mouse with no RGB) is opened for commands only, never gets a frame and never appears on the desk.
Unknown keys anywhere in a device file are errors, and every error names the file and the field
(`devices/experimental/x.toml: [dpi]: min 200 must be above 0 and below max 100`).

### Every file is compiled in

`crates/uncoil-core/build.rs` embeds every `devices/*.toml` and `devices/experimental/*.toml`, sorted by
path, with comments and blank lines dropped, so a new device needs no Rust edit. The test
`every_device_file_parses` parses all of them, checks that a file's folder matches its `support`, and that
no id or product id is used twice. A file that fails to parse is left out at run time (and logged) rather
than stopping the daemon. User files in `%APPDATA%\uncoil\devices` still override built-ins by id.

### Support levels and read-only checks

`support = "supported"` (the default) means confirmed on real hardware; `support = "experimental"` means the
file was built from OpenRazer / OpenRGB data and nobody has confirmed it yet (those files live in
`devices/experimental/`). A supported device can also list features nobody has confirmed on it yet:
`unverified = ["dpi", "poll_rate", "power"]` on the Basilisk V3 Pro.

Before uncoil changes anything stored in an experimental device (or an unverified feature), it runs that
feature's **read-only check**: it reads the current value with the matching "get" command and checks the
reply makes sense. One check per feature, run on first need or by `check.run`, cached on the device thread
until the device disconnects:

| feature | reads | passes when |
|---|---|---|
| keymap (keyboard) | normal-layer mapping of P (26), A (31), Esc (110) | each maps to its own key or the file's default |
| keymap (mouse) | buttons 1 and 2 | left click and right click |
| profiles | `05/8A`, `05/80` | a well-formed count |
| dpi | `04/85` (and `04/86` when the mouse has stages) | DPI and every stage inside `[dpi]` min..max |
| poll_rate | `00/85` or `00/C0` | a rate listed in `[poll_rate]` |
| power | `07/80` + `07/84`, `07/83`, `07/81` (the enabled parts) | well-formed, idle 60–900 s, threshold `0x0C`–`0x3F` |
| dial / oled | `17/80` / `17/83` | a known mode / a percentage |
| lighting / hw_effects | `0F/80` regions | informational only: regions add up to the file's matrix; never blocks lighting |

A failed or not-yet-run check makes that feature's writes fail with code `check_failed` and a plain message
("uncoil couldn't confirm Razer DeathAdder V3 Pro answers the way it expects, so it won't change its DPI
settings (…)"); reads still work. Supported devices report `not_needed`. The live DPI change also waits for
the DPI check, although it is not stored. `capabilities` carries `support`, `checks` (one per declared
feature) and `unverified`; `devices` carries `support`.

**Left-click guard.** On every mouse, `keymap.set` / `keymap.reset` refuse a normal-layer change that would
leave no button producing left click (code `left_click_guard`: "This would leave no button that
left-clicks. Map another button to left click first."). The check only reads the other buttons when the key
being changed is currently left click.

### Transaction ids per command group

Each `[[usb]]` endpoint has a default `transaction_id`. Firmwares that want another id for some commands
get a `[usb.transaction_ids]` table; missing groups use the default. The transport (`UsbEndpoint::wire`)
puts the right id on every report, whatever id the report was built with.

| group | commands |
|---|---|
| `frame` | `0F/03` custom frame rows, `0F/02` with effect 8 (custom frame) |
| `effect` | every other class `0F` command (firmware effects, brightness, regions) |
| `keymap` | `02/0D`, `02/8D`, `02/0C`, `02/8C`, `02/84` |
| `profile` | class `05` |
| `dpi` | `04/05`, `04/85`, `04/06`, `04/86` |
| `poll` | `00/05`, `00/85`, `00/40`, `00/C0` |
| `power` | `07/80`, `07/84`, `07/03`, `07/83` |
| `low_battery` | `07/01`, `07/81` (falls back to `power`) |
| `device` | `00/04`, `00/84`, `00/81`, `00/82` |

`reply_wait_us` sets the pause before reading a reply (wireless receivers); `alt_usages` lists more
(usage page, usage) pairs accepted on the same interface. When several collections of one interface match,
the first listed wins and only one is opened.

### Unknown Razer devices

On every rescan the daemon lists HID devices with Razer's vendor id (0x1532) that no definition knows,
logs each product id once, and publishes them as `status.unknown_devices` (`[{product_id, interfaces}]`).

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
| `check.run` | | `[{feature, state, detail}]`, state `passed` / `failed` / `untested` / `not_needed` | |
| `performance.get` | | `{dpi, dpi_min, dpi_max, stages, stages_max, poll_hz, poll_rates}` | |
| `performance.set` | `dpi` (`{x, y}`), `stages` (`{active, list}`), `poll_hz`, `write` | `dpi` alone: the new state; else before / after | `stages`, `poll_hz` (and `dpi` on `varstore` mice) |
| `power.get` | | `{battery_pct, charging, idle_s, idle_range, low_battery_pct, low_battery_range}` | |
| `power.set` | `idle_s`, `low_battery_pct`, `write` | before / after | **yes** |

A failed request is `{"id":…, "ok":false, "error":"<plain words>", "code":"<code>"}`; `code` is present
only for `check_failed`, `left_click_guard` and `not_supported`. The desktop app's bridge turns it into the
error text `"<code>: <plain words>"`, next to the existing `unreachable:` prefix.

Backups (`uncoil keymap export/import`) are built from `keymap.dump` / `keymap.set` on the client side.

## Who may talk to the daemon

The daemon runs elevated (the logon task uses "highest privileges") as the logged-in user. The pipe's
security descriptor is `D:P(A;;GA;;;<that user's SID>)S:(ML;;NW;;;ME)`: a protected DACL with a single
allow entry for that user, and a medium integrity label so the same user's *unelevated* CLI and GUI can
connect. Other users, services without that SID, and remote machines (`PIPE_REJECT_REMOTE_CLIENTS`) are
refused. `FILE_FLAG_FIRST_PIPE_INSTANCE` makes the daemon refuse to share the name if another process
created `\\.\pipe\uncoil` first: it logs `control pipe … unavailable` and keeps driving the lighting without
a pipe. At most 8 clients at a time; request lines are capped at 64 KB.

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
  to a running daemon: `uncoil --pipe \\.\pipe\uncoil-fake devices`. It serves the keyboard, the Basilisk
  (with made-up DPI 1600, stages 400/800/1600/3200/6400, 1000 Hz, battery 78 %, sleep 300 s, warning 15 %)
  and the experimental DeathAdder V3 Pro from its device file (checks start untested; `check.run` passes
  them), and reports one unknown Razer device, product ID 0x0FFE.
- The browser mock's daemon answers (`apps/uncoil/src/lib/mock/daemon/*.json`) are the fake's answers:
  `UNCOIL_UPDATE_MOCK=1 cargo test -p uncoild gui_mock` rewrites them.

## Footprint

Release `uncoild.exe`: 0.66 MB before the control channel, about 0.93 MB with it (serde for the requests,
the five feature modules, the pipe server). The CLI is a separate 0.6 MB binary that only runs when used.
The layered effects, key listener and audio meter added about 82 KB (0.93 MB to 1.02 MB, measured
2026-10-03); most of it is serde for the new effect, layer and mask types. Experimental devices, checks,
DPI / poll rate / power and per-group transaction ids took it from 1,015,296 to 1,339,904 bytes (+317 KB,
measured 2026-10-03): about 146 KB was the 32 embedded device files (comments stripped; 29 of them
experimental), about 171 KB code, mostly TOML and JSON (de)serialisers for the new sections and results.
Deflating the device files into one blob (inflated once, on first use) brought it to 1,208,320 bytes.
