# Getting help

uncoil is a pre-release, one-maintainer project. Help is best effort, and the fastest route is usually a
well-filed issue.

## Before you ask

1. **Is Synapse running?** Quit it, including its tray icon and startup entry. Two programs driving the
   same device is the most common cause of flicker, frozen frames and dead media keys.
2. **Is the daemon running?** `Get-ScheduledTask uncoil` and `Get-Process uncoild`. If the task exists but
   the process doesn't, run `Start-ScheduledTask uncoil`.
3. **Read the log** at `%LOCALAPPDATA%\uncoil\uncoild.log`. It says which devices were found, on which
   interface, and what failed.
4. **Dial or media keys dead after leaving Synapse?** The device is probably still in driver mode. Starting
   uncoil fixes it; so does unplugging the device for a few seconds.
5. Check [open and closed issues](https://github.com/justinthevoid/uncoil/issues?q=is%3Aissue) and the
   quirks in [docs/PROTOCOL.md](docs/PROTOCOL.md).

## Where to go

| You want to | Go to |
|---|---|
| Report something broken | [Bug report](https://github.com/justinthevoid/uncoil/issues/new?template=bug_report.yml) |
| Get a device supported | [Device support request](https://github.com/justinthevoid/uncoil/issues/new?template=device_support.yml) |
| Suggest a feature | [Feature request](https://github.com/justinthevoid/uncoil/issues/new?template=feature_request.yml) |
| Ask a question or share a setup | [Discussions](https://github.com/justinthevoid/uncoil/discussions), if enabled; otherwise an issue |
| Report a security problem | [SECURITY.md](SECURITY.md), privately |

When you post a log or screenshot, remove serial numbers, your Windows username and anything else personal.
Never attach HID captures that contain typed keys.

## What isn't supported

- Razer software, accounts, warranties or firmware updates. uncoil is independent of Razer; contact Razer for
  those.
- macOS and Linux. On Linux, [OpenRazer](https://openrazer.github.io) is the mature option.
- Devices not listed in the README, until someone adds a device file. That someone could be you; see
  [CONTRIBUTING.md](CONTRIBUTING.md#adding-a-device).
