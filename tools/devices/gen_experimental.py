#!/usr/bin/env python3
"""Generate devices/experimental/*.toml from the device research.

Inputs (all in the repository):
  tools/reference/device-research/razer-devices.json   facts from OpenRazer @a84cd0ae / OpenRGB @df3024be
  tools/devices/keyboard-matrices.json                 per-key matrix tables (OpenRGB layouts, applied)
  tools/devices/overrides.json                         hand-written choices, each with a reason
  docs/blackwidow-keyids.json                          Synapse names of the AT-101 key ids

Output: one devices/experimental/<id>.toml per device, support = "experimental". Files are rewritten every run;
change the inputs, not the output. Run from anywhere:

  python tools/devices/gen_experimental.py            write the files
  python tools/devices/gen_experimental.py --check    exit 1 if any file would change

Only facts (ids, sizes, names, numbers) come from OpenRazer / OpenRGB; nothing here is their code.
"""

from __future__ import annotations

import json
import re
import sys
import textwrap
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RESEARCH = ROOT / "tools/reference/device-research/razer-devices.json"
MATRICES = Path(__file__).with_name("keyboard-matrices.json")
OVERRIDES = Path(__file__).with_name("overrides.json")
KEYIDS = ROOT / "docs/blackwidow-keyids.json"
OUT = ROOT / "devices/experimental"

OPENRAZER_COMMIT = "a84cd0ae"
OPENRGB_COMMIT = "df3024be"

# Contract section 2 command groups, in file order. frame/effect are "lighting" groups.
GROUPS = ["frame", "effect", "keymap", "profile", "dpi", "poll", "power", "low_battery", "device"]
LIGHTING_GROUPS = {"frame", "effect"}

EFFECT_ORDER = ["off", "static", "breathing", "spectrum", "wave", "wheel", "reactive", "starlight"]
FEATURE_ORDER = ["lighting", "hw_effects", "keymap", "profiles", "dpi", "poll_rate", "power"]

HYPERPOLLING_CODES = {8000, 4000, 2000, 1000, 500, 250, 125}
CLASSIC_CODES = {1000, 500, 125}


class GenError(Exception):
    pass


# ---------------------------------------------------------------------------------------------------- helpers


def h(v: str | int | None) -> int | None:
    if v is None:
        return None
    if isinstance(v, int):
        return v
    return int(v, 16) if v.lower().startswith("0x") else int(v)


def hx(n: int, width: int = 2) -> str:
    return f"0x{n:0{width}X}"


def q(s: str) -> str:
    """TOML basic string (JSON escaping is valid TOML)."""
    return json.dumps(s, ensure_ascii=False)


def num(x: float) -> str:
    s = f"{x:.4f}".rstrip("0").rstrip(".")
    return s + ".0" if "." not in s else s


def wrap(text: str, prefix: str = "# ", width: int = 112) -> list[str]:
    out = []
    for para in text.split("\n"):
        out += textwrap.wrap(para, width=width - len(prefix), break_long_words=False, break_on_hyphens=False) or [""]
    return [(prefix + line).rstrip() for line in out]


def short_daemon(cite: str | None) -> str | None:
    """'OpenRazer daemon/.../keyboards.py:1108 (class RazerBlackWidowV3); METHODS ...' -> 'keyboards.py:1108 RazerBlackWidowV3'."""
    if not cite:
        return None
    m = re.search(r"hardware/(\S+?:\d+) \(class (\w+)\)", cite)
    return f"{m.group(1)} {m.group(2)}" if m else cite


def short_rgb(cite: str | None) -> str | None:
    if not cite:
        return None
    m = re.search(r"(RazerDevices\.cpp:\d+) \((\w+)\)", cite)
    return f"{m.group(1)} {m.group(2)}" if m else cite


def short_detect(det: dict) -> str:
    m = re.search(r"(RazerControllerDetect\.cpp:\d+)", det.get("cite", ""))
    return m.group(1) if m else det.get("cite", "")


# ------------------------------------------------------------------------------------------ transaction ids


def fn_base(name: str) -> str:
    n = name.replace("razer_attr_", "")
    if n == "razer_set_device_mode":
        return "device_mode"
    return re.sub(r"^(read|write)_", "", n)


def group_of(fn: str) -> str | None:
    if fn in ("matrix_custom_frame", "matrix_effect_custom"):
        return "frame"
    if fn.startswith("matrix_effect_") or fn in ("matrix_brightness", "led_brightness", "matrix_reactive_trigger"):
        return "effect"
    if fn in ("dpi", "dpi_stages"):
        return "dpi"
    if fn == "poll_rate":
        return "poll"
    if fn in ("charge_level", "charge_status", "device_idle_time"):
        return "power"
    if fn == "charge_low_threshold":
        return "low_battery"
    if fn in ("device_mode", "device_serial", "firmware_version"):
        return "device"
    return None


