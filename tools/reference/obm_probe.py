"""Read-only probe of the keyboard's onboard key mappings (OBM): profiles, and the Normal/Hypershift
assignment for a few keys. Command layout from OpenSynapse's ViperObmProtocol (MIT)."""
import sys, time, hid, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

TID = 0x1F
FUNCS = {0: 'Off', 1: 'ButtonCode', 2: 'KeyCode', 3: 'MacroI', 4: 'MacroII', 5: 'MacroIII', 6: 'Dpi', 7: 'Profile',
         8: 'Lighting', 9: 'PowerKeys', 10: 'MediaKeys', 11: 'DoubleClick', 12: 'ModeButtonKey', 13: 'TurboKey',
         14: 'TurboButton', 15: 'MacroIV', 16: 'Controller', 17: 'RazerKey', 18: 'WinShortcut'}


def report(size, cls, cid, args):
    r = bytearray(91)
    r[2] = TID; r[6] = size; r[7] = cls; r[8] = cid
    r[9:9 + len(args)] = bytes(args)
    c = 0
    for b in r[3:89]:
        c ^= b
    r[89] = c
    return bytes(r)


def ask(d, size, cls, cid, args=()):
    d.send_feature_report(report(size, cls, cid, list(args)))
    time.sleep(0.01)
    resp = d.get_feature_report(0, 91)
    return resp[1], resp[7], list(resp[9:9 + max(size, 1)])


info = next(i for i in hid.enumerate(0x1532, 0x02B3) if (i['interface_number'], i['usage_page'], i['usage']) == (3, 0x0C, 0x01))
d = hid.device(); d.open_path(info['path'])

st, n, a = ask(d, 0x01, 0x05, 0x8A); print(f'max profiles: status={st} -> {a[0]}')
st, n, a = ask(d, 0x01, 0x05, 0x80); print(f'profile count: status={st} -> {a[0]}')
st, n, a = ask(d, 0x50, 0x05, 0x81); print(f'profile ids: status={st} -> count={a[0]} ids={a[1:1 + a[0]]}')
profiles = a[1:1 + a[0]] if st == 2 and a[0] else [1]

for prof in profiles:
    for key, name in [(26, 'P'), (76, 'Delete'), (124, 'PrintScreen'), (120, 'F9')]:
        for mode in (0, 1):
            st, n, a = ask(d, 0x50, 0x02, 0x8C, [prof, key, mode])
            fn, ln = a[3], a[4]
            print(f'  profile {prof} key {key:3d} {name:11s} {"Normal    " if mode == 0 else "Hypershift"} '
                  f'status={st} len={n} -> fn={FUNCS.get(fn, fn)} data={a[5:5 + ln]}')
d.close()
