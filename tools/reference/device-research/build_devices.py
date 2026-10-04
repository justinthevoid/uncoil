"""Assemble razer-devices.json from parsed OpenRazer + OpenRGB facts (facts only, no code copied)."""
import json, re, os, collections, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import parse_or as O, parse_rgb as R, parse_drv as V

OR_SHA = "a84cd0aefe37079a92637dc8469b5fd5bcb56d5f"
RGB_SHA = "df3024befb0aff7326a3cb0662c75092dab96dc1"

# (id, name, kind, [(pid, connection, openrazer_class)])
DEVICES = [
 ("razer-blackwidow-v3", "Razer BlackWidow V3", "keyboard", [(0x024E, "wired", "RazerBlackWidowV3")]),
 ("razer-blackwidow-v3-pro", "Razer BlackWidow V3 Pro", "keyboard", [(0x025A, "wired", "RazerBlackWidowV3ProWired"), (0x025C, "dongle", "RazerBlackWidowV3ProWireless"), (0x025B, "bluetooth", None)]),
 ("razer-blackwidow-v3-tkl", "Razer BlackWidow V3 Tenkeyless", "keyboard", [(0x0A24, "wired", "RazerBlackWidowV3TK")]),
 ("razer-blackwidow-v4", "Razer BlackWidow V4", "keyboard", [(0x0287, "wired", "RazerBlackWidowV4")]),
 ("razer-blackwidow-v4-pro", "Razer BlackWidow V4 Pro", "keyboard", [(0x028D, "wired", "RazerBlackWidowV4Pro")]),
 ("razer-blackwidow-v4-x", "Razer BlackWidow V4 X", "keyboard", [(0x0293, "wired", "RazerBlackWidowV4X")]),
 ("razer-blackwidow-v4-75", "Razer BlackWidow V4 75%", "keyboard", [(0x02A5, "wired", "RazerBlackWidowV4_75PCT")]),
 ("razer-huntsman-v2", "Razer Huntsman V2", "keyboard", [(0x026C, "wired", "RazerHuntsmanV2")]),
 ("razer-huntsman-v2-tkl", "Razer Huntsman V2 Tenkeyless", "keyboard", [(0x026B, "wired", "RazerHuntsmanV2Tenkeyless")]),
 ("razer-huntsman-v3-pro", "Razer Huntsman V3 Pro", "keyboard", [(0x02A6, "wired", "RazerHuntsmanV3Pro")]),
 ("razer-huntsman-v3-pro-tkl", "Razer Huntsman V3 Pro Tenkeyless", "keyboard", [(0x02A7, "wired", "RazerHuntsmanV3ProTKL")]),
 ("razer-huntsman-mini", "Razer Huntsman Mini", "keyboard", [(0x0257, "wired", "RazerHuntsmanMini")]),
 ("razer-ornata-v3", "Razer Ornata V3", "keyboard", [(0x028F, "wired", "RazerOrnataV3_Alternate"), (0x02A1, "wired", "RazerOrnataV3")]),
 ("razer-deathadder-v3", "Razer DeathAdder V3", "mouse", [(0x00B2, "wired", "RazerDeathAdderV3")]),
 ("razer-deathadder-v3-pro", "Razer DeathAdder V3 Pro", "mouse", [(0x00B6, "wired", "RazerDeathAdderV3ProWired"), (0x00B7, "dongle", "RazerDeathAdderV3ProWireless"), (0x00C2, "wired", "RazerDeathAdderV3ProWired_Alternate"), (0x00C3, "dongle", "RazerDeathAdderV3ProWireless_Alternate")]),
 ("razer-deathadder-v2", "Razer DeathAdder V2", "mouse", [(0x0084, "wired", "RazerDeathAdderV2")]),
 ("razer-viper-v2-pro", "Razer Viper V2 Pro", "mouse", [(0x00A5, "wired", "RazerViperV2ProWired"), (0x00A6, "dongle", "RazerViperV2ProWireless")]),
 ("razer-viper-v3-pro", "Razer Viper V3 Pro", "mouse", [(0x00C0, "wired", "RazerViperV3ProWired"), (0x00C1, "dongle", "RazerViperV3ProWireless")]),
 ("razer-viper-mini", "Razer Viper Mini", "mouse", [(0x008A, "wired", "RazerViperMini")]),
 ("razer-basilisk-v3", "Razer Basilisk V3", "mouse", [(0x0099, "wired", "RazerBasiliskV3")]),
 ("razer-basilisk-v3-x-hyperspeed", "Razer Basilisk V3 X HyperSpeed", "mouse", [(0x00B9, "dongle", "RazerBasiliskV3XHyperSpeed")]),
 ("razer-basilisk-v3-35k", "Razer Basilisk V3 35K", "mouse", [(0x00CB, "wired", "RazerBasiliskV3_35K")]),
 ("razer-cobra", "Razer Cobra", "mouse", [(0x00A3, "wired", "RazerCobra")]),
 ("razer-cobra-pro", "Razer Cobra Pro", "mouse", [(0x00AF, "wired", "RazerCobraProWired"), (0x00B0, "dongle", "RazerCobraProWireless")]),
 ("razer-naga-v2-pro", "Razer Naga V2 Pro", "mouse", [(0x00A7, "wired", "RazerNagaV2ProWired"), (0x00A8, "dongle", "RazerNagaV2ProWireless")]),
 ("razer-firefly-v2", "Razer Firefly V2", "mousemat", [(0x0C04, "wired", "RazerFireflyV2")]),
 ("razer-strider-chroma", "Razer Strider Chroma", "mousemat", [(0x0C05, "wired", "RazerStriderChroma")]),
 ("razer-base-station-v2-chroma", "Razer Base Station V2 Chroma", "other", [(0x0F20, "wired", "RazerBaseStationV2Chroma")]),
 ("razer-mouse-dock-pro", "Razer Mouse Dock Pro", "other", [(0x00A4, "wired", "RazerMouseDockPro")]),
 ("razer-chroma-addressable-rgb-controller", "Razer Chroma Addressable RGB Controller", "other", [(0x0F1F, "wired", "RazerChromaARGB")]),
]