def openrazer_groups(drv: dict) -> tuple[dict[str, dict[str, set[int]]], dict[str, set[int]]]:
    """Per contract group: {function: {txn values}} as OpenRazer's driver sends them; plus ungrouped functions."""
    majority = h(drv.get("txn_majority"))
    exc = {fn_base(k): {h(v) for v in vals} for k, vals in (drv.get("txn_exceptions") or {}).items()}
    groups: dict[str, dict[str, set[int]]] = {}
    other: dict[str, set[int]] = {}
    fns = {fn_base(a) for a in (drv.get("attrs") or [])} | set(exc)
    for fn in sorted(fns):
        vals = exc.get(fn, {majority} if majority is not None else set())
        g = group_of(fn)
        if g:
            groups.setdefault(g, {})[fn] = vals
        else:
            other[fn] = vals
    return groups, other


def resolve_txn(u: dict, ov: dict, lit: bool, used_groups: set[str]) -> tuple[int, dict[str, int], list[str]]:
    """Returns (transaction_id, group overrides, comment lines)."""
    src = u["sources"]
    drv = src.get("openrazer_driver") or {}
    rgb = src.get("openrgb") or {}
    rgb_txn = h(rgb.get("transaction_id"))
    pov = ov.get("usb", {}).get(u["product_id"], {})
    notes: list[str] = []

    research = h(u.get("transaction_id"))
    if "transaction_id" in pov:
        default = h(pov["transaction_id"])
        notes.append(pov["why"])
    elif research is not None:
        default = research
        conf = u.get("transaction_id_confidence")
        notes.append(f"transaction_id {hx(default)}: " + ("both sources agree." if conf == "agree" else f"{conf}."))
    else:
        cand = u.get("transaction_id_candidates") or {}
        default = h(drv.get("txn_majority"))
        notes.append(
            f"Sources disagree (OpenRazer {cand.get('openrazer_lighting')}, OpenRGB {cand.get('openrgb')}) and the "
            "research makes no recommendation: lighting commands use OpenRGB's value, everything else OpenRazer's. "
            "Confidence: low."
        )
    if default is None:
        raise GenError(f"{u['product_id']}: no transaction id")

    if "lighting_txn" in pov:
        lighting = h(pov["lighting_txn"])
    elif rgb_txn is not None:
        lighting = rgb_txn
    else:
        lighting = None

    groups, other = openrazer_groups(drv)
    overrides: dict[str, int] = {}
    for g in GROUPS:
        if g not in used_groups:
            continue
        fns = groups.get(g, {})
        vals = set().union(*fns.values()) if fns else set()
        if g in LIGHTING_GROUPS:
            if not lit:
                continue
            chosen = lighting if lighting is not None else (next(iter(vals)) if len(vals) == 1 else default)
            if vals and vals != {chosen}:
                detail = ", ".join(f"{fn} {'/'.join(hx(v) for v in sorted(s))}" for fn, s in fns.items() if s != {chosen})
                notes.append(f"{g}: {hx(chosen)} (OpenRGB / research); OpenRazer sends {detail}.")
            if chosen != default:
                overrides[g] = chosen
        else:
            if not fns:
                continue
            if len(vals) == 1:
                v = next(iter(vals))
                if v != default:
                    overrides[g] = v
                    notes.append(f"{g}: {hx(v)}, as OpenRazer sends {', '.join(fns)}.")
            else:
                detail = ", ".join(f"{fn} {'/'.join(hx(v) for v in sorted(s))}" for fn, s in fns.items())
                notes.append(
                    f"{g}: OpenRazer mixes values ({detail}); one value per group fits, so it stays {hx(default)}."
                )
    odd = {fn: s for fn, s in other.items() if s != {default}}
    if odd:
        detail = ", ".join(f"{fn} {'/'.join(hx(v) for v in sorted(s))}" for fn, s in odd.items())
        notes.append(f"Not used by uncoil, for reference: OpenRazer sends {detail}.")
    if "keymap" in used_groups or "profile" in used_groups:
        notes.append("keymap / profile commands are in neither source; they use transaction_id.")
    return default, overrides, notes


# --------------------------------------------------------------------------------------------- keyboards

# Physical key templates, ANSI, in key units (1u = 19.05 mm): row -> (y, [(name, x, w)]).
def _run(names: list[str], x0: float, w: float = 1.0) -> list[tuple[str, float, float]]:
    return [(n, x0 + i * w, w) for i, n in enumerate(names)]


