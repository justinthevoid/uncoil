"""raw_devices.json -> razer-devices.json with consensus values, confidence and conflicts."""
import json, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import build_devices as B

H = os.path.dirname(os.path.abspath(__file__))
raw = json.load(open(os.path.join(H, "raw_devices.json")))

KB_FORM = {  # physical form factor; OpenRGB KEYBOARD_SIZE is the cited source, extras noted
 "razer-blackwidow-v3": ("full", None),
 "razer-blackwidow-v3-pro": ("full", None),
 "razer-blackwidow-v3-tkl": ("tkl", None),
 "razer-blackwidow-v4": ("full", "plus M1-M6 macro column, media keys, command dial (OpenRGB razer_blackwidow_v4_layout inserts M1-M6 and media keys); 8x23 matrix includes underglow"),
 "razer-blackwidow-v4-pro": ("full", "plus M1-M5 macro column, command dial, media keys (OpenRGB razer_blackwidow_v4_pro_layout); 8x23 matrix includes underglow"),
 "razer-blackwidow-v4-x": ("full", "plus M1-M6 macro column (OpenRGB razer_blackwidow_v4_x_layout)"),
 "razer-blackwidow-v4-75": ("75%", "OpenRGB reuses razer_blackwidow_v4_pro_75_layout (underglow packed in cols 0/17 like the V4 Pro 75%); OpenRazer says 6x16"),
 "razer-huntsman-v2": ("full", "media keys + volume dial on the real board; OpenRGB layout table only adjusts the base full layout"),
 "razer-huntsman-v2-tkl": ("tkl", None),
 "razer-huntsman-v3-pro": ("full", "analog keys, rotary dial"),
 "razer-huntsman-v3-pro-tkl": ("tkl", "OpenRGB entry is named 'Huntsman V3 Pro TKL White'"),
 "razer-huntsman-mini": ("60%", None),
 "razer-ornata-v3": ("full", "membrane, zone-lit (1x10 zones, not per-key): no per-key layout in OpenRGB (layout_table NULL)"),
}

