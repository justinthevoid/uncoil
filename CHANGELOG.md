# Changelog

All notable changes to uncoil are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project will follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Until 1.0, minor versions
may change config and device-file formats; such changes are called out under **Changed**.

## [Unreleased]

### Added

- **Leave a PC device to its own software.** On the app's PC page, choose a device and pick "Leave it to its
  own software" (it lands in `openrgb.live.exclude`; "Light it again" undoes it). Besides no longer sending it
  frames, uncoil's OpenRGB now turns off any detector whose name the exclusion matches and restarts, so
  OpenRGB lets go of the device and iCUE (or the maker's app) can take it back, for example the iCUE Link
  fans. Only detectors uncoil turned off are turned back on.
- **uncoil steps aside for programs that light PC parts themselves.** A table of them (iCUE, Armoury Crate's
  lighting service, MSI Mystic Light, SignalRGB), kept as data in `crates/uncoil-core/owners.toml`, says which
  OpenRGB devices each one claims. While one runs, uncoil stops lighting those devices in live mode, its
  OpenRGB restarts with the detectors that find them turned off (so the program can take them back, an iCUE
  Link hub included), and the hardware hand-off skips them; a few seconds after it quits, uncoil lights them
  again. Changes wait for two looks in a row to agree, so a program starting up costs one OpenRGB restart.
  The PC page lists the devices under "Run by other software". This replaces the single "skip RAM while
  iCUE runs" rule, which also stays covered: iCUE claims all RAM.
- **Who lights it, per PC device.** The PC page's new "Who lights it" choice pins a device: Automatic (the
  table above decides), uncoil always (even while a program that claims it runs), or a program while it runs,
  for when a program lights more than its own make, such as iCUE driving an ASUS motherboard through its
  plugin. Pins are `openrgb.live.pins`; "Run by other software" rows get "Light it with uncoil", and a
  "Your choices" list undoes pins.

### Fixed

- **Live OpenRGB no longer runs beside a second OpenRGB.** OpenRGB's own Windows service runs in session 0,
  where the task's "close OpenRGB in this session" never reached it, so uncoil started a second server on the
  same port and could end up driving the service's, which had every detector on (the iCUE Link hub left to
  iCUE included): the case lights flickered between iCUE and OpenRGB. The task now stops instead (result
  `2`), the daemon drives only uncoil's own server, and a second OpenRGB beside it shows as a conflict.
- **Turning live OpenRGB off closes OpenRGB** within a couple of seconds, instead of leaving it holding the
  devices until sign-out.
- **iCUE starting after uncoil** now makes live mode let go of the RAM within a few seconds; it was checked
  only when the daemon connected to OpenRGB, so RAM could be written by both.

### Changed

- **Website:** a new landing page, "Uncoiling". The desk's 113 real LEDs start wound into the uncoil spiral,
  unwind onto their measured positions as you scroll, and the app's own traced device drawings draw themselves
  in before the wave switches on; the same points then carry the measured numbers, one field across the desk,
  Fn+P in the keyboard's memory, and the device list. It now shows the 29 experimental devices alongside the
  3 tested ones. Reduced motion and no-JavaScript get a still frame per scene. The docs and the 404 page
  moved into the same dark world (Fraunces, brass, the current page lit like an LED).

## [0.1.0] - 2026-10-07

The first release: everything so far.

### Added

- **`uncoild`, the daemon.** One headless process that drives Razer lighting directly over user-mode HID:
  no services, no kernel drivers, no network (live OpenRGB mode talks to OpenRGB on 127.0.0.1 only).
  Installed as a per-user, unelevated logon task by
  `scripts/install-task.ps1` into `%ProgramFiles%\uncoil`; removed by `scripts/uninstall-task.ps1`.
- **Desk-wide effects:** wave (angle, speed, band width, direction), spectrum, breathing, static, starlight,
  fire (flames rising from the front of the desk), wheel, reactive (keys light when pressed), ripple (rings
  spread from each press), an audio meter that fills the desk with the system volume peak, and off, with
  brightness and saturation. Effects are sampled at each LED's physical position, so one wave crosses
  keyboard, underglow, mouse and mat continuously. Starlight and fire are deterministic per LED position, so
  every device shares one field with no per-LED state. FastLED rainbow hue map for even-looking colour bands.
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
  layout and the features it has), all compiled in by a build script, with user overrides from
  `%APPDATA%\uncoil\devices`.
