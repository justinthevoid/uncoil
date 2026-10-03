"""Summarise a drv_capture.py log: split into sections by single-letter markers the user typed (a, b, c...),
and show, per section, the raw HID reports and the keystrokes Windows saw."""
import re, sys, collections

lines = open(sys.argv[1], encoding='utf-8').read().splitlines()
sections = collections.OrderedDict({'(start)': []})
cur = '(start)'
prev_char_down = None
for ln in lines:
    m = re.match(r'\s*([\d.]+)\s+(.*)', ln)
    if not m:
        continue
    t, body = float(m.group(1)), m.group(2)
    k = re.match(r"KEY down '(.)'", body)
    if k and k.group(1) in 'abcdef' and prev_char_down != k.group(1):
        cur = f"[{k.group(1)}] @{t:.1f}s"
        sections[cur] = []
        prev_char_down = k.group(1)
        continue
    if body.startswith('KEY up'):
        continue
    sections[cur].append((t, body))

for name, evs in sections.items():
    print(f'===== {name}  ({len(evs)} events)')
    last = None
    for t, b in evs:
        b = re.sub(r'HID (if\d+ 0x\w+/0x\w+)\s+', r'\1 ', b)
        # strip trailing zero bytes for readability
        b = re.sub(r'( 00)+$', ' ..', b)
        if b == last:
            continue
        print(f'  {t:7.2f}  {b}')
        last = b
