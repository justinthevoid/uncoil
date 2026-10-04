# What else OpenRazer, OpenRGB and their ecosystem offer uncoil (2026-10-03)

Research only; no code was changed. This builds on
`tools/reference/device-research/razer-devices-notes.md` and `docs/PROTOCOL.md` and does not repeat what they
already cover (transport, DPI/stages, poll rate, battery/charging/idle/low-battery, transaction-id groups,
matrix conflicts, `0F/80`/`0F/81` probing, key ids). Facts only; no code was copied.

## Sources (shallow clones, read 2026-10-03)

| project | commit | date | licence (verified, see §7) |
|---|---|---|---|
| openrazer/openrazer | `a84cd0ae` | 2026-10-03 | GPL-2.0-or-later (all 76 SPDX headers); logos CC-BY-SA-4.0 |
| CalcProgrammer1/OpenRGB (GitLab) | `df3024be` (master); tag `release_1.0` → `81bbe18a` | 2026-10-03 | GPL-2.0-or-later (1840 files), 18 files GPL-2.0-only, bundled mbedtls Apache-2.0 |
| OpenRGBDevelopers/OpenRGBEffectsPlugin | `f90f3ec5` | 2026-09-18 | GPL-2.0-or-later |
| OpenRGBDevelopers/OpenRGBVisualMapPlugin | `e9648aaa` | 2026-09-14 | GPL-2.0-or-later |
| polychromatic/polychromatic | `52a7e4d9` | 2026-10-01 | GPLv3 ("licensed under the GPLv3", no "or later") |
| z3ntu/libopenrazer, z3ntu/RazerGenie | `2d919637`, `193fd6d1` | 2026-06-01 | GPL-3.0-or-later (REUSE) |
| z3ntu/razer_test | `0de4b0a` | 2025-09-16 | no SPDX; stale, 7 devices |
| Achtuur/openrgb-rs2 (crate `openrgb2` 0.4.1) | `d7e50723` | 2026-09-05 | `GPL-2.0` (= GPL-2.0-only) |
| nicoulaj/openrgb-rs (crate `openrgb` 0.1.2) | `d35d674f` | 2022-05-14 | `GPL-2.0` (= GPL-2.0-only), unmaintained |
| Artemis-RGB/Artemis | GitHub API | 2026-09-26 | PolyForm Noncommercial 1.0.0 (not open source) |
| Artemis-RGB/Artemis.Plugins.Wrappers | GitHub API | 2026-03-27 | no licence file (all rights reserved) |

OR = OpenRazer `a84cd0ae`, RGB = OpenRGB `df3024be`. `chromacommon` = `driver/razerchromacommon.c`.
VARSTORE = 1, NOSTORE = 0.

---

## 1. Ranked top 8

