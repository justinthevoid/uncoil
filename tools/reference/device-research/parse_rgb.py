import re, json, os, sys
D = os.path.join(os.path.dirname(os.path.abspath(__file__)), "src")
def rd(f): return open(os.path.join(D, f), encoding="utf-8", errors="replace").read()

hdr = rd("rgb_RazerDevices.h")
pids = {}
for m in re.finditer(r"#define\s+(RAZER_\w+_PID)\s+(0x[0-9A-Fa-f]+)", hdr):
    pids[m.group(1)] = int(m.group(2), 16)

cpp = rd("rgb_RazerDevices.cpp")
lines = cpp.split("\n")
def lineno(pos): return cpp.count("\n", 0, pos) + 1

zones = {}
for m in re.finditer(r"static const razer_zone (\w+)\s*=\s*\{([^}]*)\}", cpp):
    parts = [p.strip() for p in m.group(2).split(",") if p.strip()]
    zones[m.group(1)] = {"name": parts[0], "type": parts[1], "rows": int(parts[2]), "cols": int(parts[3]), "line": lineno(m.start())}

devs = []
for m in re.finditer(r"static const razer_device (\w+)\s*=\s*\{(.*?)\n\};", cpp, re.S):
    body = m.group(2)
    name = re.search(r'"([^"]+)"', body).group(1)
    toks = [t.strip() for t in re.sub(r"\{[^}]*\}", "ZONES", body.split('"', 2)[2]).split(",") if t.strip()]
    # toks: pid, type, matrix_type, txn, rows, cols, ZONES, layout
    zl = re.search(r"\{([^}]*)\}", body).group(1)
    zlist = [z.strip().lstrip("&") for z in zl.split(",") if z.strip() and z.strip() != "NULL"]
    pidsym = toks[0]
    devs.append({
        "var": m.group(1), "name": name, "pid_sym": pidsym, "pid": hex(pids.get(pidsym, -1)) if pidsym in pids else pidsym,
        "type": toks[1], "matrix_type": toks[2], "txn": toks[3], "rows": toks[4], "cols": toks[5],
        "zones": [dict(zones.get(z, {"missing": z}), var=z) for z in zlist],
        "layout": toks[7].lstrip("&") if len(toks) > 7 else None,
        "line": lineno(m.start()),
    })

det_raw = rd("rgb_RazerControllerDetect.cpp")
NL = chr(10)
det = NL.join(("" if l.lstrip().startswith("//") else re.sub(r"/\*.*?\*/", "", l)) for l in det_raw.split(NL))
detect = {}
for m in re.finditer(r'REGISTER_HID_DETECTOR_(\w+)\("([^"]+)",\s*(\w+),\s*(\w+),\s*(\w+)(.*?)\);', det):
    rest = [x.strip() for x in m.group(6).split(",") if x.strip()]
    detect.setdefault(m.group(5), []).append({"macro": m.group(1), "name": m.group(2), "fn": m.group(3), "vid": m.group(4), "args": rest, "line": lineno.__call__(0) if False else det.count("\n", 0, m.start()) + 1})

if __name__ == "__main__":
    pat = sys.argv[1] if len(sys.argv) > 1 else ""
    if pat == "--list":
        for d in devs:
            print(d["pid"], d["name"], d["type"].replace("DEVICE_TYPE_", ""), d["matrix_type"].replace("RAZER_MATRIX_TYPE_", ""), d["txn"], f'{d["rows"]}x{d["cols"]}')
    else:
        out = [dict(d, detect=detect.get(d["pid_sym"])) for d in devs if re.search(pat, d["name"], re.I)]
        print(json.dumps(out, indent=1))