EFFECT_MAP = [("set_none_effect", "off"), ("set_static_effect", "static"), ("set_breath_single_effect", "breathing"),
              ("set_breath_dual_effect", "breathing_dual"), ("set_breath_random_effect", "breathing_random"),
              ("set_spectrum_effect", "spectrum"), ("set_wave_effect", "wave"), ("set_wheel_effect", "wheel"),
              ("set_reactive_effect", "reactive"), ("set_starlight_single_effect", "starlight"),
              ("set_starlight_dual_effect", "starlight_dual"), ("set_starlight_random_effect", "starlight_random"),
              ("set_ripple_effect", "ripple (daemon software effect, not firmware)"), ("set_custom_effect", "custom_frame")]
ZONE_PREFIX = {"logo": 0x04, "scroll": 0x01, "left": 0x11, "right": 0x10, "backlight": 0x05, "charging": 0x20,
               "fast_charging": 0x21, "fully_charged": 0x22}

rgb_by_pid = {}
for d in R.devs:
    if isinstance(d["pid"], str) and d["pid"].startswith("0x"):
        rgb_by_pid.setdefault(int(d["pid"], 16), []).append(d)

layout_size = {}
src = R.cpp
for m in re.finditer(r"keyboard_keymap_overlay_values\s+(\w+)\s*\{\s*KEYBOARD_SIZE::(\w+)", src):
    layout_size[m.group(1)] = (m.group(2), src.count("\n", 0, m.start()) + 1)

# OpenRGB LED-id switch membership (RazerController.cpp constructor, LED id section)
ctrl = open(os.path.join(R.D, "rgb_RazerController.cpp"), encoding="utf-8").read().split("\n")
def case_block(lo, hi):
    return {re.sub(r"_PID$", "", m) for l in ctrl[lo - 1:hi] for m in re.findall(r"case RAZER_(\w+):", l)}