- **Supported devices:** Razer BlackWidow V4 Pro 75% (wired; per-key plus 18 underglow LEDs), Basilisk V3
  Pro (wired and HyperSpeed dongle), Goliathus Chroma Extended.
- **Experimental devices:** 29 device files in `devices/experimental/`, set up from OpenRazer and OpenRGB data
  and not yet confirmed on real hardware (`support = "experimental"`), generated by
  `tools/devices/gen_experimental.py`.
- **Read-only checks before writes.** On an experimental device, and for features a supported device lists
  as `unverified`, uncoil reads the current value first and refuses to change anything stored in the
  device until that read makes sense (`uncoil check DEVICE`, `check.run`). Lighting is never blocked.
- **Control pipe and `uncoil` CLI.** The daemon serves `\\.\pipe\uncoil` (newline-delimited JSON, current
  user only) and stays the single owner of device I/O: commands are queued per device and run between
  frames. The `uncoil` command line covers status, devices, capabilities, onboard key maps (get, dump, set,
  reset, TOML export/import of the normal and Fn layers), profiles, the command dial, OLED settings,
  firmware effects, DPI and DPI stages, poll rate, power (battery, charging, sleep timer, low-battery
  warning) and the read-only checks. Writes to a device's memory need `--write`, print before/after, are
  read back where the device allows it, logged and journaled. Design: `docs/ARCHITECTURE.md`.
- **Left-click guard:** on every mouse, a key map change that would leave no button that left-clicks is
  refused.
- **Unknown Razer devices** (vendor 0x1532, no definition) are logged once and listed in the status
  (`unknown_devices`).
- Devices without lighting (e.g. a mouse with no RGB) are opened for commands only and never sent frames.
- Connected devices that the config does not place are put on the desk next to devices of their kind (a
  second keyboard below the first, another mouse to the right); the default desk is unchanged.
- **Hot-reloaded config** at `%APPDATA%\uncoil\config.json`; live status at
  `%LOCALAPPDATA%\uncoil\status.json`; a small self-trimming log at `%LOCALAPPDATA%\uncoil\uncoild.log`.
- **Optional OpenRGB hand-off** (off by default, `openrgb.mode = "hardware"`): one OpenRGB CLI run at logon
  puts the motherboard, GPU and RAM devices listed in `openrgb.devices` on their own hardware modes, from the
  elevated task `uncoil-openrgb` (`install-task.ps1 -OpenRgb`).
- **Live OpenRGB** (off by default, `openrgb.mode = "live"`): the motherboard, RAM and GPU follow the desk
  effect. The `uncoil-openrgb` task runs OpenRGB as an SDK server on 127.0.0.1 with every Razer detector off,
  and the daemon streams frames to it from its own thread through a new std-only client,
  `crates/uncoil-openrgb` (protocol version 5 at most; the server is treated as untrusted input). OpenRGB's
  devices join the desk as a "PC" column left of the keyboard (`openrgb:<name>` ids, placeable in
  `config.desk`, usable in Studio masks); `openrgb.live.port` and `openrgb.live.exclude` adjust it. Razer
  devices, and RAM while iCUE runs, are left out. `status.openrgb` reports the connection and the devices.