FULL_ROWS = [
    (0.0, [("Escape", 0, 1)] + _run(["F1", "F2", "F3", "F4"], 2) + _run(["F5", "F6", "F7", "F8"], 6.5)
     + _run(["F9", "F10", "F11", "F12"], 11) + _run(["Print Screen", "Scroll Lock", "Pause"], 15.25)),
    (1.25, _run(["`", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "="], 0) + [("Backspace", 13, 2)]
     + _run(["Insert", "Home", "Page Up"], 15.25) + _run(["Num Lock", "Numpad /", "Numpad *", "Numpad -"], 18.5)),
    (2.25, [("Tab", 0, 1.5)] + _run(["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P", "[", "]"], 1.5)
     + [("\\", 13.5, 1.5)] + _run(["Delete", "End", "Page Down"], 15.25)
     + _run(["Numpad 7", "Numpad 8", "Numpad 9", "Numpad +"], 18.5)),
    (3.25, [("Caps Lock", 0, 1.75)] + _run(["A", "S", "D", "F", "G", "H", "J", "K", "L", ";", "'"], 1.75)
     + [("Enter", 12.75, 2.25)] + _run(["Numpad 4", "Numpad 5", "Numpad 6"], 18.5)),
    (4.25, [("Left Shift", 0, 2.25)] + _run(["Z", "X", "C", "V", "B", "N", "M", ",", ".", "/"], 2.25)
     + [("Right Shift", 12.25, 2.75), ("Up Arrow", 16.25, 1)]
     + _run(["Numpad 1", "Numpad 2", "Numpad 3", "Numpad Enter"], 18.5)),
    (5.25, _run(["Left Control", "Left Windows", "Left Alt"], 0, 1.25) + [("Space", 3.75, 6.25)]
     + _run(["Right Alt", "Right Fn", "Menu", "Right Control"], 10, 1.25)
     + _run(["Left Arrow", "Down Arrow", "Right Arrow"], 15.25) + [("Numpad 0", 18.5, 2), ("Numpad .", 20.5, 1)]),
]
NUMPAD_X = 18.5

# The 75% rows of devices/razer-blackwidow-v4-pro-75.toml (verified on that keyboard).
SEVENTY_FIVE_ROWS = [
    (0.0, [("Escape", 0, 1)] + _run(["F1", "F2", "F3", "F4"], 1.25) + _run(["F5", "F6", "F7", "F8"], 5.5)
     + _run(["F9", "F10", "F11", "F12"], 9.75)),
    (1.25, _run(["`", "1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-", "="], 0) + [("Backspace", 13, 2),
                                                                                        ("Delete", 15.25, 1)]),
    (2.25, [("Tab", 0, 1.5)] + _run(["Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P", "[", "]"], 1.5)
     + [("\\", 13.5, 1.5), ("Page Up", 15.25, 1)]),
    (3.25, [("Caps Lock", 0, 1.75)] + _run(["A", "S", "D", "F", "G", "H", "J", "K", "L", ";", "'"], 1.75)
     + [("Enter", 12.75, 2.25), ("Page Down", 15.25, 1)]),
    (4.25, [("Left Shift", 0, 2.25)] + _run(["Z", "X", "C", "V", "B", "N", "M", ",", ".", "/"], 2.25)
     + [("Right Shift", 12.25, 1.75), ("Up Arrow", 14.25, 1), ("Insert", 15.25, 1)]),
    (5.25, _run(["Left Control", "Left Windows", "Left Alt"], 0, 1.25) + [("Space", 3.75, 6.25)]
     + _run(["Right Alt", "Right Fn", "Right Control"], 10) + _run(["Left Arrow", "Down Arrow", "Right Arrow"], 13.25)),
]


def template(form: str) -> tuple[list[tuple[float, list[tuple[str, float, float]]]], float]:
    if form == "full":
        return [(y, list(keys)) for y, keys in FULL_ROWS], 22.5
    if form == "tkl":
        return [(y, [k for k in keys if k[1] < NUMPAD_X]) for y, keys in FULL_ROWS], 18.25
    if form == "75":
        return [(y, list(keys)) for y, keys in SEVENTY_FIVE_ROWS], 16.25
    if form == "60":
        rows = []
        for y, keys in FULL_ROWS[1:]:
            ks = [("Escape", x, w) if n == "`" else (n, x, w) for n, x, w in keys if x < 15]
            rows.append((y - 1.25, ks))
        return rows, 15.0
    raise GenError(f"unknown form factor {form}")