LED_ZERO = case_block(81, 139)
WAVE_FALLBACK = case_block(529, 692)
NO_HW_BREATH = case_block(466, 485)

pidsym_rgb = {v: k for k, v in R.pids.items()}

def drv_sym(pid):
    for s, v in V.defs.items():
        if v == pid and s in V.ALL:
            return s
    return None

def summarize_drv(sym):
    if not sym:
        return None
    s = V.summary(sym)
    cnt = collections.Counter(t for f in s.values() for t in f["txn"])
    major = cnt.most_common(1)[0][0] if cnt else None
    exceptions = {fn.split(":")[1]: f["txn"] for fn, f in s.items() if f["txn"] and f["txn"] != [major]}
    calls = sorted({c for f in s.values() for c in f["calls"]})
    LIT = re.compile(r"matrix|led_brightness|logo|scroll_led|backlight|custom_frame|_effect")
    lc = collections.Counter(t for fn, f in s.items() if LIT.search(fn) and "macro_led" not in fn and "game_led" not in fn for t in f["txn"])
    oc = collections.Counter(t for fn, f in s.items() if not (LIT.search(fn) and "macro_led" not in fn and "game_led" not in fn) for t in f["txn"])
    txn_lighting = lc.most_common(1)[0][0] if lc else None
    txn_other = oc.most_common(1)[0][0] if oc else None
    file = next(iter(s)).split(":")[0] if s else None
    poll = None
    if "razer_chroma_misc_set_polling_rate2" in calls or "razer_chroma_misc_get_polling_rate2" in calls:
        poll = "00/40 set, 00/C0 get (polling_rate2, HyperPolling bitmask)"
    elif "razer_chroma_misc_set_polling_rate" in calls or "razer_chroma_misc_get_polling_rate" in calls:
        poll = "00/05 set, 00/85 get (classic: 0x01=1000, 0x02=500, 0x08=125)"
    funcs = sorted(fn.split(":")[1].replace("razer_attr_", "") for fn in s)
    return {"symbol": sym, "file": file, "txn_majority": major, "txn_lighting": txn_lighting, "txn_other": txn_other, "txn_counts": dict(cnt), "txn_exceptions": exceptions,
            "poll_command": poll, "attrs": funcs,
            "matrix_cmds": "extended (0F)" if any("extended_matrix" in c for c in calls) else ("standard (03)" if any("standard_matrix" in c for c in calls) else None)}

def _groups(fname, lo, hi):
    """case-group -> (lines of the action) inside a line range of a driver file"""
    lines = open(os.path.join(V.D, fname), encoding="utf-8").read().split("\n")[lo - 1:hi]
    res = {}; grp = []; fresh = True; act = []
    for i, l in enumerate(lines, lo):
        cs = re.findall(r"case\s+(USB_DEVICE_ID_RAZER_\w+)\s*:", l)
        if "default:" in l: cs = ["default"]
        if cs:
            if fresh: grp = []; act = []
            grp += cs; fresh = False; continue
        if l.strip():
            act.append((i, l.strip()))
            for g in grp: res[g] = list(act)
        if re.search(r"\bbreak;|\breturn\b", l): fresh = True
    return res
KBD_IDX = _groups("or_razerkbd_driver.c", 336, 402)
MOUSE_IDX = _groups("or_razermouse_driver.c", 33, 112)

def or_transport(sym, file):
    if not sym: return None
    if file == "razerkbd_driver.c":
        a = KBD_IDX.get(sym) or KBD_IDX.get("default")
        idx = next((re.search(r"0x0\d", t).group(0) for _, t in a if "report_index" in t), None)
        wait = next((re.search(r"RAZER_\w+_US", t).group(0) for _, t in a if "wait" in t), None)
        return {"report_index_interface": idx, "wait": wait, "cite": "OpenRazer driver/razerkbd_driver.c:%d (razer_get_report_params)" % a[0][0]}
    if file == "razermouse_driver.c":
        a = MOUSE_IDX.get(sym) or MOUSE_IDX.get("default")
        idx = "0x03" if any("index = 0x03" in t for _, t in a) else "0x00"
        wait = next((re.search(r"RAZER_\w+_US", t).group(0) for _, t in a if "_US" in t), None)
        return {"report_index_interface": idx, "wait": wait, "cite": "OpenRazer driver/razermouse_driver.c:%d (razer_get_report)" % a[0][0]}
    if file == "razeraccessory_driver.c":
        return {"report_index_interface": "0x00", "wait": None, "cite": "OpenRazer driver/razeraccessory_driver.c:38-42 (razer_get_report)"}

