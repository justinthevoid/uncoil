"""Walk OpenRazer kernel driver functions: for each attr function, map case-label PIDs to the
transaction id and the razer_chroma_* helper calls used in that case group."""
import re, os, sys, json, collections
D = os.path.join(os.path.dirname(os.path.abspath(__file__)), "src")
def rd(f): return open(os.path.join(D, f), encoding="utf-8", errors="replace").read()

defs = {}
for h in ["or_razermouse_driver.h", "or_razerkbd_driver.h", "or_razeraccessory_driver.h", "or_razercommon.h"]:
    for m in re.finditer(r"#define\s+(USB_DEVICE_ID_RAZER_\w+)\s+(0x[0-9A-Fa-f]+)", rd(h)):
        defs[m.group(1)] = int(m.group(2), 16)

FUNC_RE = re.compile(r"^(?:static\s+)?[\w\s\*]+?\b(\w+)\s*\(")
SW_RE = re.compile(r"\bswitch\s*\(")
CASE_RE = re.compile(r"case\s+(USB_DEVICE_ID_RAZER_\w+)\s*:")
DEF_RE = re.compile(r"\bdefault\s*:")
TXN_RE = re.compile(r"transaction_id\.id\s*=\s*(0x[0-9A-Fa-f]+)")
CALL_RE = re.compile(r"\b(razer_chroma_\w+|razer_naga_\w+)\s*\(")
BRK_RE = re.compile(r"\bbreak\s*;|\breturn\b")

def walk(fname):
    src = rd(fname).split("\n")
    res = collections.defaultdict(lambda: collections.defaultdict(list))
    func = None; depth = 0; stack = []  # [switch_depth, group, fresh]
    for i, line in enumerate(src, 1):
        code = re.sub(r"//.*", "", line)
        if depth == 0:
            m = FUNC_RE.match(code)
            if m and not code.strip().endswith(";"):
                func = m.group(1); stack = []
        if func and SW_RE.search(code):
            stack.append([depth, [], True])
        if stack:
            top = stack[-1]
            cs = CASE_RE.findall(code)
            if DEF_RE.search(code): cs.append("default")
            if cs:
                if top[2]: top[1] = []
                top[1] += cs; top[2] = False
            else:
                grp = []
                for st in stack:
                    if st[1]: grp = st[1]
                t = TXN_RE.search(code)
                calls = CALL_RE.findall(code)
                for g in grp:
                    if t: res[g][func].append(("txn", "0x" + t.group(1)[2:].upper(), i))
                    for c in calls: res[g][func].append(("call", c, i))
                if BRK_RE.search(code):
                    top[2] = True
        elif func:
            t = TXN_RE.search(code)
            if t: res["__all__"][func].append(("txn", "0x" + t.group(1)[2:].upper(), i))
        depth += code.count("{") - code.count("}")
        while stack and depth <= stack[-1][0] and "}" in code:
            stack.pop()
    return res

ALL = {}
for f in ["or_razermouse_driver.c", "or_razerkbd_driver.c", "or_razeraccessory_driver.c"]:
    for pid, funcs in walk(f).items():
        for fn, items in funcs.items():
            ALL.setdefault(pid, {})[f[3:] + ":" + fn] = items

def summary(sym):
    out = {}
    for fn, items in sorted(ALL.get(sym, {}).items()):
        out[fn] = {"txn": sorted({x[1] for x in items if x[0] == "txn"}),
                   "calls": sorted({x[1] for x in items if x[0] == "call"}),
                   "line": min(x[2] for x in items)}
    return out

if __name__ == "__main__":
    want = [int(x, 16) for x in sys.argv[1:]]
    for sym, val in defs.items():
        if val in want:
            print("=== %s 0x%04X" % (sym, val))
            txns = collections.Counter()
            for fn, s in summary(sym).items():
                for t in s["txn"]: txns[t] += 1
                print("  %-58s L%-5d txn=%s calls=%s" % (fn.split(":")[1], s["line"], ",".join(s["txn"]) or "-", ",".join(s["calls"])))
            print("  TXN COUNTS:", dict(txns))