def build_keyboard_layout(dev_id: str, matrix: list[list[str]], spec: dict) -> dict:
    rows, width = template(spec["form"])
    names = {n for row in matrix for n in row if n}
    shift = 1.5 if spec.get("macro_col") else 0.0
    placed: dict[float, list[tuple[str, float, float]]] = {}
    for y, keys in rows:
        placed[y] = [(n, x + shift, w) for n, x, w in keys]
    ys = [y for y, _ in rows]
    for name, (r, x, w) in spec.get("extras", {}).items():
        placed[ys[r]].append((name, x + shift, w))
    if shift:
        for r, row in enumerate(matrix[: len(ys)]):
            for n in row:
                if re.fullmatch(r"M\d|Dial", n):
                    placed[ys[r]].append((n, 0.0, 1.0))
    wr = spec.get("wrist_rest")
    if wr:
        placed[wr["y"]] = [(f"{wr['prefix']}{i}", wr["x0"] + shift + i * wr["w"], wr["w"]) for i in range(wr["count"])]
    width += shift

    out_rows = []
    keys_placed: set[str] = set()
    for y in sorted(placed):
        items = sorted((k for k in placed[y] if k[0] in names), key=lambda k: k[1])
        if not items:
            continue
        x, specs = 0.0, []
        for n, kx, w in items:
            if kx < x - 1e-6:
                raise GenError(f"{dev_id}: key {n} overlaps the previous key in row y={y}")
            if kx > x + 1e-6:
                specs.append(f"gap:{num(kx - x)}")
            specs.append(f"{n}:{num(w)}")
            if n in keys_placed:
                raise GenError(f"{dev_id}: key {n} placed twice")
            keys_placed.add(n)
            x = kx + w
        out_rows.append((y, specs))
        width = max(width, x)
    depth = max(y for y, _ in out_rows) + 1.0

    underglow = None
    glow: set[str] = set()
    if spec.get("underglow"):
        left = sorted(n for n in names if re.fullmatch(r"LU\d+", n))
        right = sorted(n for n in names if re.fullmatch(r"RU\d+", n))
        underglow = {"left": len(left), "right": len(right), "right_x": width + 0.55}
        glow = set(left) | set(right)
    missing = names - keys_placed - glow
    if missing:
        raise GenError(f"{dev_id}: matrix LEDs with no physical position: {sorted(missing)}")
    return {"rows": out_rows, "width": width, "depth": depth, "underglow": underglow}


# ---------------------------------------------------------------------------------------------- the files


def hw_effect_list(dev: dict, led: int, kind: str) -> tuple[list[str], list[str]]:
    lighting = dev["lighting"]
    whole = [e for e in lighting.get("hw_effects_openrazer_whole_device", []) if not e.startswith(("custom", "ripple"))]
    notes = []
    raw = whole
    if not raw:
        zones = lighting.get("hw_effects_openrazer_zones", {})
        zone = next((z for z in zones.values() if h(z["led_id"]) == led), None) or next(iter(zones.values()), None)
        raw = zone["effects"] if zone else []
        notes.append("OpenRazer has no whole-device effects here; the list is its per-zone list.")
    mapped = set()
    for e in raw:
        e = e.replace("breath_", "breathing_")
        if e == "none":
            mapped.add("off")
        elif e.startswith("breathing"):
            mapped.add("breathing")
        elif e.startswith("starlight"):
            mapped.add("starlight")
        elif e in EFFECT_ORDER:
            mapped.add(e)
    if "off" not in mapped:
        mapped.add("off")
        notes.append("off (effect 0) is not in OpenRazer's list for this device but is accepted by every Razer device.")
    if lighting.get("openrgb_breathing_forced_off"):
        if "breathing" in mapped:
            mapped.discard("breathing")
        notes.append("No breathing: OpenRGB turns firmware breathing off for the Basilisk V3 family.")
    if kind in ("mousemat", "other") and "reactive" in mapped:
        mapped.discard("reactive")
        notes.append("No reactive: it needs key or button input this device does not have.")
    return [e for e in EFFECT_ORDER if e in mapped], notes


def matrix_for(dev: dict, ov: dict, tables: dict, key_names: dict) -> tuple[list[list[str]] | None, list[str]]:
    lighting = dev.get("lighting") or {}
    if not lighting.get("openrgb_zones") and not lighting.get("per_led_custom_frame"):
        return None, []
    notes = []
    kb = dev.get("keyboard") or {}
    table_name = kb.get("openrgb_layout_table")
    if table_name and "matrix" not in ov:
        table = tables[table_name]
        rows = [[key_names.get(n, n) for n in row] for row in table["rows"]]
        for r, c, name in ov.get("extra_cells", []):
            if rows[r][c]:
                raise GenError(f"{dev['id']}: extra cell {r},{c} is not empty")
            rows[r][c] = name
        cols = len(rows[0])
        for spec in ov.get("extra_matrix_rows", []):
            row = []
            for prefix, count in spec:
                row += [f"{prefix}{i}" for i in range(count)]
            rows.append(row + [""] * (cols - len(row)))
        notes.append(f"Matrix: OpenRGB {table_name} ({table['cite']}), applied to its base ANSI layout; see "
                     "tools/devices/keyboard-matrices.json.")
        return rows, notes
    if "matrix" in ov:
        return [list(r) for r in ov["matrix"]], notes
    if "matrix_prefix" in ov:
        dims = dev["usb"][0]["matrix"]
        cols = ov.get("matrix_cols") or (dims["openrgb"] or dims["openrazer"])[1]
        return [[f"{ov['matrix_prefix']} {i + 1}" for i in range(cols)]], notes
    raise GenError(f"{dev['id']}: lit device with no matrix source")


def check_dims(dev: dict, matrix: list[list[str]], ov: dict) -> None:
    rows, cols = len(matrix), len(matrix[0])
    for u in dev["usb"]:
        m = u.get("matrix") or {}
        cands = [tuple(x) for x in (m.get("openrgb"), m.get("openrazer")) if x]
        if cands and (rows, cols) not in cands:
            # V4 / V4 Pro: OpenRGB's KEYBOARD zone is 6 rows of an 8-row device matrix; the rest are extra rows.
            if not ov.get("extra_matrix_rows"):
                raise GenError(f"{dev['id']}: matrix {rows}x{cols} matches neither source {cands}")
        if len(set(cands)) > 1 and "matrix_why" not in ov:
            raise GenError(f"{dev['id']}: sources disagree on the matrix {cands}; overrides.json needs matrix_why")


