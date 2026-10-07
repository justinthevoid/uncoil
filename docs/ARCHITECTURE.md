# Architecture

uncoil is one always-running daemon (`uncoild`) that owns every Razer device, plus clients that ask it
for things: the `uncoil` CLI and the desktop app. Nothing but the daemon opens a device. Protocol details
live in [`PROTOCOL.md`](PROTOCOL.md); this file is about how the code is put together.

```
 uncoil (CLI)      desktop app (Tauri)                    unelevated, the logged-in user
      │                  │
      └──── \\.\pipe\uncoil ─ newline-delimited JSON ────────────────────────────────────────────
                         │
 uncoild (logon task, unelevated)
   pipe acceptor ─▶ client thread ─▶ Control::dispatch
                                       ├─ status / devices / capabilities: answered directly
                                       └─ device command ─▶ job queue of that device
   main loop: config reload, display fade, hot-plug,       │
              conflict scan                                │
   one renderer thread per device ◀────────────────────────┘
     loop { send frame; wait for next frame ← jobs run here, between frames (exec::run) }
                         │   each request under Global\RazerLinkReadWriteGuardMutex
                     HID feature reports (uncoil-hid::LiveDevice)
   openrgb thread (live mode only) ── TCP 127.0.0.1, OpenRGB SDK ───────┐
                                                                        │
 uncoild --openrgb-once (optional "uncoil-openrgb" task, elevated, at logon)
   hardware: reads config.json ─▶ OpenRGB.exe -d … -m …, then exits     │
   live:     writes OpenRGB.json ─▶ OpenRGB.exe --server --server-host 127.0.0.1 ◀┘
             and waits while it runs (motherboard, GPU, RAM; nothing over the pipe)
```

The daemon runs as the logged-in user, unelevated. OpenRGB needs administrator rights (RAM lighting sits on
the SMBus), so it runs from its own elevated task, registered by `install-task.ps1 -OpenRgb`: in `hardware`
mode a one-shot hand-off, in `live` mode an SDK server the daemon connects to as a client.
`install-task.ps1 -Elevated` is the fallback that runs the daemon itself elevated; that daemon then runs the
hand-off or the server itself.

## Crates and modules

