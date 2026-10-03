"""Physical LED positions for the reference desk, in key units (1u = 19.05 mm), y grows toward you.

Keyboard: Razer BlackWidow V4 Pro 75% (ANSI). The firmware matrix is 6x18, but the 18 underglow
LEDs are packed into spare slots (e.g. three LEFT underglow LEDs sit at top-row cols 14-16, three
RIGHT ones in the bottom row around the spacebar). Map below comes from OpenRGB's
razer_blackwidow_v4_pro_75_layout + a dump of the live device (razer_layout_dump.json).
"""

# (row, col) in the firmware matrix -> name, exactly as the device reports it
KB_MATRIX = [
    ['LU0', 'Escape', 'F1', 'F2', 'F3', 'F4', 'F5', 'F6', 'F7', 'F8', 'F9', 'F10', 'F11', 'F12', 'LU1', 'LU5', 'LU8', 'RU0'],
    ['LU2', '`', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=', None, 'Backspace', 'Delete', 'RU2'],
    ['LU3', 'Tab', 'Q', 'W', 'E', 'R', 'T', 'Y', 'U', 'I', 'O', 'P', '[', ']', '\\', None, 'Page Up', 'RU3'],
    ['LU4', 'Caps Lock', 'A', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L', ';', "'", None, 'Enter', None, 'Page Down', 'RU4'],
    ['LU6', 'Left Shift', None, 'Z', 'X', 'C', 'V', 'B', 'N', 'M', ',', '.', '/', None, 'Right Shift', 'Up Arrow', 'Insert', 'RU6'],
    ['LU7', 'Left Control', 'Left Windows', 'Left Alt', None, 'RU1', 'RU5', 'Space', 'RU8', None, None, 'Right Alt', 'Right Fn', 'Right Control', 'Left Arrow', 'Down Arrow', 'Right Arrow', 'RU7'],
]

# Physical rows: (y, [(name, width_u) | ('gap', width_u), ...]) left to right from x = 0
_ROWS = [
    (0.0,  [('Escape', 1), ('gap', .25), ('F1', 1), ('F2', 1), ('F3', 1), ('F4', 1), ('gap', .25),
            ('F5', 1), ('F6', 1), ('F7', 1), ('F8', 1), ('gap', .25), ('F9', 1), ('F10', 1), ('F11', 1), ('F12', 1)]),
    (1.25, [('`', 1), ('1', 1), ('2', 1), ('3', 1), ('4', 1), ('5', 1), ('6', 1), ('7', 1), ('8', 1), ('9', 1),
            ('0', 1), ('-', 1), ('=', 1), ('Backspace', 2), ('gap', .25), ('Delete', 1)]),
    (2.25, [('Tab', 1.5), ('Q', 1), ('W', 1), ('E', 1), ('R', 1), ('T', 1), ('Y', 1), ('U', 1), ('I', 1), ('O', 1),
            ('P', 1), ('[', 1), (']', 1), ('\\', 1.5), ('gap', .25), ('Page Up', 1)]),
    (3.25, [('Caps Lock', 1.75), ('A', 1), ('S', 1), ('D', 1), ('F', 1), ('G', 1), ('H', 1), ('J', 1), ('K', 1),
            ('L', 1), (';', 1), ("'", 1), ('Enter', 2.25), ('gap', .25), ('Page Down', 1)]),
    (4.25, [('Left Shift', 2.25), ('Z', 1), ('X', 1), ('C', 1), ('V', 1), ('B', 1), ('N', 1), ('M', 1), (',', 1),
            ('.', 1), ('/', 1), ('Right Shift', 1.75), ('gap', .25), ('Up Arrow', 1), ('Insert', 1)]),
    (5.25, [('Left Control', 1.25), ('Left Windows', 1.25), ('Left Alt', 1.25), ('Space', 6.25), ('Right Alt', 1),
            ('Right Fn', 1), ('Right Control', 1), ('gap', .25), ('Left Arrow', 1), ('Down Arrow', 1), ('Right Arrow', 1)]),
]

KB_WIDTH = 16.25
KB_DEPTH = 6.25


def _key_geometry():
    """name -> (x0, y0, w, h) rectangle of each key."""
    rects = {}
    for y, items in _ROWS:
        x = 0.0
        for name, w in items:
            if name != 'gap':
                rects[name] = (x, y, w, 1.0)
            x += w
    return rects


KEY_RECTS = _key_geometry()


def _underglow(side, i):
    # 9 LEDs per side, numbered back (0) to front (8), running just outside the case edge
    y = 0.2 + i * (KB_DEPTH - 0.4) / 8
    x = -0.55 if side == 'L' else KB_WIDTH + 0.55
    return (x, y)


def led_position(name):
    if name is None:
        return None
    if name[:2] in ('LU', 'RU') and name[2:].isdigit():
        return _underglow(name[0], int(name[2:]))
    x0, y0, w, h = KEY_RECTS[name]
    return (x0 + w / 2, y0 + h / 2)


def keyboard_positions():
    """[row][col] -> (x, y) or None, matching the firmware matrix."""
    return [[led_position(n) for n in row] for row in KB_MATRIX]


# Mouse: Basilisk V3 Pro to the right of the keyboard (~130 x 75 mm => ~6.8u x 3.9u footprint;
# the lit body is narrower). OpenRGB LED order: 0 scroll wheel, 1 logo, 2..12 underglow strip.
MOUSE_CX, MOUSE_CY = KB_WIDTH + 4.5, 3.1
MOUSE_HALF_W, MOUSE_HALF_L = 1.4, 3.2


def mouse_positions(n=13):
    cx, cy, hw, hl = MOUSE_CX, MOUSE_CY, MOUSE_HALF_W, MOUSE_HALF_L
    pts = [(cx, cy - hl + 1.0), (cx, cy + hl - 1.4)]          # wheel near the front, logo on the palm
    strip = n - 2
    # underglow runs down the left side, around the back, and up the right side
    for i in range(strip):
        t = i / max(1, strip - 1)
        if t < 0.4:
            pts.append((cx - hw, cy - hl + 0.6 + (t / 0.4) * (2 * hl - 1.0)))
        elif t < 0.6:
            pts.append((cx - hw + ((t - 0.4) / 0.2) * 2 * hw, cy + hl - 0.2))
        else:
            pts.append((cx + hw, cy + hl - 0.4 - ((t - 0.6) / 0.4) * (2 * hl - 1.0)))
    return pts


# Mat: Goliathus Chroma Extended, one LED (the whole edge strip is a single colour). It sits under
# everything, so it takes the colour at the centre of the desk.
MAT_RECT = (-2.0, -1.5, KB_WIDTH + 10.0, KB_DEPTH + 3.5)    # x0, y0, w, h (for previews)
MAT_POS = (MAT_RECT[0] + MAT_RECT[2] / 2, MAT_RECT[1] + MAT_RECT[3] / 2)