def poll_section(dev: dict, ov: dict) -> tuple[dict | None, list[str]]:
    if "poll_rate" in ov:
        p = dict(ov["poll_rate"])
        return p, [p.pop("why")]
    src = (dev.get("mouse") or dev.get("keyboard") or {}).get("poll_rates_by_pid") or {}
    kinds = {}
    for pid, p in src.items():
        if not p.get("rates"):
            continue
        cmd = p.get("command") or ""
        kind = "hyperpolling" if "00/40" in cmd else "classic" if "00/05" in cmd else None
        if kind is None:
            raise GenError(f"{dev['id']}: unknown poll command {cmd}")
        kinds[pid] = (kind, tuple(p["rates"]), p.get("source", ""))
    if not kinds:
        return None, []
    if len(set((k, r) for k, r, _ in kinds.values())) > 1:
        raise GenError(f"{dev['id']}: PIDs disagree on polling {kinds}; overrides.json needs poll_rate")
    kind, rates, source = next(iter(kinds.values()))
    allowed = HYPERPOLLING_CODES if kind == "hyperpolling" else CLASSIC_CODES
    if not set(rates) <= allowed:
        raise GenError(f"{dev['id']}: rates {rates} not codable as {kind}")
    notes = [f"Rates and command from OpenRazer ({source or 'daemon POLL_RATES, driver poll command'})."]
    set_twice = bool(ov.get("poll_set_twice", {}).get("value", False))
    if set_twice:
        notes.append(ov["poll_set_twice"]["why"])
    return {"kind": kind, "rates": list(rates), "set_twice": set_twice}, notes