- **Razer device lock.** Every request to a Razer device runs under `Global\RazerLinkReadWriteGuardMutex`,
  the lock OpenRGB (and apparently Razer's software) takes. One lock thread holds it for the daemon in 20 ms
  turns, so uncoil's devices still send in parallel and other programs get it between turns. Waiting at most
  25 ms, a busy lock skips the rest of a frame, or retries a command once and then fails with plain words. A check that can't get the lock stays untested
  rather than failing.
- **Scroll wheel settings** (`scroll.get`, `scroll.set`, `uncoil scroll`): tactile or free spin, scroll
  acceleration and Smart Reel (`02/14`, `02/16`, `02/17`, from OpenRazer), stored in the mouse and gated by a
  read-only check. Scroll mode and Smart Reel confirmed on the Basilisk V3 Pro, acceleration not yet (so
  `scroll` stays `unverified` there); experimental on the Basilisk V3 and V3 35K. DPI, DPI stages and the poll
  rate are confirmed on the Basilisk V3 Pro. Device files gain `[scroll]` and a `scroll` transaction-id group.
- **Per-device lighting.** The Lighting page lights the whole desk, chosen devices (chips, or click a device
  on the desk; Ctrl-click for several) or lights picked across devices, each with its own effect. It is
  stored as a simple Studio (a whole-desk layer plus opaque zones) and collapses back to one effect when no
  zone differs. Studio masks gain `lights`: any `[device, shape]` pairs, on any devices.
- **Device drawings.** The Basilisk V3 Pro is drawn from a silhouette traced from its top-down product
  photo, with its panel seams, grips and buttons redrawn as line art and uncoil's spiral at the logo LED
  (`tools/art/basilisk.py`; the photo is not in the repository). Keyboards get keycaps with a top face and
  skirt in their key-group frames, and side underglow as light bars; the BlackWidow V4 Pro 75%'s case,
  front lip, OLED and side dial are measured from its top-down product photo, with uncoil's wordmark on the
  lip. On the lit desk devices wear their real black finish; the mat has its cloth, lit edge and cable hub.
- **BlackWidow V4 Pro 75% F-row positions.** Esc and F1-F12 run without gaps, as on the real keyboard (they
  had three 0.25-key gaps), so effects sample them where they are.
- **More device drawings.** Ten more mice get outlines traced from their store photos
  (`tools/art/mouse.py`), with buttons placed by proportion, on the Buttons page and the desk; seven more
  keyboards get their case measured from their photos (`tools/art/keyboard.py`). Mats are drawn with their
  surface, cable hub and edge light in LED order (the whole ring on one-LED mats); the Mouse Dock Pro and Base
  Station V2 Chroma with their lit rings, pad, upright and arm.
- **BlackWidow V4 Pro wrist rest.** Its 20-LED strip runs along the rest's front edge, as in Razer's photo,
  not just in front of the keys; the rest is drawn with the keyboard.
- **Inside the PC.** A PC page on the Desk tab draws the PC's own lighting (motherboard, memory, graphics
  card, fans, AIO pump) where it sits, lit with the desk's effect, from what live OpenRGB reports.
- **Animated effect cards.** Each effect card runs its effect on a small keyboard, from the same effect
  code as the desk preview (20 fps at most, still with reduced motion).
- **Device info** (`info.get`, `uncoil info`): firmware version on every device, and a keyboard's layout and
  colour (`00/81`, `00/86`), read once per connection; never the serial number.
- **Conflict notice.** The daemon looks for Razer Synapse, the Razer Chroma SDK services, OpenRGB and
  SignalRGB by process image name at every rescan, logs each once and lists them in `status.conflicts`; the
  app and `uncoil status` show them. uncoil's own live OpenRGB server doesn't count.
- **Desktop app** (Tauri 2, SvelteKit, Tailwind; design in `DESIGN.md`): your desk drawn to scale and lit
  with the live effect from the engine's real effect code, effect and display settings, device status, and
  a browser mock for UI work without hardware.
  - **Lighting and Studio** for the whole desk, with a card for every effect.
  - **Keys / Buttons:** the keyboard drawn as solid keycaps from its real geometry, normal and Fn layers;
    remap any key to a key (with modifiers), a mouse button or nothing from a searchable list; restore the
    original. Mouse buttons too.
  - **Dial & screen:** the command dial's mode, OLED brightness and readout.
  - **Onboard effects, Performance** (DPI, stages, poll rate) and **Battery & sleep** per device, shown when
    the device file lists the feature; experimental devices say what the read-only check found.
  - Every onboard write takes a second, explicit confirmation and reports the read-back.
  - **Tray icon** with an effect menu, Open and Quit. Optional: keep the app in the tray when its window
    closes, and start it in the tray with Windows (a per-user startup entry, `uncoil-app.exe --tray`). Only
    one copy runs; starting it again shows the window. These preferences live in `%APPDATA%\uncoil\app.json`.
  - **Battery notifications** for devices with the power feature: once at the chosen level (20 % by default,
    15–50 %), once more at 10 %, and once when charging reaches 100 %; checked every 10 minutes while the app
    runs.
  - A notice when Synapse, Razer Chroma, OpenRGB or SignalRGB is running.
  - A **Scroll wheel** section on Performance; firmware, layout and colour on Device info.
  - **OpenRGB** modes (off, hand off once at sign-in, live) with the live connection's status in Settings,
    "Through OpenRGB" on Devices, and the desk preview draws the OpenRGB devices.
