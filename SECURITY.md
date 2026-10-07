# Security policy

## Supported versions

uncoil is pre-release (0.x). Security fixes go into `main` and the most recent release.
Older builds are not patched; update instead.

| Version | Supported |
|---|---|
| `main` | yes |
| the latest release | yes |
| anything older | no |

## Reporting a vulnerability

Please report privately. Do not open a public issue, discussion or pull request for a security problem.

1. **Preferred:** GitHub private vulnerability reporting, under the repository's
   [Security tab → Report a vulnerability](https://github.com/justinthevoid/uncoil/security/advisories/new).
2. **Or email** <justin@justinthevoid.com> with "uncoil security" in the subject.

Include the version or commit, your Windows version, the steps to reproduce, and what an attacker gains.
A proof of concept helps but isn't required. Don't include HID captures that contain your keystrokes.

This is a one-maintainer project, so timelines are best effort: an acknowledgement within a week, an
assessment within two, and a fix or mitigation plan agreed with you before anything is disclosed. You'll
be credited in the advisory unless you'd rather not be.

## Threat model and scope

uncoil is a daemon that talks raw HID to peripherals, runs all day as the logged-in user and takes commands
over a named pipe. By default it runs **unelevated**; two optional pieces run elevated. That is where the
interesting bugs live.

### Trust boundaries

- **What runs elevated.** `scripts/install-task.ps1` registers the daemon as a logon task with run level
  Limited: `uncoild` runs as you, unelevated. Only two optional things run elevated:
  - the `uncoil-openrgb` task (`install-task.ps1 -OpenRgb`), which runs `uncoild --openrgb-once` at logon:
    in `hardware` mode it runs OpenRGB once and exits; in `live` mode it keeps OpenRGB running as an SDK
    server for as long as OpenRGB runs (see External processes below);
  - the daemon itself, if it was installed with `-Elevated` (the fallback for PCs where it cannot open its
    devices unelevated; none seen yet).

  Both read `%APPDATA%\uncoil\config.json`, which any program running as you can write, and the `-Elevated`
  daemon also reads `%APPDATA%\uncoil\devices\*.toml`. In those two cases everything read from those files
  is untrusted input to a high-integrity process. Parsing bugs, path handling and anything that turns config
  into process launches or file writes are in scope.
- **Elevated writes into user folders.** The daemon writes `status.json`, `uncoild.log` and the
  onboard-write journal `onboard-writes.jsonl` under `%LOCALAPPDATA%\uncoil`. Unelevated, a junction there can
  only send those writes where you could write anyway. In `-Elevated` mode the daemon turns on the
  redirection-trust mitigation (`ProcessRedirectionTrustPolicy`: junctions made by non-administrators are not
  followed) and refuses to write its log, status or journal into a folder that is a reparse point, through a
  link, or into a file with other hard links; the refusal is reported once on stderr and the write is
  skipped. The `--openrgb-once` task writes only `%ProgramData%\uncoil\openrgb\OpenRGB.json` (live mode),
  inside the admin-only folder: it merges every Razer detector off and the server's host and port into
  whatever settings are there, writes a temporary file opened without following links and refused if it has
  other hard links, then renames it into place. Redirection attacks (junctions, symlinks, hard links) that
  still turn an elevated write into a write elsewhere are in scope.
- **The control pipe.** The desktop app and the `uncoil` CLI talk to the daemon over `\\.\pipe\uncoil`
  (newline-delimited JSON). Its security descriptor is `D:P(A;;GA;;;<user SID>)S:(ML;;NWNR;;;ME)`: full
  access for the user the daemon runs as and nobody else, with a medium integrity label (no write up, no
  read up) so that user's unelevated programs can still connect to an `-Elevated` daemon. Remote clients are
  rejected (`PIPE_REJECT_REMOTE_CLIENTS`). The pipe is created with `FILE_FLAG_FIRST_PIPE_INSTANCE` and is
  the daemon's single-instance lock: if anything already holds the name, `uncoild` logs it and exits before
  opening a device. The daemon serves at most 8 clients, takes request lines up to 64 KB and closes a
  connection that sends nothing for 5 minutes. Clients connect with identification-level impersonation only
  (the daemon can tell who is calling but cannot act as them), refuse a server whose process does not run
  as the same user, and accept replies up to 1 MB. Release builds ignore `UNCOIL_PIPE`; only builds with the
  `fake` feature let it move the pipe. Note that any program running as you can connect, so it can do what
  the CLI can, including writes to a device's memory (which need `"write": true`). Ways past the DACL,
  parser bugs, and ways to reach something the documented commands don't allow are in scope. Details:
  [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#who-may-talk-to-the-daemon).
- **Raw HID.** The daemon and the reference tools send feature reports to devices. Device files are
  validated before use: vendor id `0x1532` only, key map get/set pairs `0x8C`/`0x0C` or `0x8D`/`0x0D` only,
  known transaction ids, size limits on matrices, layouts and lists, ids of `[a-z0-9-]`, and names without
  control characters. A file in the user's devices folder is experimental unless it sets `support`, so its
  writes wait for the read-only checks. Every report sent on a read path (gets, dumps, checks, probes) must
  be a "get" command. Anything that lets an untrusted party choose the device, the command or its arguments
  (for example a crafted device file aimed at another vendor's hardware, or a pipe request that writes
  onboard memory without `"write": true`) is in scope.
- **External processes.** OpenRGB is off by default (`openrgb.mode`, or the older
  `openrgb_hardware_rainbow`). Both modes run from the elevated `uncoil-openrgb` task (or, in `-Elevated`
  mode, from the daemon). The hardware hand-off has no built-in device names: it does nothing until
  `openrgb.devices` in `config.json` lists some. Each `match` and `mode`
  must be a plain name: 1 to 64 letters, digits, spaces and `-_.()+#&:/`, not starting with `-` or a space
  and not ending with a space; other entries are skipped. It runs
  `C:\Program Files\OpenRGB\OpenRGB.exe --noautoconnect --config %ProgramData%\uncoil\openrgb -d … -m …`,
  and only if that folder is owned by Administrators or SYSTEM, is not a reparse point and cannot be changed
  by non-administrators (the installer creates it that way). `tasklist` and `taskkill` are started by full
  path from the Windows system folder, and `taskkill` only closes an OpenRGB in the current session. The
  task reports through its exit code (Task Scheduler's last result: 0 ran and exited cleanly, 1 turned off
  or nothing configured, 2 failed). Search-path or argument injection issues there are in scope.

  **Live mode: the OpenRGB SDK server has no authentication. While it runs, any program on this PC can
  change the motherboard, RAM and GPU lighting through it (that is OpenRGB's design, not something uncoil
  can add).** uncoil starts it bound to 127.0.0.1 only, both on the command line
  (`--server --server-host 127.0.0.1 --server-port N`, N from `openrgb.live.port`, 1024–65535) and as
  `Server.default_host` in its settings; an OpenRGB too old to know `--server-host` rejects the whole command
  line rather than listening anywhere else. Every Razer detector is turned off, so that OpenRGB never touches
  the Razer devices. A job object ends OpenRGB when the task stops or exits. The daemon (unelevated) connects
  to that port as a client and treats the server as untrusted input, because while OpenRGB is not running any
  local program could listen there: packets are capped at 8 MiB, at most 64 controllers are taken and one
  with more than 4096 LEDs is refused, names lose control characters, and serial numbers are skipped without being stored. The task
  marks its server as uncoil's with a named object, `Global\uncoil-openrgb-server`; a program running as you
  could fake that name, and the worst it does is hide the "OpenRGB is running" notice. Ways to make the
  server listen beyond 127.0.0.1, or to turn what a fake server sends into more than wrong colours, are in
  scope.
- **Other programs on the same devices.** Around every request to a Razer device the daemon takes the named
  mutex `Global\RazerLinkReadWriteGuardMutex` (OpenRGB's `RazerDeviceGuard.cpp` uses the same one). uncoil
  creates it with default security, or opens one another program created, or runs without it. Any program
  running as you can hold it, which makes uncoil skip frames and fail commands with "another program is
  talking to … right now"; that is a nuisance, not an escalation. To spot conflicting programs, the daemon
  takes a process snapshot at every rescan and compares only the image names against a short list (Synapse,
  the Chroma SDK services, OpenRGB, SignalRGB); only the matching program's name is kept, shown or logged.

### Key presses and audio level (reactive and audio effects)

The reactive, ripple and audio meter effects read input from Windows. Because the daemon runs all day, these
are held to hard rules, and any way around them is in scope:

- **Key presses.** Only while the active config uses `reactive` or `ripple` (on its own or in an enabled
  studio layer), `uncoild` registers for keyboard Raw Input (`RIDEV_INPUTSINK`, on a message-only window
  in `crates/uncoil-hid/src/keys.rs`). Each event's scan code is turned into a desk position on the spot
  (`apps/uncoild/src/inputs.rs`) and then dropped. Which key was pressed is never logged, written to disk,
  sent over the control pipe or kept as a sequence; memory holds at most 64 `(x, y, time)` entries from
  the last 5 seconds, and the whole buffer is overwritten with zeros when the listener stops. A position
  does map back to a key on your layout, so this short buffer is sensitive; it never leaves the daemon's
  memory. The log records only "key listener on", "off" or "unavailable". The listener is unregistered on
  the config reload that stops needing it.
- **Audio.** Only while `audio_meter` is in use, the daemon reads the default playback device's peak level
  (`IAudioMeterInformation::GetPeakValue`, one number 0..1) at frame rate. No audio samples are captured,
  and nothing about audio is stored.

A change that logs key identities, keeps a key history, exposes presses outside the daemon, or keeps the
listener running when no effect needs it is a security bug.

### Elevation

The daemon's logon task runs unelevated (run level Limited). Two install options add elevation:

- `-OpenRgb` registers `uncoil-openrgb`, an elevated logon task for OpenRGB (RAM lighting sits on the
  SMBus, which needs administrator rights), and creates the admin-only `%ProgramData%\uncoil\openrgb` folder
  it requires. In `hardware` mode it is a one-shot; in `live` mode it keeps running, with no time limit, as
  long as OpenRGB's server does, and stopping the task ends that OpenRGB.
- `-Elevated` runs the daemon itself elevated, as installs did before; only for PCs where it cannot open its
  devices unelevated.

Installing still takes one UAC prompt, because the binary goes to `%ProgramFiles%\uncoil`, which only
administrators can write, so no program running as you can swap it. The installer checks the copy's SHA-256
against the source after copying and refuses an install path that runs through a junction or link. Builds
before 0.1.0 installed to `%LOCALAPPDATA%\uncoil\bin`; the installer no longer touches that copy and only
prints a note that it can be deleted.

### Out of scope

- Attacks that need administrator rights or physical access to begin with.
- Malicious firmware on the peripheral itself.
- Bugs in Synapse, OpenRGB, OpenRazer or Windows. Report those upstream.
- The reference scripts in `tools/reference` doing exactly what they say (they are developer tools that
  write to hardware on purpose).
- Denial of service that only turns the lights off.