def generate(dev: dict, ov: dict, tables: dict, cfg: dict, keyids: dict) -> str:
    did, kind = dev["id"], dev["kind"]
    key_names = {k: v for k, v in cfg["key_names"].items() if not k.startswith("_")}
    L: list[str] = []
    emit = L.append

    matrix, matrix_notes = matrix_for(dev, ov, tables, key_names)
    lit_device = matrix is not None
    lighting_feature = False
    if lit_device:
        pl = dev["lighting"].get("per_led_custom_frame") or {}
        both = [v for v in (pl.get("openrazer"), pl.get("openrgb")) if v is not None]
        lighting_feature = ov.get("lighting", all(both) and bool(both))
        if not ov.get("lighting", True) is False and not all(both):
            raise GenError(f"{did}: sources disagree on per-LED frames; overrides.json needs lighting + lighting_why")
        check_dims(dev, matrix, ov)

    is_mouse = kind == "mouse"
    is_kb = kind == "keyboard"
    feats_or = dev.get("features_openrazer") or {}
    mouse = dev.get("mouse") or {}

    poll, poll_notes = poll_section(dev, ov) if (is_mouse or is_kb) else (None, [])
    battery = bool(mouse.get("battery") or feats_or.get("battery") or feats_or.get("charging_status"))
    idle = bool(mouse.get("idle_timer") or feats_or.get("idle_timer"))
    low = bool(mouse.get("low_battery_threshold") or feats_or.get("low_battery_threshold"))
    power = battery or idle or low

    features = []
    if lighting_feature:
        features.append("lighting")
    if lit_device:
        features.append("hw_effects")
    if is_mouse or is_kb:
        features += ["keymap", "profiles"]
    if is_mouse and mouse.get("dpi_max"):
        features.append("dpi")
    if poll:
        features.append("poll_rate")
    if power:
        features.append("power")
    features = [f for f in FEATURE_ORDER if f in features]

    used_groups = set()
    if lit_device:
        used_groups |= {"effect"} | ({"frame"} if lighting_feature else set())
    if "keymap" in features:
        used_groups.add("keymap")
    if "profiles" in features:
        used_groups.add("profile")
    if "dpi" in features:
        used_groups.add("dpi")
    if "poll_rate" in features:
        used_groups.add("poll")
    if "power" in features:
        used_groups |= {"power", "low_battery"}
    used_groups.add("device")

    # ---- header
    emit(f"# {dev['name']} (experimental)")
    emit("#")
    L += wrap(
        "Generated by tools/devices/gen_experimental.py from tools/reference/device-research/razer-devices.json "
        f"(OpenRazer {OPENRAZER_COMMIT}, OpenRGB {OPENRGB_COMMIT}) and tools/devices/overrides.json. Do not edit by "
        "hand: change the generator's inputs and re-run it. Nobody has confirmed this file on real hardware yet; a "
        "report that it works moves it to devices/ (see CONTRIBUTING.md)."
    )
    if dev.get("conflicts") or dev.get("notes"):
        emit("#")
        emit("# Research notes:")
        for c in dev.get("conflicts", []):
            L += wrap("- conflict: " + c, prefix="#   ")
        for n in dev.get("notes", []):
            L += wrap("- " + n, prefix="#   ")
    emit("")
    emit(f"id = {q(did)}")
    emit(f"name = {q(dev['name'])}")
    emit(f"kind = {q(kind)}")
    emit("vendor_id = 0x1532")
    emit('support = "experimental"')
    if not lit_device:
        emit("# No lighting: this device has no RGB (OpenRGB does not list it).")
    elif not lighting_feature:
        L += wrap(ov["lighting_why"])
    emit("features = [" + ", ".join(q(f) for f in features) + "]")
    emit("")

    # ---- sources
    classes, symbols, rgb_structs, detects = [], [], [], []
    drv_file = None
    for u in dev["usb"]:
        s = u["sources"]
        d = short_daemon((s.get("openrazer_daemon") or {}).get("cite"))
        if d and d not in classes:
            classes.append(d)
        drv = s.get("openrazer_driver") or {}
        if drv.get("symbol") and drv["symbol"] not in symbols:
            symbols.append(drv["symbol"])
            drv_file = drv.get("file")
        r = short_rgb((s.get("openrgb") or {}).get("cite"))
        if r and r not in rgb_structs:
            rgb_structs.append(r)
        for det in (s.get("openrgb") or {}).get("detect") or []:
            dd = short_detect(det)
            if dd not in detects:
                detects.append(dd)
    emit("[sources]")
    if classes:
        emit(f"openrazer = {q('; '.join(classes) + f'; {drv_file} ' + ', '.join(symbols) + f' ({OPENRAZER_COMMIT})')}")
    else:
        emit('openrazer = "not listed"')
    if rgb_structs:
        parts = rgb_structs + detects
        table = (dev.get("keyboard") or {}).get("openrgb_layout_table")
        if table:
            parts.append(f"{table} (RazerDevices.cpp)")
        emit(f"openrgb = {q('; '.join(parts) + f' ({OPENRGB_COMMIT})')}")
    else:
        emit(f"openrgb = {q('not listed (no RGB)' if not lit_device else 'not listed')}")
    emit("")

    # ---- usb
    skipped_bt = []
    for u in dev["usb"]:
        pid = h(u["product_id"])
        if u["connection"] == "bluetooth":
            skipped_bt.append(u)
            continue
        s = u["sources"]
        drv = s.get("openrazer_driver") or {}
        rgb = s.get("openrgb") or {}
        daemon = s.get("openrazer_daemon") or {}
        pov = ov.get("usb", {}).get(u["product_id"], {})
        default, groups, txn_notes = resolve_txn(u, ov, lit_device, used_groups)
        who = []
        if daemon.get("class"):
            who.append(f"OpenRazer {daemon['class']} ({drv.get('symbol')})")
        if rgb.get("name"):
            who.append(f"OpenRGB \"{rgb['name']}\"")
        L += wrap(f"{u['connection']}: " + "; ".join(who) + ".")
        emit("[[usb]]")
        emit(f"product_id = {hx(pid, 4)}")
        emit(f"connection = {q(u['connection'])}")
        iface_note = {"agree": "both sources agree", "conflict": "sources disagree, see below"}.get(
            u.get("interface_confidence"), u.get("interface_confidence") or "")
        emit(f"interface = {u['interface']}" + (f"   # {iface_note}" if iface_note else ""))
        up, us = h(u.get("usage_page")), h(u.get("usage"))
        if up is None:
            up, us = cfg["assumed_usage"]["value"]
            L += wrap(cfg["assumed_usage"]["why"])
        emit(f"usage_page = {hx(up)}")
        emit(f"usage = {hx(us)}")
        alts = []
        for det in rgb.get("detect") or []:
            pair = (h(det["usage_page"]), h(det["usage"]))
            if None not in pair and pair != (up, us) and pair not in alts:
                alts.append(pair)
        for pair in pov.get("alt_usages", []):
            if tuple(pair) not in alts:
                alts.append(tuple(pair))
        if alts:
            if pov.get("alt_usages"):
                L += wrap(pov["why"])
            else:
                L += wrap("OpenRGB registers several HID collections on this interface; the first is the newer firmware's.")
            emit("alt_usages = [" + ", ".join(f"[{hx(a)}, {hx(b)}]" for a, b in alts) + "]")
        for line in txn_notes:
            L += wrap(line)
        emit(f"transaction_id = {hx(default)}")
        wait = (drv.get("transport") or {}).get("wait")
        waits = {k: v for k, v in cfg["reply_wait_us"].items() if not k.startswith("_")}
        if u["connection"] == "dongle" and wait in waits:
            emit(f"reply_wait_us = {waits[wait]}   # OpenRazer {wait}")
        if groups:
            emit("")
            emit("[usb.transaction_ids]")
            for g in GROUPS:
                if g in groups:
                    emit(f"{g} = {hx(groups[g])}")
        emit("")
    for u in skipped_bt:
        L += wrap(
            f"Bluetooth {u['product_id']} is not listed: OpenRGB's detector for it is commented out and OpenRazer "
            "has none, so neither source drives it."
        )
        emit("")
    for pid, extra in (ov.get("pid_comments") or {}).items():
        L += wrap(f"{pid}: {extra}")

    # ---- quirks
    quirks = ov.get("quirks", {})
    emit("[quirks]")
    if "why" in quirks:
        L += wrap(quirks["why"])
    emit(f"ack_every_report = {'true' if quirks.get('ack_every_report') else 'false'}")
    emit("custom_mode_once = true")
    emit("")

    # ---- matrix + layout
    if lit_device:
        emit("[matrix]")
        for line in matrix_notes:
            L += wrap(line)
        if "matrix_why" in ov:
            L += wrap(ov["matrix_why"])
        emit(f"rows = {len(matrix)}")
        emit(f"cols = {len(matrix[0])}")
        emit('# Name of the LED at each (row, col); "" = no LED.')
        emit("names = [")
        for row in matrix:
            emit("  [" + ", ".join(q(n) for n in row) + "],")
        emit("]")
        emit("")
        emit("[layout]")
        if is_kb and "layout" in ov:
            lay = build_keyboard_layout(did, matrix, ov["layout"])
            emit('type = "keyboard"')
            emit(f"width = {num(lay['width'])}")
            emit(f"depth = {num(lay['depth'])}")
            emit(f"# Physical rows ({ov['layout']['form']} ANSI), left to right from x = 0, in key units (1u = 19.05 mm).")
            if "layout_why" in ov:
                L += wrap(ov["layout_why"])
            for y, specs in lay["rows"]:
                emit("")
                emit("[[layout.rows]]")
                emit(f"y = {num(y)}")
                emit("keys = [" + ", ".join(q(s) for s in specs) + "]")
            if lay["underglow"]:
                ug = lay["underglow"]
                emit("")
                emit("# Underglow strips just outside the case, drawn back to front (real direction not known).")
                emit("[layout.underglow]")
                emit(f'left = {{ prefix = "LU", x = -0.55, count = {ug["left"]} }}')
                emit(f'right = {{ prefix = "RU", x = {num(ug["right_x"])}, count = {ug["right"]} }}')
                emit("y_start = 0.2")
                emit("y_end = 6.05")
        else:
            pts = ov.get("points")
            if not pts:
                raise GenError(f"{did}: no points layout in overrides.json")
            n = len(matrix) * len(matrix[0])
            if len(pts["points"]) != n:
                raise GenError(f"{did}: {len(pts['points'])} points for {n} LEDs")
            emit('type = "points"')
            emit(f"width = {num(pts['width'])}")
            emit(f"depth = {num(pts['depth'])}")
            emit("# (x, y) of each LED relative to the device centre, in key units, matrix order. Approximate.")
            if "layout_why" in ov:
                L += wrap(ov["layout_why"])
            emit("points = [" + ", ".join(f"[{num(x)}, {num(y)}]" for x, y in pts["points"]) + "]")
        emit("")

        # ---- hw effects
        if is_kb:
            led, led_note = 0x05, "Backlight (LED id 5): both sources send keyboard effects there."
        elif "hw_effects_led" in ov:
            led, led_note = h(ov["hw_effects_led"]["led"]), ov["hw_effects_led"]["why"]
        else:
            ids = dev["lighting"].get("effect_led_id") or {}
            rgb_id = h(re.match(r"0x[0-9A-Fa-f]+", ids.get("openrgb", "") or "0x00").group(0))
            or_id = re.match(r"0x[0-9A-Fa-f]+", ids.get("openrazer", "") or "")
            if or_id and h(or_id.group(0)) != rgb_id:
                raise GenError(f"{did}: effect LED ids disagree; overrides.json needs hw_effects_led")
            led, led_note = rgb_id, f"LED id {hx(rgb_id)} (whole device): both sources."
        effects, eff_notes = hw_effect_list(dev, led, kind)
        emit("[hw_effects]")
        L += wrap(led_note)
        L += wrap("Effects: OpenRazer's list for this device, mapped to uncoil's names. " + " ".join(eff_notes))
        emit(f"led = {hx(led)}")
        emit("effects = [" + ", ".join(q(e) for e in effects) + "]")
        emit("")

    # ---- dpi
    if "dpi" in features:
        emit("[dpi]")
        emit(f"min = {cfg['dpi_min']['value']}   # {cfg['dpi_min']['why']}")
        emit(f"max = {mouse['dpi_max']}   # OpenRazer daemon DPI_MAX")
        storage = ov.get("dpi_storage", {}).get("value", "nostore")
        if storage != "nostore":
            L += wrap(ov["dpi_storage"]["why"])
        else:
            emit("# 04/05 storage byte NOSTORE, as OpenRazer sends it for almost every current mouse.")
        emit(f'storage = "{storage}"')
        stages = mouse.get("dpi_stages_max") or 0 if mouse.get("dpi_stages") else 0
        emit(f"stages_max = {stages}" + ("   # OpenRazer driver limit (RAZER_MOUSE_MAX_DPI_STAGES)" if stages else ""))
        emit("")

    # ---- poll rate
    if poll:
        emit("[poll_rate]")
        for line in poll_notes:
            L += wrap(line)
        emit(f'kind = "{poll["kind"]}"')
        emit("rates = [" + ", ".join(str(r) for r in poll["rates"]) + "]")
        emit(f"set_twice = {'true' if poll['set_twice'] else 'false'}")
        emit("")

    # ---- power
    if power:
        emit("[power]")
        emit("# From OpenRazer's feature attributes for this device (charge level/status, idle time, low threshold).")
        emit(f"battery = {'true' if battery else 'false'}")
        emit(f"idle = {'true' if idle else 'false'}")
        emit(f"low_battery = {'true' if low else 'false'}")
        emit("")

    # ---- keymap
    if is_kb:
        km = {k: v for k, v in cfg["keymap_keys"].items() if not k.startswith("_")}
        emit("[keymap]")
        L += wrap(cfg["keymap_keys"]["_why"])
        emit("get = 0x8D")
        emit("set = 0x0D")
        emit('layers = ["normal", "hypershift"]')
        emit("keys = [")
        if matrix and "layout" in ov:
            order = [n for _, specs in build_keyboard_layout(did, matrix, ov["layout"])["rows"]
                     for n in (s.rsplit(":", 1)[0] for s in specs)]
            with_led = True
        else:
            order = [n for _, keys in template(ov.get("keymap_layout", "full"))[0] for n, _, _ in keys]
            with_led = False
            emit("  # No per-key LEDs on this board, so keys carry no led name.")
        seen = set()
        for n in order:
            if n not in km or n in seen:
                continue
            seen.add(n)
            kid, default = km[n]
            sname = keyids.get(str(kid))
            if not sname:
                raise GenError(f"key id {kid} ({n}) is not in docs/blackwidow-keyids.json")
            sname = sname.removeprefix("KEY_")
            led = f", led = {q(n)}" if with_led else ""
            emit(f"  {{ id = {kid}, name = {q(sname)}{led}, default = {q(default)} }},")
        emit("]")
        emit("")
    elif is_mouse:
        mb = cfg["mouse_buttons"]
        emit("[keymap]")
        L += wrap(mb["_why"])
        emit("get = 0x8C")
        emit("set = 0x0C")
        emit('layers = ["normal", "hypershift"]')
        emit("keys = [")
        buttons = mb["common"] + (mb["tilt"] if ov.get("tilt") else [])
        for bid, bname, default in buttons:
            emit(f"  {{ id = {bid}, name = {q(bname)}, default = {q(default)} }},")
        emit("]")
        if ov.get("tilt"):
            emit("# 52/53: tilt wheel left/right, as on the Basilisk V3 Pro.")
        emit("")

    while L and L[-1] == "":
        L.pop()
    return "\n".join(L) + "\n"