- **Feature modules** in `uncoil-core` (`features::{hw_effect, keymap, profile, dial, oled, performance,
  power, scroll, info}`) and the shared `ipc` types for the app; device files declare `features` and the matching sections
  (79 keys of the BlackWidow V4 Pro 75%, 13 Basilisk V3 Pro buttons).
- **Protocol documentation:** `docs/PROTOCOL.md`, a 30-command catalog mined from Synapse's own logs, the
  BlackWidow key-id table, the onboard key-map commands (verified by mapping Fn+P to Print Screen in the
  keyboard's own memory), and, from a second pass over Synapse's logs and read-only hardware probes
  (`tools/reference/readonly_probe.py`), the firmware-effect layout and per-device support, the mouse button
  map, function-id data layouts, profiles, command-dial modes and OLED getters. OpenRazer's shared mouse
  commands (DPI, poll rate, power) are documented as prior art; DPI, stages and the poll rate were then
  confirmed on the Basilisk V3 Pro (2026-10-07).
- **Reverse-engineering tools** in `tools/reference` (log miners, read-only probes, capture and analysis).
- `uncoild --fake` (cargo feature `fake`) serves fake devices on `\\.\pipe\uncoil-fake`, for trying the CLI
  and the app without hardware.
- **The desk is a canvas.** The wheel zooms, dragging empty space pans, double-click fits. **Arrange** moves
  devices (drag, or arrow keys; Alt for finer steps, Shift for whole keys) to where they really sit, written to
  `desk` in `config.json`, so effects like the wave cross the desk as it is; Reset puts them back. The PC's
  OpenRGB devices are drawn as one PC, its parts inside, and move together.
- **THIRD_PARTY_NOTICES.md**: the licences of the Rust crates and npm packages compiled into the binaries,
  and of FastLED's hue map, generated by `scripts/notices/notices.mjs`. It ships in the app's install folder
  (with LICENSE) and with each release, which fails while the file is stale; About links to it.
- Project scaffolding: CI, release workflow, issue and pull request templates, contributing guide, security
  policy, code of conduct, and the website and docs in `site/`.

### Changed

For anyone running an earlier build from source:

- **Errors keep their code end to end.** The desktop app gets `{message, code, unreachable}` from the
  bridge instead of parsing text; `uncoil --json` prints errors as `{"error", "code"}` on stderr.
- **The app loads your extra device files** from `%APPDATA%\uncoil\devices`, like the daemon, so they
  join the desk preview and Studio.
- **A broken config file is reported** in the daemon's log (at start and on reload) instead of silently
  falling back to defaults. At start it still means defaults; on reload the daemon keeps the settings it
  was running, so a half-saved or mistyped file no longer resets the effect, desk and OpenRGB mode.
- **The app never saves over a config it didn't load.** A `config.json` that doesn't parse is shown as an
  error instead of defaults the next change would write over it, and a file changed outside the window (a
  hand edit, the CLI) is reloaded instead of overwritten.
- **The app installs for all users** into `%ProgramFiles%\uncoil`, beside `uncoild.exe`, instead of
  `%LOCALAPPDATA%\uncoil` (where the daemon keeps its log and status). The installer names its publisher,
  licence and homepage. About shows the app's own version and says when the engine's differs.
- The PC column of OpenRGB devices starts left of everything on the desk (a wide mat included), not just
  the keyboard.
- The browser mock now matches the real daemon's check states, and tests keep it and the TypeScript
  effect code in step with the engine.
- **The daemon runs unelevated by default.** `scripts/install-task.ps1` registers the `uncoil` task with run
  level Limited. `-OpenRgb` adds the elevated task `uncoil-openrgb` (`uncoild --openrgb-once`) for the
  OpenRGB hand-off or live server; `-Elevated` is the fallback that runs the daemon itself elevated. The binary goes to
  `%ProgramFiles%\uncoil` instead of `%LOCALAPPDATA%\uncoil\bin` and its hash is checked after copying; the
  installer no longer deletes the old copy, it prints a note. `uninstall-task.ps1` removes both tasks and
  `%ProgramData%\uncoil` and leaves your profile alone.
- **OpenRGB hand-off** is off by default and configured by `openrgb.devices` in `config.json` (plain names
  only); there are no built-in device names. OpenRGB gets an admin-only settings folder
  (`%ProgramData%\uncoil\openrgb`), and only an OpenRGB in your session is closed first.
- **`openrgb.mode`** (`off`, `hardware`, `live`) chooses what uncoil does with OpenRGB. The older
  `openrgb_hardware_rainbow` is still read: `true` means `hardware` when `openrgb.mode` is not set.
- **Stricter device file validation** (Razer vendor id only, known key map commands and transaction ids,
  size limits, plain ids and names). Files in `%APPDATA%\uncoil\devices` are experimental unless they set
  `support`; files over 1 MB are skipped.
- **Pipe:** the control pipe is the single-instance lock (a second uncoild exits); connections idle for 5
  minutes are closed; clients connect with identification-level impersonation and refuse a server that does
  not run as the same user; the integrity label is `NWNR`; release builds ignore `UNCOIL_PIPE`.
- Profiles in requests are limited to 1–5; jobs a device thread has not started within 10 s are dropped and
  answered with an error.
- The onboard-write journal has a `state` field (`pending`, `done`, `failed`); `keymap import` keeps a left
  click on mice and orders its writes so the guard never stops it half-way.
- **Device files:** `matrix` and `layout` are only required with `lighting` or `hw_effects`; new optional
  `support`, `unverified`, `[sources]`, `alt_usages`, `reply_wait_us`, per-command-group
  `[usb.transaction_ids]`, `[dpi]`, `[poll_rate]` and `[power]`; unknown keys are now errors that name the
  file and field.
- **Pipe:** failed requests may carry a `code` (`check_failed`, `left_click_guard`, `not_supported`);
  `devices` and `capabilities` report `support`, `capabilities` also `checks` and `unverified`. A device
  query by kind (`keyboard`, `mouse`) now prefers connected devices; with two mice connected, name one.
- The release daemon is 1,379,328 bytes (about 1.4 MB, 2026-10-04; 1,260,544 before live OpenRGB, the device
  lock, the scroll wheel, device info and conflict detection, 1,208,320 before the hardening above), up from
  0.66 MB before the control pipe;
  [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md#footprint) has the breakdown.

### Fixed

For anyone running an earlier build from source:

- After the display wakes, or when a firmware effect ends, the daemon prepares the device again until it
  answers, instead of trying once; a busy reply no longer leaves the keyboard on its onboard lighting.
- With the display off, only black frames that were actually sent count, so a busy device lock no longer
  leaves the lights on; a firmware effect that fails to follow the display is tried again.
- An onboard write that fails after its `pending` journal entry (a failed firmware-effect save, or a write
  whose read-back fails) is journalled `failed` instead of staying `pending`.
- Two user device files, or a user file and a built-in under another id, for the same USB product no longer
  open the device twice: the user's file replaces the built-in, and a second user file is left out with a
  logged reason.
- Live OpenRGB: a controller reporting more LEDs than uncoil drives (in one zone or in all of them) is
  refused instead of stalling or stopping the daemon.
- Without the Razer device lock (its handle failed), devices no longer skip frames waiting between turns.
  A command that has to wait for one of uncoil's own long commands says so, instead of blaming another
  program.
- Log lines written from several threads at once are no longer lost when the log is trimmed.
- `install-task.ps1` starts `uncoil-openrgb` again after a reinstall with `-OpenRgb`, shows install.log only
  when this run wrote it, says plainly when the administrator prompt was declined or another Windows
  user's uncoild is running the binary. `uninstall-task.ps1` says whether it uninstalled or what is left.

[Unreleased]: https://github.com/justinthevoid/uncoil/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/justinthevoid/uncoil/releases/tag/v0.1.0
