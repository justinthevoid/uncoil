import ast, os, re, sys, json, types
D = os.path.join(os.path.dirname(os.path.abspath(__file__)), "src")
KEEP = {"USB_VID", "USB_PID", "HAS_MATRIX", "MATRIX_DIMS", "METHODS", "DPI_MAX", "POLL_RATES", "WAVE_DIRS",
        "DEVICE_IMAGE", "EVENT_FILE_REGEX", "KEY_LAYOUT", "RAZER_URLS", "ZONES", "BATTERY_LEVEL", "POLL_RATE"}

classes = {}
for fname in ["or_keyboards.py", "or_mouse.py", "or_mouse_mat.py", "or_accessory.py"]:
    src = open(os.path.join(D, fname), encoding="utf-8").read()
    tree = ast.parse(src)
    for node in tree.body:
        if not isinstance(node, ast.ClassDef):
            continue
        bases = [b.id if isinstance(b, ast.Name) else ast.unparse(b) for b in node.bases]
        attrs, lines = {}, {}
        # inherit
        for b in bases:
            if b in classes:
                attrs.update(classes[b]["attrs"]); lines.update(classes[b]["lines"])
        ns = {k: types.SimpleNamespace(**v["attrs"]) for k, v in classes.items()}
        for st in node.body:
            if isinstance(st, ast.Assign) and len(st.targets) == 1 and isinstance(st.targets[0], ast.Name):
                k = st.targets[0].id
                try:
                    v = ast.literal_eval(st.value)
                except Exception:
                    try:
                        local = dict(ns); local.update({kk: vv for kk, vv in attrs.items()})
                        local["re"] = re
                        v = eval(compile(ast.Expression(st.value), fname, "eval"), {"re": re}, local)
                        if isinstance(v, re.Pattern): v = v.pattern
                    except Exception as e:
                        v = "<expr:%s>" % ast.unparse(st.value)[:120]
                attrs[k] = v; lines[k] = "%s:%d" % (fname[3:], st.lineno)
        doc = ast.get_docstring(node) or ""
        classes[node.name] = {"file": fname[3:], "line": node.lineno, "bases": bases, "doc": doc.strip(), "attrs": attrs, "lines": lines}

if __name__ == "__main__":
    pat = sys.argv[1]
    for k, v in classes.items():
        if re.search(pat, k, re.I):
            a = v["attrs"]
            out = {"class": k, "at": "%s:%d" % (v["file"], v["line"]), "bases": v["bases"],
                   "pid": hex(a["USB_PID"]) if isinstance(a.get("USB_PID"), int) else a.get("USB_PID"),
                   "dims": a.get("MATRIX_DIMS"), "has_matrix": a.get("HAS_MATRIX"), "dpi_max": a.get("DPI_MAX"),
                   "poll": a.get("POLL_RATES"), "wave": a.get("WAVE_DIRS"), "layout": a.get("KEY_LAYOUT"),
                   "methods": a.get("METHODS"), "src_lines": {kk: vv for kk, vv in v["lines"].items() if kk in ("USB_PID", "MATRIX_DIMS", "METHODS", "DPI_MAX", "POLL_RATES")}}
            if "--short" in sys.argv:
                out.pop("methods")
            print(json.dumps(out))
