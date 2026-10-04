# Security policy

## Supported versions

uncoil is pre-release. Security fixes go into `main` and the most recent release once releases exist.
Older builds are not patched; update instead.

| Version | Supported |
|---|---|
| `main` | yes |
| latest release (none yet) | yes, once published |
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

uncoil is a daemon that talks raw HID to peripherals and, in the default install, **runs elevated**. That
combination is where the interesting bugs live.

### Trust boundaries

- **Elevated daemon, user-writable inputs.** `scripts/install-task.ps1` registers a logon task with
  "run with highest privileges" so OpenRGB can reach RAM lighting over SMBus. The elevated `uncoild` then
  reads files that any process running as the same user can write: `%APPDATA%\uncoil\config.json` and
  `%APPDATA%\uncoil\devices\*.toml`. Everything read from those files is untrusted input to a
  high-integrity process. Parsing bugs, path handling and anything that turns config into process launches
  or file writes are in scope.
- **Elevated writes into user-writable folders.** The daemon writes `status.json` and `uncoild.log` under
  `%LOCALAPPDATA%\uncoil`. Redirection attacks (junctions, symlinks, hard links) that turn those writes into
  writes elsewhere are in scope.
- **IPC between the app and the daemon.** Today the desktop app and the daemon communicate only through the
  files above. If a named-pipe (or other) control channel is added, it is a trust boundary from day one: the
  pipe must be created with a DACL limited to the interactive user, reject remote clients, and treat every
  message as untrusted input to an elevated process. Weaknesses there are in scope.
- **Raw HID.** The daemon and the reference tools send feature reports to devices. Anything that lets an
  untrusted party choose the device, the command or its arguments (for example a crafted device file aimed
  at another vendor's hardware, or commands that write onboard memory) is in scope.
- **External processes.** The optional OpenRGB hand-off launches `C:\Program Files\OpenRGB\OpenRGB.exe` and
  `taskkill`. Search-path or argument injection issues there are in scope.

### Key presses and audio level (reactive and audio effects)

The reactive, ripple and audio meter effects read input from Windows. Because the daemon runs elevated and
all day, these are held to hard rules, and any way around them is in scope:

- **Key presses.** Only while the active config uses `reactive` or `ripple` (on its own or in an enabled
  studio layer), `uncoild` registers for keyboard Raw Input (`RIDEV_INPUTSINK`, on a message-only window
  in `crates/uncoil-hid/src/keys.rs`). Each event's scan code is turned into a desk position on the spot
  (`apps/uncoild/src/inputs.rs`) and then dropped. Which key was pressed is never logged, written to disk,
  sent over the control pipe or kept as a sequence; memory holds at most 64 `(x, y, time)` entries from
  the last 5 seconds, and they are cleared when the listener stops. The log records only "key listener
  on/off". The listener is unregistered on the config reload that stops needing it.
- **Audio.** Only while `audio_meter` is in use, the daemon reads the default playback device's peak level
  (`IAudioMeterInformation::GetPeakValue`, one number 0..1) at frame rate. No audio samples are captured,
  and nothing about audio is stored.

A change that logs key identities, keeps a key history, exposes presses outside the daemon, or keeps the
listener running when no effect needs it is a security bug.

### Elevation

The logon task runs elevated so OpenRGB can reach RAM lighting over SMBus. Its binary is installed to
`%ProgramFiles%\uncoil`, which only administrators can write, so an unelevated process can't swap it.
Builds before 0.1.0 installed to `%LOCALAPPDATA%\uncoil\bin`; re-running `scripts\install-task.ps1` moves
it and deletes the old copy. Dropping elevation entirely when OpenRGB RAM control isn't wanted is planned.

### Out of scope

- Attacks that need administrator rights or physical access to begin with.
- Malicious firmware on the peripheral itself.
- Bugs in Synapse, OpenRGB, OpenRazer or Windows. Report those upstream.
- The reference scripts in `tools/reference` doing exactly what they say (they are developer tools that
  write to hardware on purpose).
- Denial of service that only turns the lights off.