| # | opportunity | value | effort | risk | recommendation |
|---:|---|---|---|---|---|
| 1 | **OpenRGB SDK client**: drive motherboard/GPU/RAM live with the desk-wide effect | high (the maintainer's own ask; whole PC in sync) | M | M (OpenRGB must run elevated; slow I2C devices) | **do now**, own minimal client (not the crates) |
| 2 | **Take `Global\RazerLinkReadWriteGuardMutex`** around every Razer transaction | high (safe coexistence with OpenRGB, which #1 starts, and Synapse) | S | L | **do now** (prerequisite for #1) |
| 3 | **Basilisk V3 Pro scroll wheel**: mode (tactile/free-spin), acceleration, Smart Reel | high (maintainer's mouse; Synapse-only today) | S | L | **do now**, read-only check first |
| 4 | **Keyboard info (`00/86`) + firmware (`00/81`)**: auto layout (ANSI/ISO/JIS/AZERTY), colour variant, firmware in device info | med-high | S | L (read-only) | **do now** |
| 5 | **Tray icon + battery notifications + conflict check** (Polychromatic tray, OpenRazer BatteryNotifier) | high daily value (two wireless devices) | S-M | L | **do now** |
| 6 | **Game mode + macro LED**: light the firmware LEDs, implement game mode (Win-key block) and macro-record indicator host-side | med | S-M | M (keyboard hook = keystrokes) | **later** (after #1-#5) |
| 7 | **Import OpenRazer's fake-driver capability matrix** (273 device cfgs) and widen experimental devices to every Razer PID both projects know | med (more users, auto `features`) | S-M | L-M (untested files) | **do now** (generator already exists) |
| 8 | **uncoil as an OpenRGB SDK server** so OpenRGB plugins / Artemis drive Razer devices through uncoil | med (power users; fixes OpenRGB 1.0's frozen Razer animations) | L | M (auth-less TCP, arbitration) | **later** |

Details follow in §2-§6; each opportunity there has value / effort / risk / sources / recommendation.

### 1.1 OpenRGB SDK client (rank 1)

See §4.1 for the protocol facts. Shape: `uncoild` (already an elevated logon task, `scripts/install-task.ps1`)
spawns `OpenRGB.exe --server --server-host 127.0.0.1 --server-port 6742 --startminimized --config <uncoil-owned dir>`
(all flags exist in RGB `cli.cpp` help text) with an `OpenRGB.json` in that dir whose
`Detectors.detectors.<name> = false` disables every Razer detector (RGB `DetectionManager.cpp:996`; detector names
are the first argument of the 206 `REGISTER_HID_DETECTOR*` lines in `Controllers/RazerController/RazerControllerDetect.cpp`).
uncoil connects as a client, sets each non-Razer controller to its per-LED ("Direct", `SETCUSTOMMODE` 1100) mode,
and sends one `UPDATELEDS` (1050) per controller per frame, sampling the desk effect at each external LED's
desk position (user places "external" tiles: linear strips and matrices). Replaces `apps/uncoild/src/openrgb.rs`'s
one-shot hardware rainbow (keep it as fallback when OpenRGB is absent).

- Latency: OpenRGB runs a per-controller worker (`RGBController::DeviceCallThreadFunction`, RGB
  `RGBController/RGBController.cpp:2055`) that coalesces pending `UpdateLEDs` into one flag, so a 30 fps client
  never blocks on slow SMBus/I2C devices; they just skip frames. Localhost TCP cost is negligible (header 16 B +
  4 B/LED).
- Elevation: OpenRGB on Windows needs PawnIO **and** Administrator for SMBus (RAM, many motherboards, GPUs)
  (`Documentation/SMBusAccess.md:13-14`). uncoild is already elevated, so it can start OpenRGB elevated; the GUI
  need not be.
- iCUE: keep the existing "skip Vengeance while iCUE runs" rule. OpenRGB takes `Global\Access_SMBUS.HTP.Method`
  (`i2c_smbus/Windows/i2c_smbus_pawnio.h:22`) and `Global\CorsairLinkReadWriteGuardMutex`
  (`Controllers/CorsairController/CorsairDeviceGuard.cpp:61`), so bus corruption is unlikely, but two programs
  would still fight over colours.
- Cost: ~600-900 lines of Rust (sync `std::net::TcpStream`, no tokio) + desk UI for external tiles. The crates are
  GPL-2.0-only (§4.3), so write it from the protocol doc.

### 1.2 Razer access mutex (rank 2)

OpenRGB serialises every Razer HID write and read through a named mutex `Global\RazerLinkReadWriteGuardMutex`
(RGB `Controllers/RazerController/RazerDeviceGuard.cpp:61`, created with a NULL DACL so any session can open it;
abandoned-mutex case handled by releasing and retrying, lines ~20-35; used in `RazerController.cpp:2147` (send)
and `:2153` (receive)). The name implies it is the lock Razer's own software ("Razer Link"/Synapse) uses; OpenRGB
added it on 18 May 2024. uncoil takes no such lock today (no match for `RazerLink|CreateMutex` in the repo).
- Recommendation: take it around each full request + reply-read (stronger than OpenRGB's per-call lock), with a
  timeout (~100 ms) and "skip this frame" on timeout, never block the frame loop. Verify on hardware with Process
  Explorer (handle search for `RazerLinkReadWriteGuardMutex`) that Synapse holds it; if not, it still protects
  against OpenRGB, which #1 makes likely to run.

### 1.3 Scroll wheel settings (rank 3)

| feature | set | get | args | txn | devices (OR attr list) |
|---|---|---|---|---|---|
| scroll mode | `02/14` size 2 | `02/94` | `[VARSTORE, 0 tactile / 1 free-spin]`; reply arg 1 | `0x1F` | Basilisk V3, V3 Pro (wired/wireless), V3 Pro 35K (+ Phantom Green) |
| scroll acceleration | `02/16` size 2 | `02/96` | `[VARSTORE, 0/1]` | `0x1F` | same 7 PIDs |
| Smart Reel | `02/17` size 2 | `02/97` | `[VARSTORE, 0/1]` | `0x1F` | same 7 PIDs |

Sources: chromacommon `:1558`, `:1571`, `:1587`, `:1600`, `:1617`, `:1630`; `driver/razermouse_driver.c:2880`
(write_scroll_mode, txn 0x1F), `:2928`, `:2976`; meanings in
`daemon/openrazer_daemon/dbus_services/dbus_methods/mouse_scroll_wheel.py:7-90`. uncoil already knows the live
side: input report 5 event `57` (notch 0 / free-spin 1) and the scroll-mode button id 106 (PROTOCOL.md), so the
UI can track presses of the physical button. The research notes list the commands; uncoil does not use them.
- Recommendation: add to `features/performance.rs` (or a `scroll.rs`), mark `unverified` in
  `devices/razer-basilisk-v3-pro.toml`, read with `uncoil check mouse` first. VARSTORE = stored on the mouse.

### 1.4 Keyboard info and firmware (rank 4)

- `00/86` get keyboard info (OR `razerkbd_driver.c:503`: size 2, txn 0xFF, reply arg 0; RGB
  `RazerController.cpp:1360`: size 0, reply arg 0 = layout, arg 1 = variant).
- Layout codes (RGB `RazerController.h:115-135`): 0 none, 1 US, 2 Greek, 3 German, 4 French, 5 Russian, 6 UK,
  7 Nordic, 8 CHT, 9 Korean, 10 Turkish, 11 Thailand, 12 Japan, 13 PT-BR, 14 ES-LatAm, 15 Swiss, 16 ES-EU,
  17 Italian, 18 PT-PT, 19 Hebrew, 20 Arabic. OR adds `0x81` = en_US Mac
  (`daemon/.../dbus_methods/all.py:11`). RGB maps them to ANSI / ISO / AZERTY / JIS (`RazerController.cpp:1374-1410`,
  most marked "Unconfirmed").
- Variant byte (RGB `RazerController.h:159-161`): `0x00` Black, `0x80` Quartz, `0x82` Mercury: lets the UI draw
  the board in the right colour.
- Devices with `kbd_layout` in OR (24): BlackWidow Elite, V3, V3 Pro, V4, **V4 75% (0x02A5)**, V4 Pro, V4 TKL
  HyperSpeed, V4 X, DeathStalker V2 family, Huntsman Elite, V2, V2 Analog, V3 Pro / 8KHz / Mini / TKL, Joro,
  Ornata V2. The Pro 75% (0x02B3) is not in OR but its siblings are.
- Firmware `00/81` size 2 → `v{arg0}.{arg1}` (chromacommon `:67`; RGB `RazerController.cpp:1210-1221`). Serial
  `00/82` size 0x16 (chromacommon `:59`): **do not read or show** unless needed for device identity, and never log
  it (CLAUDE.md privacy rules).
- Recommendation: read once on connect; pick the key-label set / ISO-vs-ANSI layout from it; show firmware in
  the device panel and in `uncoil check` output (useful in bug reports, e.g. the BW V4 collection move at fw 1.5).

### 1.5 Tray, battery notifications, conflict check (rank 5)

- OpenRazer BatteryNotifier (`daemon/openrazer_daemon/misc/battery_notifier.py`): polls, notifies when level
  <= a threshold, rate-limited by a frequency; message bands <=10 % "low, please charge", <=30, <=70, ==100 % "fully
  charged" (`:82-90`). Defaults in `daemon/resources/razer.conf:13-20`: enabled, every 600 s, threshold 33 %.
- Polychromatic tray applet (`polychromatic-tray-applet`): per-device quick effects and brightness from the tray,
  plus a "procviewer"/troubleshooter that lists processes that may conflict.
- uncoil: has Synapse-quit guidance and checks, no tray, no notifications (grep found none). Tauri 2 has a
  built-in tray icon and `tauri-plugin-notification`. Since the daemon already reads battery (`07/80`) and gets
  battery events (report 5 event `49`) and connect/disconnect (event `9`), notifications need no new protocol.
- Recommendation: tray icon with battery % per wireless device, pause/resume lighting, quick effect; one toast at
  the user's threshold (default ~20 %, matching the OLED low-battery warning read as 20) and one at "full while
  charging"; at most one toast per device per hour. Conflict check: list Synapse, OpenRGB (when not started by
  uncoil), SignalRGB, iCUE, Armoury Crate if running.

### 1.6 Game mode and macro LEDs (rank 6)

- LED state: `03/00` size 3 `[storage, led, 0/1]`, get `03/80` (chromacommon `:83`, `:107`). LED effect: `03/02`
  `[storage, led, effect]`, get `03/82` (`:148`, `:164`). LED ids (`driver/razercommon.h`): MACRO 0x07, GAME 0x08,
  BATTERY 0x03, profile R/G/B 0x0C-0x0E, CHARGING 0x20 / FAST_CHARGING 0x21 / FULLY_CHARGED 0x22.
- Game LED on modern boards (BW V3 family, **V4 / V4 Pro / V4 75% / V4 X / V4 TKL / V4 Mini**, Ornata V3 family,
  Huntsman V3 Pro family): `03/00 [VARSTORE, 0x08, on]` with **txn 0xFF**; Huntsman V2 / V2 TKL use NOSTORE and
  0x1F (`razerkbd_driver.c:929-1032`). Macro LED: `03/00 [VARSTORE, 0x07, on]` and effect `03/02 [VARSTORE,
  0x07, effect]`, txn 0xFF (`:1209`, `:1236`, `:1769`, `:1886`).
- Behaviour is host-side in OpenRazer: the kernel driver blocks Super / Alt+Tab / Alt+F4 while game mode is set
  (`key_super`, `key_alt_tab`, `key_alt_f4` attrs, `:4819-4895`, stored in `block_keys[]`). On the BlackWidow,
  Fn+F10/F9 already arrive at the host as Razer keys 3 (game mode) and 4 (macro record) (PROTOCOL.md fn 17).
- Recommendation (later): on Razer key 3 toggle game mode = Win-key block via a low-level keyboard hook that only
  inspects GUI keys and logs nothing, light the game LED; on key 4 blink the macro LED while recording (macro
  recording itself is out of scope until the onboard macro format is mapped). Alternative without a hook: write
  Left/Right GUI (127/128, still untested ids) to "Off" in the Normal layer, but that is two flash writes per toggle.

### 1.7 Capability matrix + more devices (rank 7)

`pylib/openrazer/_fake_driver/*.cfg` (273 files, generated by `scripts/generate_all_fake_drivers.sh`) list, per
device, every sysfs attribute the driver creates with r/w flags and a default value (e.g.
`razerbasiliskv3pro35kwired.cfg` has `scroll_mode`, `scroll_smart_reel`, `tilt_hwheel`, `charge_low_threshold`,
`device_idle_time`, `dpi_stages`...). It is a ready-made, machine-readable feature matrix (attribute names map
1:1 to commands documented in §2-§3).
- Use: feed `tools/devices/gen_experimental.py` to set `features = [...]` automatically, cross-check the
  hand-written device files in a test, and extend the experimental set from 30 to all Razer keyboards / mice /
  mats / accessories that OR and RGB both know (RGB alone registers 206 Razer HID detectors).
- Licence: generated data from a GPL-2.0-or-later project; transcribing names/defaults is fine either way.
- Risk: more untested files; keep them `support = "experimental"`, and prefer runtime `0F/80`/`0F/81` probes.

### 1.8 uncoil as SDK server (rank 8)

See §4.2. Lets OpenRGB's GUI ("SDK Client" tab / `--client host:port`), its Effects and Visual Map plugins
(which work on network controllers too) and Artemis' OpenRGB provider drive Razer devices while uncoil keeps
sole HID ownership, quirk handling and the "read every reply" rule. PROTOCOL.md already records that OpenRGB 1.0
froze per-LED animations on these devices; this would route around that. L effort (serialise controller
descriptions per protocol version, arbitration with uncoil's own engine, port choice since 6742 is OpenRGB's).

---

## 2. Device features not yet covered by uncoil or its docs

Legend: **R** = recommendation. Devices are the ones in OR's attribute lists for that feature (parsed from the
`CREATE_DEVICE_FILE` switch in each driver's probe function at `a84cd0ae`).

### 2.1 Keyboards

| feature | facts | devices | value / effort / risk | R |
|---|---|---|---|---|
| Keyboard layout + variant | §1.4 | 24 kbds incl. V4 75% | med-high / S / L | **now** |
| Firmware version | §1.4 | all modern | med / S / L | **now** |
| Game-mode LED | §1.6 | V3/V4/Huntsman V3 Pro family | med / S / L | later (with game mode) |
| Macro LED (state, blink) | §1.6 | 61 kbds | low-med / S / L | later |
| Game mode key blocking | host-side (§1.6) | any | med / M / M (hook) | later |
| **Keyswitch optimisation** (Huntsman V2 optical: "typing" vs "gaming") | set = two reports: `02/02` size 4 then `02/15` size 5; typing (mode 0): `02/02 [0, 0x14, 0, 0x28, 0]` and `02/15 [1, 0, 0x14, 0, 0x28, 0]`; gaming (mode 1): `02/02` all zero and `02/15 [1]`; get `02/82` size 4, arg 1 == `0x14` → typing, else gaming; txn 0x1F (chromacommon `:925`, `:949`, `:980`; `razerkbd_driver.c:1131`, `:1170`) | Huntsman V2, V2 TKL only | low (no owner) / S / M (two writes, unverified) | later, experimental-only |
| **Analog actuation / rapid trigger** (Huntsman V2 Analog, Mini Analog, V3 Pro family) | **no source anywhere**: OR and RGB only drive lighting on these; grep for actuation/rapid/snap-tap finds nothing (confirmed also in the experimental Huntsman V3 Pro TOML notes) | — | high for owners / L / H | **skip** unless an owner mines Synapse logs with `tools/reference/mine_synapse_logs.py` |
| Keyboard poll rate (HyperPolling `00/40`) | already in research notes; OR exposes `poll_rate` on BW V4, **V4 75%**, V4 Pro, V4 TKL HyperSpeed, Huntsman V2 / V2 TKL | the maintainer's Pro 75% has no `poll_rate` feature today | med / S / L | **now**: read-only `00/C0` check on 0x02B3 |
| Charging-dock / charge effect (BW V3 Pro) | `03/10` size 1 `[0 = follow current effect, 1 = charge colour]` (chromacommon `:1075`); colour = `03/01` size 5 `[NOSTORE, 0x03 BATTERY_LED, r, g, b]` (`:119`); txn 0xFF (`razerkbd_driver.c:818`, `:845`) | BW V3 Pro wired/wireless | low / S / L | later (experimental device) |
| Battery / charging on wireless keyboards | same `07/80` / `07/84` as mice | BW V3 Pro, V3 Mini, V4 Mini, V4 TKL HyperSpeed, DeathStalker V2 Pro (+TKL), Joro | covered by existing power feature | extend device files |
| Hardware brightness per LED/zone | `0F/04 [VARSTORE, led, 0-255]` / `0F/84` (chromacommon `:714`, `:731`) | all extended-matrix devices | low-med / S / L (onboard write) | later: only matters for firmware effects when uncoil is not running |
| Fn toggle (Blade laptops) | `02/06 [0, 0/1]` (`:909`) | Blade 2016-2017 | — | skip (laptops) |
| Profile LEDs R/G/B | `03/00` on 0x0C-0x0E | Tartarus / Orbweaver / Nostromo keypads | — | skip |
| Pulsate / logo LED state | classic `03/02` effects | 2012-2014 boards | — | skip |

### 2.2 Mice

| feature | facts | devices | value / effort / risk | R |
|---|---|---|---|---|
| **Scroll mode / acceleration / Smart Reel** | §1.3 | Basilisk V3 family (7 PIDs) | high / S / L | **now** |
| Tilt → horizontal wheel, tilt repeat + delay | **host-side only** in OR (driver translates tilt button input; `razermouse_driver.c:3019-3061`, no device command) | 16 PIDs | low on Windows: uncoil keymap already maps tilt with fn 1 ButtonCode 104/105 or fn 14 turbo | skip |
| Dock charge effect / colour | same `03/10` + `03/01 [NOSTORE, 0x03, rgb]`; txn 0x1F on Naga Pro / **Naga V2 Pro**, 0xFF on Mamba/Lancehead/Viper Ultimate/DA V2 Pro/Basilisk Ultimate receiver (`razermouse_driver.c:1790`, `:1836`; meaning `dbus_methods/mamba.py:112`, `:132`) | 9 PIDs | low / S / L | later |
| Dock brightness | `07/02 [b]` / `07/82` (chromacommon `:1191`, `:1200`) | 2 uses in mouse driver | low / S / L | later |
| **HyperPolling dongle**: indicator LED mode, 3-LED modes, pair / unpair | indicator `07/10 [1 connection, 2 battery, 3 battery warning]`, get `07/90`; three LEDs `07/15 [m1, m2, m3]` (0 off, 1 battery, 2 connection, 3 poll rate, 4 DPI), get `07/95`; pair = `00/46 [1]` then `00/41 [1, pid_hi, pid_lo]`; unpair `00/42 [pid_hi, pid_lo]` (chromacommon `:1642-1726`; meanings `mamba.py:350-442`) | HyperPolling dongle, Viper V3 Pro wireless, Viper Mini SE wireless | med for owners (pairing is Synapse-only otherwise) / M / M (pair state) | later |
| Per-zone brightness (logo / scroll / backlight / left / right) | `0F/04` per LED id (1 wheel, 4 logo, 5 backlight, 0x10 right, 0x11 left) | 37 / 35 / 6 / 4 PIDs | low (uncoil streams frames) | later with hw effects |
| Firmware / serial | `00/81`, `00/82` (all mice per OR) | all | med / S / L | firmware now, serial never logged |

### 2.3 Accessories

| feature | facts | R |
|---|---|---|
| Chroma ARGB controller channel sizes | get `0F/88` size 0x0D `[6]` → `[6, 1, n1, 2, n2, … 6, n6]`; set `0F/08` size 0x0D same layout, txn 0xFF; reset = `0F/04` brightness per channel LED 0x1A-0x1F (VARSTORE, 0xFF) then `00/B7 [0]` and `00/36` (`driver/razeraccessory_driver.c:2147`, `:2198`, reset ~`:2280`); per-channel brightness `0F/04` on 0x1A-0x1F (`:1531-1561`); daemon `argb_controller.py` exposes get/set channel size and brightness | later, needed before the experimental ARGB controller is usable (LED count per strip) |
| Mouse Dock Pro, Base Station V2, Thunderbolt 4 Dock | matrix 1x8 / 1x8 / 1x12 in daemon `hardware/accessory.py:97-183` | covered by experimental files |
| Kraken headsets, Hanbo AIO | separate controllers in RGB (`RazerKrakenController`, `RazerKrakenV3/V4Controller` with own CRC, `RazerHanboController`) | skip until an owner appears |

### 2.4 Transport / coexistence (new, not in uncoil docs)

- Razer access mutex (§1.2). 
- Abandoned-mutex handling (RGB releases and retries on WAIT_ABANDONED).
- OpenRGB treats HID failures per device with a guard manager; uncoil already retries busy replies.
- Windows Dynamic Lighting (HID LampArray): OpenRGB has a `HIDLampArrayController` (GPL-2.0-only files, facts
  only). If a Razer device exposes LampArray, Windows' "Dynamic Lighting" setting can fight uncoil; worth a line
  in SUPPORT.md ("turn off Dynamic Lighting for this device") after checking the BW V4 Pro 75% in Windows Settings.

---

## 3. Software effects and behaviours worth borrowing

- OpenRazer ripple (`daemon/openrazer_daemon/misc/ripple_effect.py`): refresh 40 ms (`:30`), radius grows 24
  matrix cells per second (`:134`), ring thickness 2 cells. uncoil already has ripple; useful only as a default
  sanity reference.
- OpenRazer screensaver monitor (`misc/screensaver_monitor.py`): suspend lighting while the screensaver is active.
  uncoil handles display off/dim; adding "lock screen" and "idle N minutes" (GetLastInputInfo) is a small S item
  and matches Synapse's idle-off setting. **Later.**
- OpenRazer `autosave_persistence.py`: interval-based persistence of device state; uncoil writes config atomically
  already. Skip.
- OpenRGB Effects plugin list (`Effects/`): Ambient (screen capture), AudioBubbles, AudioParty, AudioSine,
  AudioStar, AudioSync, AudioVisualizer, AudioVUMeter, Bloom, BouncingBall, BreathingCircle, Bubbles, Clock,
  Comet, CrossingBeams, FractalMotion, GifPlayer, MovingPanes, NoiseMap, RadialRainbow, Rain, RotatingBeam,
  Shaders, SparkleFade, Spiral, Stack, StarryNight, Sunrise, Swap, SwirlCircles, Visor, Wavy, ZigZag and more.
  Best candidates for uncoil's desk sampler: **Ambient** (Desktop Duplication API; most-requested Synapse feature,
  M), Rain / Bubbles / Comet / Visor (S each), audio spectrum (uncoil has a meter). The plugin README warns ambient
  scaling is CPU-heavy and audio effects spike on S/PDIF devices: keep capture small and optional.

---

## 4. OpenRGB SDK integration

### 4.1 Protocol facts (RGB `NetworkProtocol.h`, `Documentation/OpenRGBSDK.md`)

- TCP, default `127.0.0.1:6742` (`NetworkProtocol.h:33`, `:44`). No authentication; the server binds localhost by
  default (`--server-host` changes it). Max packet 8 MiB (`:38`).
- Header 16 bytes, little-endian: magic `"ORGB"`, `pkt_dev_id` u32, `pkt_id` u32, `pkt_size` u32 (OpenRGBSDK.md
  §NetPacketHeader).
- Versions (`NetworkProtocol.h:19-26`; doc table lines 9-17): 0 initial (0.3), 1 versioning + vendor (0.5),
  2 profiles (0.6), 3 mode brightness + SaveMode (0.7), 4 zone segments + plugin interface (0.9), 5 zone/controller
  flags, effects-only zones, alt LED names, Clear/AddSegments (1.0rc1/rc2), **6 = release 1.0**: matrix-map
  segments, per-zone modes, remote SettingsManager/ProfileManager, update callbacks, server name, **unique
  controller ids instead of indexes** (doc §Device IDs). The `release_1.0` tag (`81bbe18a`) defines version 6; the
  doc table still marks 6 "unreleased" (stale). Negotiation: client sends `REQUEST_PROTOCOL_VERSION` (40) with its
  max; both use the minimum; no reply means version 0.
- IDs used by a lighting client: 0 controller count, 1 controller data, 40 protocol version, 50 client name,
  100 device list updated (server push), 1100 SETCUSTOMMODE, 1050 UPDATELEDS (`data_size` u32, `num_colors` u16,
  colours 4 B each), 1051 UPDATEZONELEDS (+ `zone_idx`), 1052 UPDATESINGLELED, 1101 UPDATEMODE. v6 adds
  1000-1005 zone/segment config, 1103 UPDATEZONEMODE, 1130/1131 device-specific config, 1150 SIGNALUPDATE,
  150-161 profile manager, 200/201 plugins, 250+ settings, 300+ log manager.
- Zone types (RGB `RGBControllerInterface.h:153-159`, also in `release_1.0`): SINGLE, LINEAR, MATRIX, and new
  LINEAR_LOOP, MATRIX_LOOP_X, MATRIX_LOOP_Y, SEGMENTED. Device types include MOTHERBOARD, DRAM, GPU, COOLER,
  LEDSTRIP, … MONITOR, UNKNOWN (`:184-205`). LED positions are **not** in the protocol: only zone type, matrix map
  and LED order, so uncoil must let the user place external devices (Visual Map plugin does the same with a
  drag-and-drop grid).
- Debug aid: `Documentation/OpenRGB SDK Wireshark Dissector.lua`.
- Profiles: JSON now, legacy binary `.orp` (`OPENRGB_PROFILE` header, version = protocol version;
  `ProfileManager.cpp:28-29`, `:214`, `:244`). Nothing worth importing.

### 4.2 Two directions

| | uncoil as **client** (rank 1) | uncoil as **server** (rank 8) |
|---|---|---|
| user gets | MB/GPU/RAM/fans follow the desk effect live | OpenRGB/Artemis/plugins can drive Razer gear through uncoil |
| needs running | OpenRGB with `--server` (uncoild can spawn it elevated with `--config` pointing at an uncoil-owned dir) | uncoild only; clients connect to uncoil's port |
| protocol work | parse controller data (modes, zones, LED count, matrix maps); send SETCUSTOMMODE + UPDATELEDS; handle 100 "device list updated" | serialise controller data per negotiated version (0-6), implement Update*LEDs / mode packets, push 100 on hot-plug |
| arbitration | must exclude Razer detectors in OpenRGB (else both drive the same HID devices; the mutex prevents corrupted transactions but not colour fights) | while a client streams, pause uncoil's engine for that device and resume after N s idle |
| port | 6742 | not 6742 (OpenRGB's); e.g. 6743, user adds it in OpenRGB's client list / Artemis' server list |
| effort | M | L |
| risks | OpenRGB missing / not elevated / PawnIO not installed (fall back to the current one-shot CLI); slow SMBus devices (coalesced, §1.1); OpenRGB detection on startup can take seconds | unauthenticated localhost TCP (bind 127.0.0.1 only, document it); version drift (v6 changed device ids) |

### 4.3 Rust crate options

| crate | version / date | protocol | runtime | licence | verdict |
|---|---|---|---|---|---|
| `openrgb2` (Achtuur/openrgb-rs2) | 0.4.1, 2026-09-05; ~7.3 k lines | default 6 (`src/protocol/mod.rs:19`), changelog "0.4.0 Update to SDK version 6" | tokio (`Cargo.toml:26`) | `GPL-2.0` in Cargo.toml, plain GPLv2 text, no "or later" grant in sources → **GPL-2.0-only** | **incompatible** with uncoil's GPL-3.0-or-later when linked |
| `openrgb` (nicoulaj/openrgb-rs) | 0.1.2, 2022-05-14 | old (pre-v4) | tokio + async-trait | GPL-2.0-only | unmaintained, incompatible |

Recommendation: implement a small synchronous client in `uncoil-core` (types + (de)serialisation, pure, testable
with golden bytes) and `uncoild` (socket). Facts from the GPL-2.0-or-later OpenRGB doc/headers can be used freely;
optionally ask Achtuur whether `openrgb2` could be relicensed GPL-2.0-or-later (OpenRGB itself is), which would
make it usable, but tokio would still be a heavy new daemon dependency (CONTRIBUTING: new deps need a reason).

---

## 5. Data sets

| data | where | licence / terms | use | R |
|---|---|---|---|---|
| Capability matrix, 273 devices | OR `pylib/openrazer/_fake_driver/*.cfg` | GPL-2.0-or-later (generated) | §1.7 | **now** |
| Detector table (206 Razer HID detectors: name, PID, interface, usage page, usage) | RGB `Controllers/RazerController/RazerControllerDetect.cpp` | GPL-2.0-or-later | more experimental devices; OpenRGB detector-disable list for §1.1 | **now** |
| Full OpenRGB VID/PID list | `OpenRGB --print-udev-rules` | GPL-2.0-or-later | tell users "this unknown device is handled by OpenRGB" | later |
| Keyboard layouts | RGB `KeyboardLayoutManager/` | GPL-2.0-or-later | already used (`tools/devices/keyboard-matrices.json`) | done |
| Key label grids, 9 generic layouts | RazerGenie `data/matrix_layouts/*.json` (REUSE: GPL-3.0-or-later, 2017 Luca Weiss) | same licence as uncoil: code and data reusable | low value (generic) | skip |
| Device drawings, 36 SVG + 9 key-map JSON | Polychromatic `data/devicemaps/` (`maps.json`: name → svg, rows, cols, locale, scancode json); hand-drawn vectors (no embedded raster), incl. `blackwidow_v4_pro_en_US.svg`, `basilisk_v3.svg`, `mouse_dock_pro.svg`, `huntsman_v2_analog_en_GB.svg` | GPLv3 (only): can ship inside GPL-3.0-or-later uncoil, but those files then carry GPL-3.0 terms; credit Polychromatic | device pictures without Razer imagery, but the maintainer prefers plain UI (memory) | later, optional |
| Product images | OR daemon `DEVICE_IMAGE` (229 entries across `hardware/*.py`; the 205 URLs in keyboards.py + mouse.py: assets.razerzone.com 114, dl.razerzone.com 56, assets2.razerzone.com 19, Razer Hybris/phoenix/warranty-S3 15, one third-party host) | **Razer's copyright**; PRODUCT.md forbids Razer marks in branding | do not bundle or hotlink | **skip** |
| Golden-byte fixtures | chromacommon report builders (above) | facts | extend uncoil's byte tests to every new command | **now**, with each feature |
| Wireshark dissector | RGB `Documentation/OpenRGB SDK Wireshark Dissector.lua` | GPL-2.0-or-later | debugging the SDK client/server | use, do not vendor |
| razer_test device JSON | z3ntu/razer_test `data/devices/*.json` | no SPDX; 7 devices; stale | none | skip |

---

## 6. UX ideas and anti-patterns

Steal:
- **Tray first** (Polychromatic): battery per device, quick effect, brightness, pause. Notifications as in §1.5.
- **Troubleshooter / process viewer** (Polychromatic `controller/troubleshooter.py`, `procviewer.py`): name the
  program that holds the device (Synapse, OpenRGB, SignalRGB, iCUE, Armoury Crate) with one "how to quit it" line.
  uncoil's `checks` page is the natural home.
- **Visual Map** (OpenRGB plugin): drag devices / single LEDs on a grid, "hide member controllers so they don't
  conflict". uncoil's desk already does this for Razer; external OpenRGB tiles should reuse the same editor.
- **Effects plugin**: per-effect FPS, device/zone checklist per effect, effect search, live preview widget.
- **Per-app profiles** (Artemis profile activation conditions; uncoil roadmap): foreground-window hook
  (SetWinEventHook EVENT_SYSTEM_FOREGROUND) → profile switch; keep rules to "exe name → profile".
- **Battery thresholds as one setting** (OpenRazer razer.conf: enabled / frequency / percent).

Avoid:
- OpenRGB's device tree with every zone, mode and "resize zone" exposed up front; long blocking detection on start;
  requiring the user to run everything as admin (uncoil already isolates elevation in the daemon).
- Artemis' node/data-binding editor complexity for basic use (and its non-commercial licence: UX reference only).
- SignalRGB-style accounts, paid tiers, telemetry and ads; Synapse-style login and cloud sync.
- Hotlinking vendor images (Razer CDN) or showing serial numbers in the UI.
- Writing to onboard memory on every UI change (flash wear): keep VARSTORE writes behind explicit "save to
  device", as uncoil already does with `--write`.

Ecosystem idea (not from OR/RGB, **later**, L): Razer Chroma SDK game integration. Artemis' wrapper plugin
captures Chroma-enabled games (README credits Aurora and lightfx-extender); no licence on that repo, so ideas only.
Would need a replacement `RzChromaSDK64.dll` or the Chroma REST API; risky with Synapse installed.

---

## 7. Licensing (verified from files, not GitHub's badge)

- **OpenRazer**: every source header is `SPDX-License-Identifier: GPL-2.0-or-later` (76 files, e.g.
  `driver/razerkbd_driver.c:1`, `driver/razerchromacommon.c:1`, `daemon/.../hardware/keyboards.py:1`);
  `LICENSES/` has `GPL-2.0-or-later.txt` and `CC-BY-SA-4.0.txt` (the latter only for `logo/*.svg.license`);
  `pylib/setup.py:17` says `GPLv2+`. GitHub's API shows "GPL-2.0", which is misleading.
- **OpenRGB**: 1840 files GPL-2.0-or-later (e.g. `NetworkProtocol.h`, `NetworkServer.cpp`, `RazerDeviceGuard.cpp`);
  18 files **GPL-2.0-only** (`Controllers/HIDLampArrayController` 5, `LianLiController/LianLiUniHubSLController` 4,
  `MountainKeyboardController` 4, `GigabyteRGBFusion2USBController` 2, `qt/ManualDevicesSettingsPage` 3); bundled
  mbedtls Apache-2.0 (79). Plugins (Effects, Visual Map) GPL-2.0-or-later.
- **uncoil**: GPL-3.0-or-later (`LICENSE` is GPLv3 text; README badge, PRODUCT.md:46, CONTRIBUTING.md:99).

What can be reused:
- GPL-2.0-or-later code (OpenRazer, most of OpenRGB, the OpenRGB plugins) **may be copied as code** into uncoil,
  which then distributes it under GPL-3.0-or-later; keep the original copyright/SPDX notices on copied parts.
  uncoil's "facts only" rule is stricter than the licences require, which is fine.
- GPL-2.0-only code (the 18 OpenRGB files, the `openrgb`/`openrgb2` crates) **cannot** be combined with GPL-3.0
  code: facts only.
- GPL-3.0-only (Polychromatic) and GPL-3.0-or-later (RazerGenie, libopenrazer) can be included; Polychromatic
  files stay GPL-3.0-only.
- Apache-2.0 is compatible with GPL-3.0 (one-way).
- PolyForm Noncommercial (Artemis) and unlicensed repos (Artemis wrappers): ideas only, no code or assets.
- Protocol facts (command bytes, packet layouts, ids) are not copyrightable expression; citing file/line as the
  research notes do is good practice.
- Razer product images and marks: never bundle; trademark use only to identify hardware (README already says so).

Doc fix spotted: `README.md:207-208` says OpenRazer and OpenRGB are "both GPL-2.0"; both are
**GPL-2.0-or-later**, which is what makes the code-level compatibility above possible. Same wording in
`razer-devices-notes.md:5-7` ("GPL-2.0" for OpenRazer).

---

## 8. Skip list (with reasons)

- Analog actuation / rapid trigger: no source in any project (§2.1).
- Tilt hwheel / tilt repeat: Linux driver input translation, not a device feature.
- Blade laptop features (fn toggle, Blade brightness), keypads' profile LEDs, 2012-2014 classic LED commands.
- OpenRGB `.orp` / JSON profiles: different model (per-controller modes), no import value.
- `openrgb` / `openrgb2` crates as dependencies: GPL-2.0-only (and tokio).
- razer_test, RazerGenie layouts: stale / generic.
- Product images from Razer's CDN.