| where | what | I/O |
|---|---|---|
| `crates/uncoil-core/src/proto.rs` | 90-byte report builder, `Reply` parser, the `Transport` trait | none |
| `crates/uncoil-core/src/features/` | one module per feature: `hw_effect`, `keymap`, `profile`, `dial`, `oled`, `performance` (DPI, stages, poll rate), `power`, `scroll` (scroll wheel), `info` (firmware, keyboard layout and colour) — report builders and reply parsers, each unit-tested against Synapse-logged, hardware-read or OpenRazer-documented bytes | none |
| `crates/uncoil-core/src/device.rs` | device definitions from `devices/*.toml` and `devices/experimental/*.toml` (all embedded by `build.rs`), with `support`, `features`, per-group transaction ids and the feature sections | reads TOML |
| `crates/uncoil-core/src/layout.rs` | LED positions on the desk; which devices the desk shows and where unplaced ones go (`desk_devices`, `arrange`); desk devices for OpenRGB devices built from their zones (`external_def`) | none |
| `crates/uncoil-core/src/ipc.rs` | the pipe protocol: `Request`, `Response`, `Command` and its argument structs, result types, device-name resolution, a blocking `Client` | client only |
| `crates/uncoil-core/src/effect.rs` | effects, studio layers and masks, `Frame` | none |
| `crates/uncoil-core/src/color.rs` | `Rgb`, the FastLED rainbow hue map | none |
| `crates/uncoil-core/src/config.rs` | `Config` (`config.json`) and `Status` (`status.json`) | reads / writes those files |
| `crates/uncoil-core/src/scancode.rs` | scan code to layout shape name (reactive effects) | none |
| `crates/uncoil-hid/src/display.rs` | display power state (`GUID_CONSOLE_DISPLAY_STATE`) on a hidden window | Windows |
| `crates/uncoil-hid/src/keys.rs`, `audio.rs` | key press listener (Raw Input), audio peak meter (WASAPI) | Windows input / audio |
| `apps/uncoild/src/inputs.rs` | press buffer (positions only), listener start/stop (desk geometry: `uncoil_core::layout::Desk`) | via `uncoil-hid` |
| `crates/uncoil-hid/src/transport.rs` | `LiveDevice`: frames, quirks, and `query()` (send + matching reply, busy/new retry) implementing `Transport` | HID |
| `crates/uncoil-hid/src/guard.rs` | the Razer device lock other programs share (`Global\RazerLinkReadWriteGuardMutex`), taken around every request and reply; see [Sharing devices](#sharing-devices-with-other-programs) | Windows mutex |
| `crates/uncoil-openrgb` | a small OpenRGB SDK client: std only, blocking TCP, connects to `127.0.0.1` only; see [Live OpenRGB](#live-openrgb) | TCP (localhost) |
| `apps/uncoild/src/main.rs` | main loop (config reload, display fade, rescan, conflict scan, status), one renderer thread per device; the control pipe doubles as the single-instance lock; `--fake`, `--openrgb-once` | everything above |
| `apps/uncoild/src/log.rs`, `selfstat.rs` | the log (trimmed past 256 KB), the daemon's own memory / CPU / size | files |
| `apps/uncoild/src/openrgb.rs` | the OpenRGB task's side: the hardware hand-off (targets from `openrgb.devices`, plain names only) or the live SDK server (`OpenRGB.json` merged and written, OpenRGB started on 127.0.0.1 in a kill-on-close job); `taskkill` scoped to the current session; OpenRGB given the admin-only `--config` folder `%ProgramData%\uncoil\openrgb` | process launch, one settings file |
| `apps/uncoild/src/openrgb_live.rs` | the daemon's live OpenRGB client thread: which OpenRGB devices to drive, their desk devices, a frame per tick, `status.openrgb` | via `uncoil-openrgb` |
| `apps/uncoild/src/conflicts.rs` | other programs driving the same devices, by process image name (`status.conflicts`) | Toolhelp process snapshot |
| `apps/uncoild/src/winsec.rs` | is the process elevated; redirection-safe writes to the daemon's own files under `%LOCALAPPDATA%\uncoil` when elevated; the admin-only folder check; system folder and session id | Windows security APIs |
| `apps/uncoild/src/pipe.rs` | named-pipe server; closes connections idle for 5 minutes | pipe |
| `apps/uncoild/src/control.rs` | request router, device registry, job queues | channels |
| `apps/uncoild/src/exec.rs` | runs one command against any `Transport`; write gating, read-back, journal, left-click guard | via `Transport` |
| `apps/uncoild/src/checks.rs` | read-only checks per feature, cached per connection; gate writes on experimental devices | via `Transport` |
| `apps/uncoild/src/fake.rs` | a fake keyboard, mouse and experimental DeathAdder V3 Pro that answer like real ones (tests, `--fake`) | none |
| `apps/uncoil-cli` | the `uncoil` binary | pipe |
| `apps/uncoil/src-tauri` | the desktop app's shell (`uncoil-gui`): config read/write, `get_desk` / `preview_frame` with the real engine (an optional `external` argument, `status.openrgb.devices`, adds the OpenRGB devices), a `daemon` bridge to the pipe; loads the same device files as the daemon (`device::load_installed`: built-ins plus `%APPDATA%\uncoil\devices`); `tray.rs` (tray icon and effect menu), `battery.rs` (battery notifications), `settings.rs` (the app's own `%APPDATA%\uncoil\app.json`, which the daemon never reads) | config, status, pipe, app.json |

Adding a feature is: a module in `features/` (pure, tested), one line in the `commands!` table in `ipc.rs`
(variant, wire name, args) plus an arm in `Command::policy` (which features it needs, whether it writes
the device, which read-only checks gate it), a match arm in `exec.rs`, a subcommand in the CLI, and the
feature name in the device TOMLs that have it. The `every_write_is_gated_by_a_check` test fails if a
command that writes the device is not gated by a check or listed as an exemption.

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
(`kind` = `classic` or `hyperpolling`, `rates`, `set_twice`), `[power]` (`battery`, `idle`,
`low_battery`) and `[scroll]` (`mode`, `acceleration`, `smart_reel`, at least one `true`; feature
`scroll`). `matrix` and `layout` are needed only with `lighting` or `hw_effects`; a device without them
(a mouse with no RGB) is opened for commands only, never gets a frame and never appears on the desk.
Unknown keys anywhere in a device file are errors, and every error names the file and the field
(`devices/experimental/x.toml: [dpi]: min 200 must be above 0 and below max 100`).

Device files are untrusted input (a user file runs in the daemon, and its values end up in HID reports, the
log and the GUI), so values with a size or a fixed set of meanings are checked (`DeviceDef::validate`):

- `vendor_id` must be Razer's `0x1532`; `id` is 1–64 characters of `[a-z0-9-]`, not starting with `-`.
- `[[usb]]`: 1–8 endpoints, no product id twice, at most 8 `alt_usages`, `reply_wait_us` at most 100000,
  and every transaction id one of `0x1F`, `0x3F`, `0x9F`, `0xFF`.
- `[matrix]`: rows 1–32, cols 1–25 (a frame row must fit one report), with `names` matching that size.
- `[layout]`: at most 16 key rows of 48 keys, at most 64 underglow LEDs per side or 64 `points`, and sizes
  that are finite numbers within ±1000.
- `[keymap]`: get/set must be `0x8D`/`0x0D` (keyboards) or `0x8C`/`0x0C` (mice); no key id twice.
- `[hw_effects]`: at most 16 effects. `[dpi]`: `max` at most 50000, `stages_max` at most 5.
  `[poll_rate]`: 1–8 rates the command can set.
- Text: the device name at most 64 characters, key, LED, shape and effect names at most 32, layout key
  names at most 48; none may contain control characters.

A file in `%APPDATA%\uncoil\devices` without a `support` line is `experimental` (nobody vouched for it), so
its writes wait for the read-only checks below. Files there larger than 1 MB are skipped.

### Every file is compiled in

`crates/uncoil-core/build.rs` embeds every `devices/*.toml` and `devices/experimental/*.toml`, sorted by
path, with comments and blank lines dropped, so a new device needs no Rust edit. The test
`every_device_file_parses` parses all of them, checks that a file's folder matches its `support`, and that
no id or product id is used twice. A file that fails to parse is left out at run time (and logged) rather
than stopping the daemon. User files in `%APPDATA%\uncoil\devices` still override built-ins by id (and are
experimental unless they set `support`).

### Support levels and read-only checks

`support = "supported"` (the default) means confirmed on real hardware; `support = "experimental"` means the
file was built from OpenRazer / OpenRGB data and nobody has confirmed it yet (those files live in
`devices/experimental/`). A supported device can also list features nobody has confirmed on it yet:
`unverified = ["power", "scroll"]` on the Basilisk V3 Pro.

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
| scroll | `02/94`, `02/96`, `02/97` (the enabled settings) | each reads 0 or 1 |
| dial / oled | `17/80` / `17/83` | a known mode / a percentage |
| lighting / hw_effects | `0F/80` regions | regions add up to the file's matrix; blocks only saving a firmware effect to the device (`storage: onboard`), never showing one or streaming lighting |

A failed or not-yet-run check makes that feature's writes fail with code `check_failed` and a plain message
("uncoil couldn't confirm Razer DeathAdder V3 Pro answers the way it expects, so it won't change its DPI
settings (…)"); reads still work. Supported devices report `not_needed`. The live DPI change also waits for
the DPI check, although it is not stored. `capabilities` carries `support`, `checks` (one per declared
feature) and `unverified`; `devices` carries `support`.

A check whose read gave up waiting for the [Razer device lock](#sharing-devices-with-other-programs) (another
program held it) is not a verdict on the device: it stays `untested`, the write fails with the lock's plain
message rather than `check_failed`, and the next write runs the check again.

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
| `scroll` | `02/14`, `02/94`, `02/16`, `02/96`, `02/17`, `02/97` |
| `device` | `00/04`, `00/84`, `00/81`, `00/82`, `00/86` |

`reply_wait_us` sets the pause before reading a reply (wireless receivers); `alt_usages` lists more
(usage page, usage) pairs accepted on the same interface. When several collections of one interface match,
the first listed wins and only one is opened.

### Unknown Razer devices

On every rescan the daemon lists HID devices with Razer's vendor id (0x1532) that no definition knows,
logs each product id once, and publishes them as `status.unknown_devices` (`[{product_id, interfaces}]`).

## The control pipe

`\\.\pipe\uncoil`. Only builds with the `fake` feature let `UNCOIL_PIPE` move it (development and tests);
release builds ignore it. One JSON object per line each way:

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
| `scroll.get` | | `{mode, acceleration, smart_reel}` (`mode`: `tactile` / `free_spin`; `null` where the device lacks the setting or didn't answer) | |
| `scroll.set` | `mode`, `acceleration`, `smart_reel`, `write` | before / after; only what differs is sent | **yes** |
| `info.get` | | `{firmware, layout, layout_code, variant}`; firmware on every device, layout and colour variant on keyboards | |

`status` also carries `conflicts` (`[{app, detail}]`, see [Sharing devices](#sharing-devices-with-other-programs))
and `openrgb` (`{state, detail, devices, ours}`, see [Live OpenRGB](#live-openrgb)).

`info.get` works on any device, supported or experimental, and is never gated by a check: it sends only
`00/81` and, on keyboards, `00/86`, never `00/82` (the serial number). It is read once per connection and
kept until the device disconnects (a device that didn't answer is asked again next time).

`profile` is 1 to 5 (onboard profiles are numbered from 1; Razer devices keep at most 5); anything else is
refused before a report is sent.

A failed request is `{"id":…, "ok":false, "error":"<plain words>", "code":"<code>"}`; `code` is present
only for `check_failed`, `left_click_guard` and `not_supported`. The desktop app's bridge returns
`{message, code, unreachable}` to the front end (`unreachable` when the pipe can't be reached at all). Errors that answer the
connection rather than a request (`too many clients`, `request too long`) carry no `id`; `ipc::Client`
treats such an error as the answer to whatever it is waiting for.

Backups (`uncoil keymap export/import`) are built from `keymap.dump` / `keymap.set` on the client side.

## Who may talk to the daemon

The daemon runs as the logged-in user, unelevated by default (elevated only with the `-Elevated` install).
The pipe's security descriptor is `D:P(A;;GA;;;<that user's SID>)S:(ML;;NWNR;;;ME)`: a protected DACL with
a single allow entry for that user, and a medium integrity label (no write up, no read up) so the same
user's *unelevated* CLI and GUI can still connect to an elevated daemon. Other users, services without that
SID, and remote machines (`PIPE_REJECT_REMOTE_CLIENTS`) are refused.

The pipe is the single-instance lock. It is created with `FILE_FLAG_FIRST_PIPE_INSTANCE`, so if another
uncoild, or any other program, already holds `\\.\pipe\uncoil`, the daemon logs
`control pipe … unavailable (…): another uncoild, or another program, holds it; exiting` and exits before it
opens a device.

At most 8 clients at a time (the ninth gets `too many clients` and is disconnected); request lines are
capped at 64 KB; a connection that sends no request for 5 minutes is closed. Clients (`ipc::Client`) open
the pipe with identification-level impersonation only, so the daemon can tell who is calling but cannot act
as them, and refuse a server whose process does not run as the same user (`something else is serving the
uncoil pipe`). They read replies up to 1 MB.

## Commands run between frames

Each renderer thread waits for its next frame in `recv_timeout` on its job queue instead of `sleep`. A job
that arrives is run immediately (`exec::run`), then the thread goes back to waiting until the frame is due.
So feature traffic never interleaves with a frame upload, the keyboard's "read every reply" quirk holds,
and nothing polls: an idle daemon's control path costs no CPU (measured: 0 ms CPU over 20 s for the pipe
server with two fake devices). A key-map dump holds the keyboard's animation for about half a second.

A client waits up to 15 s for a device thread. A job the thread has not started within 10 s is dropped, not
run, and answered with an error ("… was busy for too long; nothing was done"), so a GUI retry after a
timeout cannot apply the same change twice.

A device seen on two endpoints at once (cable and dongle) has two device threads. The first one registered
serves commands; the second waits as a backup and takes over only when the first goes away.

Firmware effects replace streamed frames: after `effect.hw` the renderer stops sending frames, re-applies
the effect after the PC wakes, turns it off while the display is off, and checks every 2 s that the device
is still there. Any config change (or `effect.software`) brings back the software effect.

## Sharing devices with other programs

**The device lock.** OpenRGB takes a named mutex, `Global\RazerLinkReadWriteGuardMutex`, around every report
it sends to or reads from a Razer device (`RazerDeviceGuard.cpp`), and Razer's own software appears to share
it. uncoil takes the same lock (`uncoil_hid::guard`) around each request and its reply, so two programs
never interleave reports on one device. A Windows mutex belongs to the thread that took it, so one lock
thread takes it for the whole daemon, in turns of 20 ms (`guard::TURN`); device threads count themselves
in and out of the current turn around each report and its reply, and send in parallel. At the end of a turn
the lock thread lets the reports in flight finish and releases the mutex, so a program waiting on it gets
it before the next turn. Waits are at most 25 ms:

- a frame report whose lock is busy skips the rest of that frame (the next one comes a frame later);
- a command (`ask`, `query`) tries twice and then fails with plain words: "another program is talking to
  Razer Basilisk V3 Pro right now (Razer's software and OpenRGB use the same device lock); try again in a
  moment". A renderer's "still there?" ping that fails this way does not count as the device going away, and
  a read-only check that fails this way stays untested;
- putting a device back in normal mode on shutdown is sent even if the lock stays busy.

The daemon creates the mutex with default security, or opens the one another program created when it can't
create it, or, failing both, logs `Razer device lock unavailable (…); running without it` and carries on.
Windows mutexes belong to the thread that took them and can be taken again by that thread, so nested calls on
one device thread are fine, and the device threads of one uncoild take turns.

**Conflict detection.** The lock keeps reports apart, but two programs sending colours to one device still
make it flicker between them, and key map or DPI writes can land on top of each other. At every rescan
(every 5 s) the main loop takes a Toolhelp process snapshot and compares only the image names against a
fixed table; nothing else about other processes is read, and only the matching program's name is kept:

| program | image names |
|---|---|
| Razer Synapse | `RazerAppEngine.exe` (Synapse 4), `Razer Synapse 3.exe`, `Razer Synapse Service.exe`, `Razer Synapse Service Process.exe`, `RazerCentralService.exe` |
| Razer Chroma | `RzSDKService.exe`, `RzSDKServer.exe`, `RzChromaStreamServer.exe` |
| OpenRGB | `OpenRGB.exe`, except when it is uncoil's own live server (`status.openrgb.ours`) |
| SignalRGB | `SignalRgb.exe` |

Each program found becomes one entry in `status.conflicts` (`{app, detail}`, `detail` a plain sentence on
what to do), logged once when it appears (`Razer Synapse is running and may fight uncoil over the devices`)
and again only after it went away and came back. The app shows them as a notice; `uncoil status` prints
them.

## Writes are explicit, read back and logged

Every command that writes onboard memory is refused unless the request says `"write": true` (CLI:
`--write`). When it runs, `exec` reads the current value, skips the write if it already matches, writes,
reads back, and reports `before`, `after` and `verified`. Each write gets a line in `uncoild.log`
(`ONBOARD WRITE …`, or `ONBOARD WRITE NOT DONE …` when a confirmed write fails) and JSON lines in
`%LOCALAPPDATA%\uncoil\onboard-writes.jsonl`, each with a `state`: `pending` (with the value from before)
goes in before anything is sent, `done` after the read-back, and `failed` (with what was already applied)
when a write of several reports stops part-way. `keymap.reset` restores the value from before uncoil's
*first* write to that key (from the journal, including a `pending` entry), falling back to the factory
default in the device file.

## Effects, layers and inputs

Effects live in `crates/uncoil-core/src/effect.rs` and are pure functions of (desk position, time):
`Effect::at_with(t, sat, val, &Inputs)` freezes the effect into a `Frame`, and
`Frame::color_led(device_id, shape_name, x, y)` colours one LED (`color_at(x, y)` remains for callers that
don't know the LED). Every effect yields a colour and an alpha. A plain effect is composited over black.
`studio` stacks layers bottom (index 0) to top; each enabled layer whose mask (`all`, `devices`, `keys` by
desk shape name on one device, or `lights`: any `[device, shape]` pairs across the desk) covers the LED blends over the result: `out = mix(out, rgb, alpha * opacity)`. Alpha is 1
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

## Live OpenRGB

With `openrgb.mode = "live"`, the PC's other lighting (motherboard, RAM, GPU) follows the desk effect. The
elevated `uncoil-openrgb` task (`openrgb.rs`) writes OpenRGB's settings (every Razer detector off, the server
on `127.0.0.1` at `openrgb.live.port`, 6742 by default) and starts OpenRGB as an SDK server bound to
`127.0.0.1`, in a job that ends it when the task ends. The daemon connects to it as a client.

**The client** (`crates/uncoil-openrgb`) is written from the protocol facts in OpenRGB's `OpenRGBSDK.md` and
`NetworkProtocol.h`, std only. It asks for protocol version 5 at most (version 6 adds acknowledgements and
unique controller ids, which a lighting client doesn't need) and uses the packets
`REQUEST_PROTOCOL_VERSION` (40), `SET_CLIENT_NAME` (50), `REQUEST_CONTROLLER_COUNT` (0),
`REQUEST_CONTROLLER_DATA` (1), `SET_CUSTOM_MODE` (1100), `UPDATE_LEDS` (1050) and the server's
`DEVICE_LIST_UPDATED` (100). It only ever connects to `127.0.0.1` (the API takes a port, never a host) and
treats the server as untrusted input: packets up to 8 MiB, at most 64 controllers, at most 4096 LEDs per
controller, control characters stripped from names, serials and other unneeded fields skipped unread.

**The thread** (`openrgb_live.rs`) is its own, so a slow or stuck OpenRGB only ever delays OpenRGB's frames,
never the Razer devices'. It idles (one config look a second) unless live mode is on. Connected, it puts
each driven device in its per-LED mode and sends a frame per tick, at the configured `fps` but at most 30:
each frame is computed when it is sent, a device's frame is skipped when it hasn't changed (and resent once a
second anyway, in case the device was reset), and writes time out after 1 s. A failed connection or write is
retried with a growing pause, 1 s doubling to 10 s; `DEVICE_LIST_UPDATED` makes it list the devices again.
When the display fade reaches zero it sends three dark frames and then idles, like the renderers. Turning
live mode off drops the connection and sends nothing more, so the PC's lights stay as last set.

It drives every OpenRGB device except Razer ones (name or vendor containing "Razer", which also covers
products such as the Lian Li O11 Dynamic Razer Edition case; uncoil's server has those detectors off
anyway), hidden ones, ones without LEDs, names matched by
`openrgb.live.exclude` (part of the name, any case), and RAM while Corsair iCUE runs (both would write the
SMBus).

**On the desk** each driven device becomes a desk device with id `openrgb:<slug of its name>` (`-2`, `-3`
for duplicates) and kind `other`, built from its OpenRGB zones (`layout::external_def`): zones sit side by
side, a single-LED zone as a point, a linear zone as a vertical strip, a matrix zone as its grid, the LED area
at most 3 key units tall. LED names come from the zone names ("Aura Mainboard 1", "Logo"). Unplaced, they
stack top to bottom in a "PC" column (motherboards, then RAM, GPUs and the rest) whose right edge is one gap
(1u) left of the keyboard, top-aligned with it, 0.5u apart; a position in `config.desk` (the device's centre)
wins. `status.openrgb` carries `state` (`off`, `waiting`, `connected`, `error`), a plain `detail`, `ours`
(the running OpenRGB is uncoil's own server) and `devices` (`[{id, name, leds, zones}]`); the app passes those
devices to `get_desk` / `preview_frame` as `external`, so its preview builds the same desk.

## Testing without hardware

- `uncoil-core`: every report builder is asserted against bytes from Synapse's logs, OpenRazer's captures
  or the 2026-10-03 hardware reads; spec strings round-trip; device TOMLs validate.
- `uncoild`: `exec` runs against `FakeDevice` (answers like the real keyboard and mouse, records every
  report); `control` tests drive fake device threads through the real router; `pipe` tests start the real
  named-pipe server in-process and talk to it with `ipc::Client`, including a full stack test.
- `cargo build -p uncoild --features fake` then `uncoild --fake` serves fake devices on
  `\\.\pipe\uncoil-fake` (or `UNCOIL_PIPE`), so the CLI and GUI can be exercised with no device and next
  to a running daemon: `uncoil --pipe \\.\pipe\uncoil-fake devices`. It serves the keyboard, the Basilisk
  (with made-up DPI 1600, stages 400/800/1600/3200/6400, 1000 Hz, battery 78 %, sleep 300 s, warning 15 %;
  scroll wheel tactile, acceleration on, Smart Reel off) and the experimental DeathAdder V3 Pro from its
  device file (checks start untested; `check.run` passes them). Made-up device info: firmware 1.03 on the
  keyboard, 1.04 on the Basilisk, 1.02 on the DeathAdder; the keyboard reads US (ANSI), Black. It reports one
  unknown Razer device, product ID 0x0FFE, and Razer Synapse running as a conflict, so the app's notice can
  be tried.
- The browser mock's daemon answers (`apps/uncoil/src/lib/mock/daemon/*.json`, including `scroll-mouse.json`
  and `info-keyboard.json`, `info-mouse.json`, `info-deathadder.json`) are the fake's answers.
  `cargo test -p uncoild gui_mock` fails when they drift (or when the folder holds a file it doesn't
  generate); `UNCOIL_UPDATE_MOCK=1` rewrites them.
- The TypeScript mirrors (`effect.ts`, `effects.ts`, `keys.ts` and the mock's default config) are checked
  against `apps/uncoil/src/lib/mock/fixtures.json`, which `cargo test -p uncoil-core ts_mirror` writes
  from the Rust engine (and compares when not updating). `pnpm check` runs
  `apps/uncoil/scripts/check-mirror.mjs` (colours within one step).

## Footprint

Release `uncoild.exe`: 0.66 MB before the control channel, about 0.93 MB with it (serde for the requests,
the five feature modules, the pipe server). The CLI is a separate 0.6 MB binary that only runs when used.
The layered effects, key listener and audio meter added about 82 KB (0.93 MB to 1.02 MB, measured
2026-10-03); most of it is serde for the new effect, layer and mask types. Experimental devices, checks,
DPI / poll rate / power and per-group transaction ids took it from 1,015,296 to 1,339,904 bytes (+317 KB,
measured 2026-10-03): about 146 KB was the 32 embedded device files (comments stripped; 29 of them
experimental), about 171 KB code, mostly TOML and JSON (de)serialisers for the new sections and results.
Deflating the device files into one blob (inflated once, on first use) brought it to 1,208,320 bytes.
Running unelevated and treating device files and the pipe as untrusted (validation, the pipe idle timeout
and server check, `winsec.rs`, the config-driven OpenRGB hand-off) took it to 1,257,472 bytes (about
1.3 MB, measured 2026-10-03). The command-policy and write-helper refactor left it at 1,260,544 bytes
(measured 2026-10-04). Live OpenRGB, the device lock, the scroll wheel, device info and conflict detection
took it to 1,379,328 bytes (about 1.4 MB, +116 KB, measured 2026-10-04): about 62 KB is the live OpenRGB
client, including the `OpenRGB.json` merge, and about 46 KB the scroll wheel, device info, the lock and the
conflict check.
