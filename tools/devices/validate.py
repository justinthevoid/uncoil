#!/usr/bin/env python3
"""Check device files against the experimental-device contract (and each other).

  python tools/devices/validate.py          check devices/experimental/*.toml (+ cross-checks with devices/*.toml)

Checks: required sections per feature, matrix names vs layout names, unique ids / LED names / key ids,
USB product ids unique across every device file, transaction ids in the known set, key ids in
docs/blackwidow-keyids.json, and that gen_experimental.py would not change any file. Needs Python 3.11+
(tomllib) or the `tomli` package.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

try:
    import tomllib  # type: ignore[import-not-found]
except ModuleNotFoundError:  # Python 3.10
    try:
        import tomli as tomllib  # type: ignore[no-redef]
    except ModuleNotFoundError:
        sys.exit("validate.py needs Python 3.11+ or `pip install tomli`")

ROOT = Path(__file__).resolve().parents[2]
SUPPORTED = ROOT / "devices"
EXPERIMENTAL = ROOT / "devices/experimental"
KEYIDS = ROOT / "docs/blackwidow-keyids.json"

KINDS = {"keyboard", "mouse", "mousemat", "headset", "other"}
FEATURES = {"lighting", "hw_effects", "keymap", "profiles", "dial", "oled", "dpi", "poll_rate", "power"}
CONNECTIONS = {"wired", "dongle", "bluetooth"}
TXN = {0x1F, 0x3F, 0x9F, 0xFF}
GROUPS = {"frame", "effect", "keymap", "profile", "dpi", "poll", "power", "low_battery", "device"}
EFFECTS = {"off", "static", "breathing", "spectrum", "wave", "wheel", "reactive", "starlight"}
CLASSIC = {125, 500, 1000}
HYPERPOLLING = {125, 250, 500, 1000, 2000, 4000, 8000}
MOUSE_BUTTONS = {1, 2, 3, 4, 5, 9, 10, 52, 53}
KEYBOARD_CHECK_KEYS = {26, 31, 110}  # contract section 3: P, A, Esc
MOUSE_CHECK_BUTTONS = {1, 2}


def parse_key(spec: str) -> tuple[str, float]:
    name, _, w = spec.rpartition(":")
    return (name, float(w)) if name else (spec, 1.0)


def check(path: Path, d: dict, experimental: bool, keyids: dict, err) -> None:
    def e(msg: str) -> None:
        err(f"{path.relative_to(ROOT).as_posix()}: {msg}")

    for k in ("id", "name", "kind", "vendor_id", "usb"):
        if k not in d:
            e(f"missing `{k}`")
            return
    if d["id"] != path.stem:
        e(f"id {d['id']!r} does not match the file name")
    if d["kind"] not in KINDS:
        e(f"unknown kind {d['kind']!r}")
    if d["vendor_id"] != 0x1532:
        e("vendor_id is not 0x1532")
    support = d.get("support", "supported")
    if experimental and support != "experimental":
        e('files in devices/experimental/ need support = "experimental"')
    if not experimental and support != "supported":
        e("files in devices/ are supported; drop the support line or move the file to devices/experimental/")
    feats = d.get("features", ["lighting"])
    unknown = set(feats) - FEATURES
    if unknown:
        e(f"unknown features {sorted(unknown)}")
    if len(set(feats)) != len(feats):
        e("a feature is listed twice")
    if experimental and set(feats) & {"dial", "oled"}:
        e("dial / oled are not part of the experimental contract")

    # ---- usb
    if not d["usb"]:
        e("no [[usb]] entries")
    for u in d["usb"]:
        tag = f"usb {u.get('product_id', '?'):#06x}" if isinstance(u.get("product_id"), int) else "usb ?"
        for k in ("product_id", "interface", "usage_page", "usage", "transaction_id"):
            if k not in u:
                e(f"{tag}: missing `{k}`")
        conn = u.get("connection", "")
        if conn not in CONNECTIONS:
            e(f"{tag}: connection {conn!r} is not wired / dongle / bluetooth")
        if experimental and conn == "bluetooth":
            e(f"{tag}: Bluetooth is unsupported in both sources; list it in a comment instead")
        if u.get("transaction_id") not in TXN:
            e(f"{tag}: transaction_id {u.get('transaction_id')} not in the known set")
        for g, v in (u.get("transaction_ids") or {}).items():
            if g not in GROUPS:
                e(f"{tag}: unknown transaction id group {g!r}")
            if v not in TXN:
                e(f"{tag}: transaction_ids.{g} = {v} not in the known set")
            if v == u.get("transaction_id"):
                e(f"{tag}: transaction_ids.{g} repeats transaction_id")
        for pair in u.get("alt_usages", []):
            if not (isinstance(pair, list) and len(pair) == 2 and all(isinstance(x, int) for x in pair)):
                e(f"{tag}: alt_usages entries are [page, usage] pairs")
            elif tuple(pair) == (u.get("usage_page"), u.get("usage")):
                e(f"{tag}: alt_usages repeats the main usage")
        if "reply_wait_us" in u:
            if not (isinstance(u["reply_wait_us"], int) and 0 < u["reply_wait_us"] <= 100_000):
                e(f"{tag}: reply_wait_us out of range")
            if conn != "dongle":
                e(f"{tag}: reply_wait_us is meant for wireless receivers")

    # ---- feature sections
    lit = "lighting" in feats or "hw_effects" in feats
    if lit:
        for k in ("matrix", "layout"):
            if k not in d:
                e(f"lighting / hw_effects need [{k}]")
    elif experimental and ("matrix" in d or "layout" in d):
        e("[matrix] / [layout] without lighting or hw_effects")
    need = {"hw_effects": "hw_effects", "dpi": "dpi", "poll_rate": "poll_rate", "power": "power", "keymap": "keymap"}
    for f, sec in need.items():
        if f in feats and sec not in d:
            e(f"feature {f} needs [{sec}]")
        if experimental and f not in feats and sec in d:
            e(f"[{sec}] present but {f} is not in features")

    if "hw_effects" in d:
        hw = d["hw_effects"]
        if not isinstance(hw.get("led", 0), int) or not 0 <= hw.get("led", 0) <= 0xFF:
            e("hw_effects.led is not a byte")
        bad = set(hw.get("effects", [])) - EFFECTS
        if bad:
            e(f"unknown hw effects {sorted(bad)}")
        if not hw.get("effects"):
            e("hw_effects.effects is empty")
    if "dpi" in d:
        p = d["dpi"]
        if not (isinstance(p.get("min"), int) and isinstance(p.get("max"), int) and 0 < p["min"] < p["max"]):
            e("dpi.min / dpi.max invalid")
        if p.get("storage", "nostore") not in ("nostore", "varstore"):
            e("dpi.storage is not nostore / varstore")
        if not 0 <= p.get("stages_max", 0) <= 5:
            e("dpi.stages_max outside 0..5")
    if "poll_rate" in d:
        p = d["poll_rate"]
        codes = {"classic": CLASSIC, "hyperpolling": HYPERPOLLING}.get(p.get("kind"))
        if codes is None:
            e("poll_rate.kind is not classic / hyperpolling")
        elif not p.get("rates") or not set(p["rates"]) <= codes:
            e(f"poll_rate.rates {p.get('rates')} not all codable as {p.get('kind')}")
        if p.get("set_twice") and p.get("kind") != "hyperpolling":
            e("poll_rate.set_twice only applies to hyperpolling")
    if "power" in d:
        p = d["power"]
        if not any(p.get(k) for k in ("battery", "idle", "low_battery")):
            e("[power] with nothing in it")

    # ---- matrix / layout
    names: list[str] = []
    if "matrix" in d:
        m = d["matrix"]
        if len(m.get("names", [])) != m.get("rows"):
            e("matrix.names row count differs from rows")
        for r, row in enumerate(m.get("names", [])):
            if len(row) != m.get("cols"):
                e(f"matrix row {r} has {len(row)} cols, expected {m.get('cols')}")
        names = [n for row in m.get("names", []) for n in row if n]
        dupes = {n for n in names if names.count(n) > 1}
        if dupes:
            e(f"LED names used twice: {sorted(dupes)}")
    if "layout" in d and "matrix" in d:
        lay = d["layout"]
        if lay.get("type") == "points":
            n = d["matrix"]["rows"] * d["matrix"]["cols"]
            if len(lay.get("points", [])) != n:
                e(f"{len(lay.get('points', []))} layout points for {n} matrix slots")
        elif lay.get("type") == "keyboard":
            placed = set()
            for row in lay.get("rows", []):
                x = 0.0
                for spec in row.get("keys", []):
                    name, w = parse_key(spec)
                    if w <= 0:
                        e(f"layout key {spec!r} has no width")
                    if name != "gap":
                        if name in placed:
                            e(f"layout key {name!r} placed twice")
                        placed.add(name)
                        if name not in names:
                            e(f"layout key {name!r} is not in the matrix")
                    x += w
                if x > lay.get("width", 0) + 1e-3:
                    e(f"layout row y={row.get('y')} is wider ({x}) than layout.width")
            ug = lay.get("underglow") or {}
            for side in ("left", "right"):
                s = ug.get(side)
                if s:
                    for i in range(s["count"]):
                        n = f"{s['prefix']}{i}"
                        placed.add(n)
                        if n not in names:
                            e(f"underglow LED {n} is not in the matrix")
            missing = set(names) - placed
            if missing:
                e(f"matrix LEDs with no physical position: {sorted(missing)}")
        else:
            e(f"unknown layout type {lay.get('type')!r}")

    # ---- keymap
    if "keymap" in d:
        km = d["keymap"]
        ids = [k["id"] for k in km.get("keys", [])]
        if len(set(ids)) != len(ids):
            e("keymap key id listed twice")
        for k in km.get("keys", []):
            if "default" in k and not re.match(r"^(key|button|off|dpi|profile|shortcut|media|razer|raw) ?", k["default"]):
                e(f"key {k['name']}: odd default {k['default']!r}")
            if k.get("led") and k["led"] not in names:
                e(f"key {k['name']}: led {k['led']!r} is not in the matrix")
        if experimental and d["kind"] == "keyboard":
            if (km.get("get"), km.get("set")) != (0x8D, 0x0D):
                e("keyboard keymap should use 02/8D get, 02/0D set")
            bad = [i for i in ids if str(i) not in keyids]
            if bad:
                e(f"key ids not in docs/blackwidow-keyids.json: {bad}")
            if not KEYBOARD_CHECK_KEYS <= set(ids):
                e("keymap lacks P / A / Esc, which the read-only check reads")
            for k in km.get("keys", []):
                want = keyids.get(str(k["id"]), "").removeprefix("KEY_")
                if want and k["name"] != want:
                    e(f"key id {k['id']} is {want} in docs/blackwidow-keyids.json, not {k['name']}")
        if experimental and d["kind"] == "mouse":
            if (km.get("get"), km.get("set")) != (0x8C, 0x0C):
                e("mouse keymap should use 02/8C get, 02/0C set")
            if not set(ids) <= MOUSE_BUTTONS:
                e(f"experimental mice list only the common buttons, got {sorted(set(ids) - MOUSE_BUTTONS)}")
            if not MOUSE_CHECK_BUTTONS <= set(ids):
                e("keymap lacks buttons 1 / 2, which the read-only check reads")


def main() -> int:
    keyids = json.loads(KEYIDS.read_text(encoding="utf-8"))
    errors: list[str] = []
    files = [(p, False) for p in sorted(SUPPORTED.glob("*.toml"))]
    files += [(p, True) for p in sorted(EXPERIMENTAL.glob("*.toml"))]
    ids: dict[str, Path] = {}
    pids: dict[int, Path] = {}
    for path, experimental in files:
        try:
            d = tomllib.loads(path.read_text(encoding="utf-8"))
        except Exception as ex:  # noqa: BLE001
            errors.append(f"{path.relative_to(ROOT).as_posix()}: TOML error: {ex}")
            continue
        check(path, d, experimental, keyids, errors.append)
        if d.get("id") in ids:
            errors.append(f"id {d['id']} in both {ids[d['id']].name} and {path.name}")
        ids[d.get("id")] = path
        for u in d.get("usb", []):
            pid = u.get("product_id")
            if pid in pids:
                errors.append(f"product id {pid:#06x} in both {pids[pid].name} and {path.name}")
            pids[pid] = path

    gen = subprocess.run([sys.executable, str(Path(__file__).with_name("gen_experimental.py")), "--check"],
                         capture_output=True, text=True)
    if gen.returncode != 0:
        errors.append("gen_experimental.py --check: " + (gen.stdout + gen.stderr).strip().splitlines()[-1])

    n_exp = sum(1 for _, x in files if x)
    if errors:
        print("\n".join(errors))
        print(f"{len(errors)} problem(s) in {len(files)} files")
        return 1
    print(f"ok: {len(files)} device files ({n_exp} experimental), {len(pids)} product ids, all unique")
    return 0


if __name__ == "__main__":
    sys.exit(main())