MANUAL_CONFLICTS = {
 "razer-cobra-pro": ["Per-LED: OpenRazer has no matrix / custom frame for Cobra Pro (zone effects only: whole device 0x00, logo 0x04, scroll 0x01; mouse.py RazerCobraProWired METHODS), OpenRGB drives it as a 1x11 per-LED matrix (RazerDevices.cpp)."],
 "razer-cobra": ["Per-LED/LED id: OpenRazer has no matrix and only logo effects (LOGO_LED 0x04); OpenRGB treats it as 1x1 and sends effects to 0x05 (not in its LED-id-zero list, RazerController.cpp:81-176)."],
 "razer-viper-mini": ["Effect LED id: OpenRazer logo methods use LOGO_LED 0x04 (razermouse_driver.c logo_matrix_effect_* -> *_common(LOGO_LED)); OpenRGB sends effects to 0x05 (default branch RazerController.cpp:174-176). Probe 0F/80 to settle."],
 "razer-basilisk-v3-x-hyperspeed": ["Per-LED: OpenRazer has no matrix (scroll-wheel zone effects only, LED 0x01); OpenRGB lists a 1x1 matrix with effects on LED 0x00."],
 "razer-deathadder-v2": ["Zones: OpenRazer MATRIX_DIMS [1,1] but separate logo (0x04) and scroll (0x01) zone methods; OpenRGB uses one 1x2 matrix (logo + scroll wheel)."],
 "razer-blackwidow-v3-pro": ["Bluetooth PID 0x025B: OpenRGB has an entry in RazerDevices.cpp but its HID detector is commented out (RazerControllerDetect.cpp:225); OpenRazer has no class. Treat as unsupported."],
}
MANUAL_NOTES = {
 "razer-blackwidow-v3": ["OpenRazer writes custom-frame rows with 0x1F while firmware effects use 0x3F (razerkbd_driver.c razer_attr_write_matrix_custom_frame vs effect functions); OpenRGB uses 0x3F throughout."],
 "razer-blackwidow-v3-tkl": ["Same custom-frame 0x1F / effects 0x3F split as the BlackWidow V3 in OpenRazer."],
 "razer-blackwidow-v3-pro": ["Wired 0x025A: OpenRazer custom frame 0x1F, effects 0x3F (as BW V3).", "Wireless dongle 0x025C: OpenRazer uses 0x9F for everything lighting/battery and a 4.9 ms wait (RAZER_BLACKWIDOW_V3_WIRELESS_WAIT_US, razerkbd_driver.h:167); OpenRGB uses 0x3F and interface 3. OpenRGB's other HyperSpeed boards (V3 Mini, V4 TKL wireless) use 0x9F, which supports OpenRazer here."],
 "razer-blackwidow-v4": ["HID collection depends on firmware: page 0x01/usage 0x00 before 1.5, page 0x0C/usage 0x01 from 1.5, always interface 3 (RazerControllerDetect.cpp:230-231). Match on interface, not usage.", "8x23 matrix includes macro column, media keys and underglow; OpenRGB layout razer_blackwidow_v4_layout."],
 "razer-blackwidow-v4-pro": ["HID collection depends on firmware (as BW V4; RazerControllerDetect.cpp:232-233).", "Has command dial and side/underglow LEDs in the 8x23 matrix."],
 "razer-blackwidow-v4-75": ["OpenRGB detects on interface 3, page 0x01, usage 0x00 (RazerControllerDetect.cpp:236). uncoil's V4 Pro 75% answers on 3/0x0C/0x01, so the usage may be firmware-dependent like the V4.", "Matrix 6x18 (OpenRGB, same table as the V4 Pro 75% with underglow in cols 0 and 17) vs 6x16 (OpenRazer). Likely OpenRazer omits the underglow columns; verify with 0F/80."],
 "razer-huntsman-v2": ["Transaction id disagreement (0x1F OpenRazer vs 0x3F OpenRGB). OpenRazer's value is per-command code with explicit cases; Synapse itself uses a rolling counter (PROTOCOL.md), so firmware may accept either. Try 0x1F first, fall back to 0x3F.", "HyperPolling keyboard: poll via 00/40 + 00/C0 up to 8000 Hz (OpenRazer POLL_RATES)."],
 "razer-huntsman-v2-tkl": ["As Huntsman V2 (txn 0x1F vs 0x3F). Matrix 6x18 (OpenRazer) vs 6x17 (OpenRGB)."],
 "razer-huntsman-v3-pro": ["Txn 0x1F (OpenRazer) vs 0x3F (OpenRGB). Analog actuation/rapid-trigger commands are not in either source.", "Sibling Huntsman V3 Pro 8KHz (0x02CF) is the only device where OpenRazer reads the active profile (05/84) and uses it as the storage byte of 0F/02 effects (razerkbd_driver.c:689 razer_get_active_varstore)."],
 "razer-huntsman-v3-pro-tkl": ["Matrix 6x22 (OpenRazer, same as full-size) vs 6x19 (OpenRGB); OpenRazer's value looks copied from the full board. Txn 0x1F vs 0x3F."],
 "razer-ornata-v3": ["Two PIDs: 0x028F (OpenRazer RazerOrnataV3_Alternate / OpenRGB 'Ornata V3') and 0x02A1 (OpenRazer RazerOrnataV3 / OpenRGB 'Ornata V3 rev2'). Zone-lit membrane, 10 zones."],
 "razer-deathadder-v3": ["No RGB. HyperPolling: poll via 00/40 + 00/C0 (125..8000 Hz). Not in OpenRGB (nothing to light)."],
 "razer-deathadder-v3-pro": ["No RGB; not in OpenRGB. 0x00C2/0x00C3 are alternate PIDs that OpenRazer handles identically to 0x00B6/0x00B7.", "OpenRazer daemon lists poll rates 125/500/1000 only (default list); the HyperPolling dongle (0x00B3, not listed here) is a separate device in OpenRazer."],
 "razer-viper-v2-pro": ["No RGB; not in OpenRGB. Low-battery threshold get/set use 0xFF while everything else uses 0x1F (razermouse_driver.c)."],
 "razer-viper-v3-pro": ["No RGB; not in OpenRGB. Wired 0x00C0 uses classic poll 00/05 (125/500/1000); wireless 0x00C1 uses 00/40 + 00/C0 up to 8000 Hz and the 59.9 ms receiver wait.", "Wireless PID also exposes HyperPolling dongle indicator-LED commands (07/10, 07/90) sent with 0xFF."],
 "razer-basilisk-v3": ["Feature reports on interface 3 (OpenRazer wIndex 0x03, OpenRGB 3/0x0C/0x01), unlike most mice (interface 0).", "DPI stages commands use 0xFF in OpenRazer while the rest use 0x1F. OpenRGB forces breathing off for Basilisk V3 family (software breathing in Synapse captures)."],
 "razer-basilisk-v3-35k": ["Interface 3 like the Basilisk V3. Matrix 1x13 (OpenRazer) vs 1x11 (OpenRGB, same as Basilisk V3)."],
 "razer-naga-v2-pro": ["OpenRGB calls it 'Naga Pro V2'. Set-DPI uses VARSTORE on this mouse (razermouse_driver.c, Naga group) where most mice use NOSTORE."],
 "razer-mouse-dock-pro": ["Transaction id 0xFF (both). OpenRGB notes wave works although the firmware does not list it; reactive is listed but needs a linked device. 1x8 (OpenRazer) vs 1x9 (OpenRGB)."],
 "razer-chroma-addressable-rgb-controller": ["Two interfaces: interface 0 carries the normal 90-byte commands, interface 1 a larger ARGB frame report (report id 0x04 for channels 0-4, 0x84 for channel 5+, header [channel, channel, pad, last_idx] + up to 315 colour bytes). OpenRazer razercommon.c razer_send_argb_msg and OpenRGB RazerController.cpp razer_create_custom_frame_argb_report agree. 6 channels x up to 80 LEDs; channel LED ids 0x1A-0x1F."],
 "razer-firefly-v2": ["Reactive is listed by OpenRazer with trigger_reactive_effect (host-triggered)."],
}