out = []
for did, name, kind, ents in DEVICES:
    usb = []
    or_info = None
    for pid, conn, ocls in ents:
        e = {"product_id": "0x%04X" % pid, "connection": conn}
        rg = rgb_by_pid.get(pid, [])
        rgsym = pidsym_rgb.get(pid)
        det = R.detect.get(rgsym) if rgsym else None
        if rg:
            r = rg[0]
            e["openrgb"] = {"name": r["name"], "transaction_id": r["txn"], "matrix_type": r["matrix_type"].replace("RAZER_MATRIX_TYPE_", "").lower(),
                            "rows": int(r["rows"]), "cols": int(r["cols"]),
                            "zones": [{"name": z.get("name", "").replace("ZONE_EN_", ""), "type": z.get("type", "").replace("ZONE_TYPE_", "").lower(), "rows": z.get("rows"), "cols": z.get("cols")} for z in r["zones"]],
                            "layout_table": r["layout"] if r["layout"] not in (None, "NULL") else None,
                            "led_id_for_effects": ("0x00 (zero)" if re.sub(r"_PID$", "", rgsym or "").replace("RAZER_", "") in LED_ZERO else "0x05 (backlight)"),
                            "cite": "OpenRGB Controllers/RazerController/RazerDevices.cpp:%d (%s)" % (r["line"], r["var"])}
            e["openrgb"]["detect"] = [{"interface": d0["args"][0] if len(d0["args"]) > 0 else None,
                                          "usage_page": d0["args"][1] if len(d0["args"]) > 1 else None,
                                          "usage": d0["args"][2] if len(d0["args"]) > 2 else None,
                                          "vid": d0["vid"], "macro": "REGISTER_HID_DETECTOR_" + d0["macro"],
                                          "cite": "OpenRGB Controllers/RazerController/RazerControllerDetect.cpp:%d" % d0["line"]} for d0 in (det or [])] or None
        else:
            e["openrgb"] = None
        if ocls and ocls in O.classes:
            c = O.classes[ocls]; a = c["attrs"]
            meths = a.get("METHODS") or []
            polls = a.get("POLL_RATES") or ([125, 500, 1000] if "set_poll_rate" in meths else None)
            e["openrazer_daemon"] = {"class": ocls, "matrix_dims": a.get("MATRIX_DIMS"), "has_matrix": a.get("HAS_MATRIX"),
                                     "dpi_max": a.get("DPI_MAX"), "poll_rates": polls,
                                     "poll_rates_source": ("explicit POLL_RATES" if a.get("POLL_RATES") else ("daemon default (device_base.py:115)" if polls else None)),
                                     "wave_dirs": list(a.get("WAVE_DIRS")) if a.get("WAVE_DIRS") else None,
                                     "methods": meths,
                                     "cite": "OpenRazer daemon/openrazer_daemon/hardware/%s:%d (class %s); METHODS at %s" % (c["file"], c["line"], ocls, c["lines"].get("METHODS"))}
        else:
            e["openrazer_daemon"] = None
        e["openrazer_driver"] = summarize_drv(drv_sym(pid))
        if e["openrazer_driver"]:
            e["openrazer_driver"]["transport"] = or_transport(e["openrazer_driver"]["symbol"], e["openrazer_driver"]["file"])
        usb.append(e)
    out.append({"id": did, "name": name, "kind": kind, "usb": usb})

json.dump({"sources": {"openrazer_commit": OR_SHA, "openrgb_commit": RGB_SHA}, "devices": out},
          open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "raw_devices.json"), "w"), indent=1)
print("ok", len(out))