def main(argv: list[str]) -> int:
    check = "--check" in argv
    research = json.loads(RESEARCH.read_text(encoding="utf-8"))
    tables = json.loads(MATRICES.read_text(encoding="utf-8"))["tables"]
    cfg = json.loads(OVERRIDES.read_text(encoding="utf-8"))
    keyids = json.loads(KEYIDS.read_text(encoding="utf-8"))
    OUT.mkdir(parents=True, exist_ok=True)

    written, skipped, changed = [], [], []
    expected = set()
    for dev in research["devices"]:
        did = dev["id"]
        if did in cfg["skip"]:
            skipped.append((did, cfg["skip"][did]))
            continue
        ov = cfg["devices"].get(did)
        if ov is None:
            raise GenError(f"{did}: no entry in overrides.json devices (add {{}} if nothing needs overriding)")
        text = generate(dev, ov, tables, cfg, keyids)
        path = OUT / f"{did}.toml"
        expected.add(path.name)
        old = path.read_text(encoding="utf-8") if path.exists() else None
        if old != text:
            changed.append(path.name)
            if not check:
                path.write_text(text, encoding="utf-8", newline="\n")
        written.append((did, dev["kind"]))
    stray = sorted(p.name for p in OUT.glob("*.toml") if p.name not in expected)

    kinds: dict[str, int] = {}
    for _, k in written:
        kinds[k] = kinds.get(k, 0) + 1
    print(f"{len(written)} device files ({', '.join(f'{v} {k}' for k, v in sorted(kinds.items()))})"
          + (f", {len(changed)} {'would change' if check else 'changed'}" if changed else ", none changed"))
    for did, why in skipped:
        print(f"skipped {did}: {why}")
    if stray:
        print(f"not generated by this script (remove or promote): {', '.join(stray)}")
    if check and changed:
        print("out of date: " + ", ".join(changed))
        return 1
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv[1:]))
    except GenError as e:
        print(f"error: {e}", file=sys.stderr)
        sys.exit(2)