def effects_from(methods):
    if not methods: return None
    eff = []
    for m, n in B.EFFECT_MAP:
        if m in methods: eff.append(n)
    zones = {}
    for z, led in B.ZONE_PREFIX.items():
        zm = sorted({m[len("set_" + z + "_"):] for m in methods if m.startswith("set_" + z + "_") and not m.endswith("brightness")})
        if zm: zones[z] = {"led_id": "0x%02X" % led, "effects": zm}
    return eff, zones

def norm_hex(x):
    try: return "0x%02X" % int(x, 16)
    except Exception: return x

devices = []
for d in raw["devices"]:
    usb = []
    conflicts = []
    notes = []
    first = None
    for u in d["usb"]:
        r = u["openrgb"]; o = u["openrazer_daemon"]; v = u["openrazer_driver"]
        rg_tx = norm_hex(r["transaction_id"]) if r else None
        or_tx = (v["txn_lighting"] or v["txn_majority"]) if v else None
        if v and v["txn_other"] and v["txn_lighting"] and v["txn_other"] != v["txn_lighting"]:
            notes.append("PID %s: OpenRazer sends lighting with %s but other commands with %s (functions: %s)" % (u["product_id"], v["txn_lighting"], v["txn_other"], ", ".join(sorted(k.replace("razer_attr_", "") for k, t in v["txn_exceptions"].items() if v["txn_other"] in t)) or "majority"))
        if rg_tx and or_tx:
            tx_conf = "agree" if rg_tx == or_tx else "conflict"
            if tx_conf == "conflict":
                conflicts.append("PID %s transaction id: OpenRGB %s vs OpenRazer driver %s" % (u["product_id"], rg_tx, or_tx))
        else:
            tx_conf = "one source" if (rg_tx or or_tx) else "unknown"
        # interface
        rg_if = [x for x in (r.get("detect") or [])] if r else []
        or_if = v["transport"]["report_index_interface"] if v and v.get("transport") else None
        iface = None; up = None; us = None; if_conf = "unknown"
        if rg_if:
            iface = int(rg_if[-1]["interface"], 16) if rg_if[-1]["interface"] else None
            up = rg_if[-1]["usage_page"]; us = rg_if[-1]["usage"]
        if or_if is not None and iface is not None:
            if_conf = "agree" if int(or_if, 16) == iface else "conflict"
            if if_conf == "conflict":
                conflicts.append("PID %s feature-report interface: OpenRGB %d vs OpenRazer wIndex %s" % (u["product_id"], iface, or_if))
        elif or_if is not None:
            iface = int(or_if, 16); if_conf = "one source (OpenRazer wIndex; usage page/usage unknown)"
        elif iface is not None:
            if_conf = "one source (OpenRGB)"
        if len(rg_if) > 1:
            conflicts.append("PID %s: OpenRGB registers %d HID collections (%s) - firmware-dependent usage page/usage" % (
                u["product_id"], len(rg_if), "; ".join("%s/%s/%s" % (x["interface"], x["usage_page"], x["usage"]) for x in rg_if)))
        # matrix
        rdim = [r["rows"], r["cols"]] if r else None
        odim = o["matrix_dims"] if o else None
        if rdim and odim:
            m_conf = "agree" if rdim == odim else "conflict"
            if m_conf == "conflict":
                conflicts.append("PID %s matrix: OpenRGB %dx%d vs OpenRazer %dx%d" % (u["product_id"], rdim[0], rdim[1], odim[0], odim[1]))
        else:
            m_conf = "one source" if (rdim or odim) else "none (no RGB matrix in either source)"
        entry = {
            "product_id": u["product_id"], "connection": u["connection"],
            "interface": iface, "usage_page": up, "usage": us, "interface_confidence": if_conf,
            "transaction_id": (rg_tx if tx_conf == "agree" else (or_tx or rg_tx) if tx_conf == "one source" else None),
            "transaction_id_candidates": {"openrgb": rg_tx, "openrazer_lighting": (v or {}).get("txn_lighting"), "openrazer_other": (v or {}).get("txn_other")},
            "transaction_id_confidence": tx_conf,
            "transaction_id_exceptions_openrazer": (v or {}).get("txn_exceptions") or None,
            "matrix": {"openrgb": rdim, "openrazer": odim, "confidence": m_conf},
            "sources": {"openrgb": r, "openrazer_daemon": o, "openrazer_driver": v},
        }
        usb.append(entry)
        if first is None: first = u
    # device-level summary from first PID that has data
    o = next((u["openrazer_daemon"] for u in d["usb"] if u["openrazer_daemon"]), None)
    r = next((u["openrgb"] for u in d["usb"] if u["openrgb"]), None)
    v = next((u["openrazer_driver"] for u in d["usb"] if u["openrazer_driver"]), None)
    meths = (o or {}).get("methods") or []
    eff = effects_from(meths)
    lighting = None
    if r or (o and (o.get("has_matrix") or eff and (eff[0] or eff[1]))):
        lighting = {
            "per_led_custom_frame": {"openrazer": "set_custom_effect" in meths if o else None, "openrgb": (r["rows"] * r["cols"] > 0) if r else None},
            "matrix_commands": (v or {}).get("matrix_cmds") or ("extended (0F)" if r and r["matrix_type"].startswith("extended") else None),
            "openrgb_zones": r["zones"] if r else None,
            "effect_led_id": {"openrgb": r["led_id_for_effects"] if r else None,
                               "openrazer": ("0x05 (BACKLIGHT_LED) for keyboards" if d["kind"] == "keyboard" else "0x00 (ZERO_LED) for whole-device set_*_effect; zone methods use the ids below")},
            "hw_effects_openrazer_whole_device": eff[0] if eff else None,
            "hw_effects_openrazer_zones": eff[1] if eff else None,
            "openrgb_wave_without_effect_list": (re.sub(r"^RAZER_|_PID$", "", B.pidsym_rgb.get(int(first["product_id"], 16), "")) in B.WAVE_FALLBACK),
            "openrgb_breathing_forced_off": (re.sub(r"^RAZER_|_PID$", "", B.pidsym_rgb.get(int(first["product_id"], 16), "")) in B.NO_HW_BREATH),
            "note": "OpenRGB asks the firmware for regions (0F/80) and per-region effects (0F/81) at runtime and only falls back to its PID lists; uncoil can do the same read-only probe.",
        }
    feat = {
        "dpi": "set_dpi_xy" in meths, "dpi_stages": "set_dpi_stages" in meths, "poll_rate": "set_poll_rate" in meths,
        "battery": "get_battery" in meths, "charging_status": "is_charging" in meths, "idle_timer": "set_idle_time" in meths,
        "low_battery_threshold": "set_low_battery_threshold" in meths, "scroll_mode": "set_scroll_mode" in meths,
        "scroll_acceleration": "set_scroll_acceleration" in meths, "scroll_smart_reel": "set_scroll_smart_reel" in meths,
        "keyswitch_optimization": "set_keyswitch_optimization" in meths, "game_mode": "set_game_mode" in meths,
        "macro_mode_led": "set_macro_mode" in meths, "keyboard_layout_query": "get_keyboard_layout" in meths,
        "brightness": any(m.endswith("brightness") and m.startswith("set_") for m in meths),
    } if o else None
    dev = {"id": d["id"], "name": d["name"], "kind": d["kind"], "status": "experimental (maintainer does not own)",
           "usb": usb, "lighting": lighting, "features_openrazer": feat}
    if d["kind"] == "keyboard":
        form, extra = KB_FORM[d["id"]]
        lt = r["layout_table"] if r else None
        dev["keyboard"] = {
            "form_factor": form, "form_factor_note": extra,
            "openrgb_layout_table": lt,
            "openrgb_layout_size": B.layout_size.get(lt, (None,))[0] if lt else None,
            "openrgb_layout_cite": ("OpenRGB RazerDevices.cpp:%d (%s): base KeyboardLayoutManager layout + overlay edits; key names and positions come from KeyboardLayoutManager" % (B.layout_size[lt][1], lt)) if lt in B.layout_size else None,
            "ansi_iso": "sold in regional variants; read at runtime with 00/86 get keyboard layout (OpenRazer razerkbd_driver.c:503 razer_attr_read_kbd_layout; OpenRGB RazerController.cpp razer_get_keyboard_info, 1=US ANSI, 6=UK ISO ...)",
            "poll_rates_by_pid": {u["product_id"]: {"rates": (u["openrazer_daemon"] or {}).get("poll_rates"), "command": (u["openrazer_driver"] or {}).get("poll_command")} for u in d["usb"]},
            "remap_key_ids": "not in OpenRazer or OpenRGB; see notes Q1 (HID AT-101 positions, verified on BW V4 Pro 75% only)",
        }
    if d["kind"] == "mouse":
        dev["mouse"] = {
            "dpi_max": (o or {}).get("dpi_max"), "dpi_max_cite": (o or {}).get("cite"),
            "dpi_stages": feat["dpi_stages"] if feat else None, "dpi_stages_max": 5 if feat and feat["dpi_stages"] else None,
            "dpi_stages_cite": "OpenRazer razermouse_driver.h:137 RAZER_MOUSE_MAX_DPI_STAGES=5 (driver limit)",
            "poll_rates_by_pid": {u["product_id"]: {"rates": (u["openrazer_daemon"] or {}).get("poll_rates"), "source": (u["openrazer_daemon"] or {}).get("poll_rates_source"), "command": (u["openrazer_driver"] or {}).get("poll_command")} for u in d["usb"]},
            "battery": feat["battery"] if feat else None, "charging": feat["charging_status"] if feat else None,
            "idle_timer": feat["idle_timer"] if feat else None, "idle_timer_range_s": [60, 900] if feat and feat["idle_timer"] else None,
            "low_battery_threshold": feat["low_battery_threshold"] if feat else None,
            "low_battery_threshold_raw_range": ["0x0C", "0x3F"] if feat and feat["low_battery_threshold"] else None,
            "buttons": None, "button_ids": None,
            "buttons_note": "Neither OpenRazer nor OpenRGB maps mouse buttons; read the ids on hardware with 02/84 (as done for the Basilisk V3 Pro).",
            "openrazer_wait_by_pid": {u["product_id"]: ((u["openrazer_driver"] or {}).get("transport") or {}).get("wait") for u in d["usb"]},
        }
    dev["conflicts"] = conflicts
    dev["notes"] = notes + MANUAL_NOTES.get(d["id"], [])
    for c in MANUAL_CONFLICTS.get(d["id"], []): dev["conflicts"].append(c)
    devices.append(dev)

out = {
    "generated": "2026-10-03",
    "sources": {
        "openrazer": {"repo": "https://github.com/openrazer/openrazer", "commit": raw["sources"]["openrazer_commit"], "license": "GPL-2.0 (facts transcribed only)"},
        "openrgb": {"repo": "https://gitlab.com/CalcProgrammer1/OpenRGB", "commit": raw["sources"]["openrgb_commit"], "license": "GPL-2.0-or-later (facts transcribed only)"},
    },
    "legend": {
        "interface": "USB interface number carrying the 90-byte feature report (OpenRGB HID detector interface; OpenRazer uses it as the control-transfer wIndex)",
        "transaction_id": "set only when the sources agree or only one source exists; otherwise null with candidates listed",
        "confidence": "agree = both sources match; one source; conflict = see conflicts[]",
        "line numbers": "refer to the commits above",
    },
    "devices": devices,
}
json.dump(out, open(os.path.join(H, "razer-devices.json"), "w"), indent=1)
print(len(devices), "devices;", sum(len(x["conflicts"]) for x in devices), "conflicts")
for x in devices:
    for c in x["conflicts"]: print(" ", x["id"], "|", c)
