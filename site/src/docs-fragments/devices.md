<!-- Body of /docs/devices/, rendered by src/pages/docs/devices.astro under the generated device table. -->

Both tables are generated from the files in [`devices/`](https://github.com/justinthevoid/uncoil/tree/main/devices)
and [`devices/experimental/`](https://github.com/justinthevoid/uncoil/tree/main/devices/experimental), the same
files compiled into `uncoild`. The experimental files are written by `tools/devices/gen_experimental.py` from the
OpenRazer / OpenRGB research; each one says in comments where its values came from and how sure they are.
"Device effects only" means the sources disagree about per-LED lighting, so uncoil only uses the device's own
effects until someone confirms more. Notes on the supported devices:

- **BlackWidow V4 Pro 75%:** wired only for now. The wireless (HyperSpeed dongle) variant is known to OpenRGB
  as `1532:02B4` but hasn't been tested, so it isn't listed. Its 18 underglow LEDs live in odd matrix slots;
  see [Protocol](/docs/protocol/#quirks-the-expensive-lessons).
- **Basilisk V3 Pro:** wired and HyperSpeed dongle. The dongle answers "no answer" (`0x04`) while the mouse is
  on its cable; uncoil treats that as "not here yet" and picks the mouse up on whichever path answers.
- **Goliathus Chroma Extended:** the whole edge strip is one LED, sampled at the centre of the mat.

## Adding a device

Supporting a device should be a file, not a fork. A device definition says how to reach the device over USB,
which quirks it has, what its LED matrix looks like, and where each LED physically sits.

You can try a definition without rebuilding anything: drop the `.toml` into `%APPDATA%\uncoil\devices\` and
restart the `uncoil` task. Files there are loaded after the built-in ones, and a file with the same `id` as a
built-in replaces it. Once it works, open a pull request adding it to `devices/` so it ships for everyone.

### Where the facts come from

1. **OpenRGB and OpenRazer** already know most Razer devices: product id, interface, the LED matrix size and
   order, and the transaction id. Their source is the first place to look.
2. **Synapse's own logs**, if you still have Synapse installed, record every command it sends to the device,
   with the raw bytes. `tools/reference/mine_synapse_logs.py` turns them into a catalog. See
   [Protocol](/docs/protocol/#how-this-was-learned).
3. **The device itself.** The small Python probes in `tools/reference/` send single commands and read back the
   status codes, and `preview.py` renders an effect onto a layout as an animated GIF, so a misplaced LED shows.

### Anatomy of a device file

The smallest real example, the Goliathus mat:

```toml title="devices/razer-goliathus-chroma-extended.toml"
id = "razer-goliathus-chroma-extended"   # stable id, used in config.json "desk"
name = "Razer Goliathus Chroma Extended"
kind = "mousemat"                        # keyboard | mouse | mousemat | headset | other
vendor_id = 0x1532

[[usb]]                                  # one entry per way of connecting
product_id = 0x0C02
connection = "wired"                     # free text: wired, dongle, ...
interface = 0                            # HID interface, usage page and usage pick the
usage_page = 0x01                        # collection that answers feature reports
usage = 0x02
transaction_id = 0x3F                    # 0x1F for most keyboards and mice

[quirks]
ack_every_report = false                 # read the reply after every report
custom_mode_once = true                  # send the custom-frame effect once, not per frame

[matrix]
rows = 1
cols = 1
names = [["Edge"]]                       # LED name per (row, col); "" = no LED in that slot

[layout]
type = "points"                          # explicit LED points, relative to the device centre
width = 26.25                            # footprint in key units (1u = 19.05 mm)
depth = 9.75
points = [[0.0, 0.0]]                    # one [x, y] per LED, in matrix order
```

#### `[[usb]]`

Each entry is one way the device can be connected. uncoil opens the HID collection matching `interface`,
`usage_page` and `usage`, and talks to it with the given `transaction_id`. A wireless mouse usually has two
entries: the cable and the dongle.

#### `[quirks]`

| Key | Default | When to set it |
|---|---|---|
| `ack_every_report` | `false` | The device silently stops applying frames after a while unless each reply is read back. The BlackWidow needs this. |
| `custom_mode_once` | `true` | Re-sending the custom-frame effect every frame freezes the device on its first frame. True for everything seen so far. |

#### `[matrix]`

The LED matrix as the firmware addresses it: `rows` × `cols`, with the name of the LED in each slot and `""`
where there is none. Names matter for keyboards, where they are matched to the physical key layout.

#### `[layout]`: points

For mice, mats and anything that isn't a keyboard. `points` lists an `[x, y]` per LED, in key units relative
to the device centre, in row-major matrix order. The Basilisk's strip, for example, runs down the left side,
around the back and up the right.

#### `[layout]`: keyboard

Keyboards describe their physical rows instead, and LEDs are matched to keys by name:

```toml
[layout]
type = "keyboard"
width = 16.25
depth = 6.25

[[layout.rows]]
y = 0.0
keys = ["Escape:1", "gap:0.25", "F1:1", "F2:1", "F3:1", "F4:1", "gap:0.25", "F5:1"]   # "name:width"

# Underglow LEDs just outside the case, back to front, named prefix + index (LU0..LU8, RU0..RU8)
[layout.underglow]
left = { prefix = "LU", x = -0.55, count = 9 }
right = { prefix = "RU", x = 16.8, count = 9 }
y_start = 0.2
y_end = 6.05
```

Each row lists keys left to right from `x = 0` as `"name:width"` in key units; `gap:w` is empty space. The
`name` must match the name used in `[matrix]`.

### Checking a new definition

- `cargo test -p uncoil-core` parses and validates every built-in definition and checks that every named
  keyboard slot lands on the desk.
- The desktop app's Lighting view draws each placed device with its LEDs coloured by the live effect, which
  shows a wrong position or a mirrored strip at a glance.
- `%LOCALAPPDATA%\uncoil\uncoild.log` says whether the device was found and opened.

If a device needs more than data (a new command, a new transport behaviour), open an issue with what you found;
see [Contributing](/docs/contributing/).
