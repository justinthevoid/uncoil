"""Map Fn+P (Hypershift layer of key 26) to Print Screen in the BlackWidow's onboard memory.
get = 0x02/0x8D [profile, key, layer] -> [profile, key, layer, fn, len, data...]
set = 0x02/0x0D [profile, key, layer, fn, len, data...]   (fn 2 = KeyCode, data [modifiers, HID usage])
Usage: python obm_set_fnp.py            # set Fn+P -> PrintScreen (HID 0x46)
       python obm_set_fnp.py --restore  # back to KeyCode [0, 0] (what Synapse left)"""
import sys, time, hid, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

TID = 0x1F


def report(size, cls, cid, args):
    r = bytearray(91)
    r[2] = TID; r[6] = size; r[7] = cls; r[8] = cid
    r[9:9 + len(args)] = bytes(args)
    c = 0
    for b in r[3:89]:
        c ^= b
    r[89] = c
    return bytes(r)


info = next(i for i in hid.enumerate(0x1532, 0x02B3) if (i['interface_number'], i['usage_page'], i['usage']) == (3, 0x0C, 0x01))
d = hid.device(); d.open_path(info['path'])


def ask(size, cls, cid, args):
    d.send_feature_report(report(size, cls, cid, list(args)))
    time.sleep(0.015)
    r = d.get_feature_report(0, 91)
    return r[1], list(r[9:9 + 12])


def get(key, layer):
    st, a = ask(0x50, 0x02, 0x8D, [1, key, layer])
    return st, a[:5 + a[4]] if st == 2 else a


print('check  Fn+Delete :', get(76, 1))
print('check  Fn+F9     :', get(120, 1))
print('before Fn+P      :', get(26, 1))
data = [0, 0] if '--restore' in sys.argv else [0, 0x46]
st, a = ask(0x50, 0x02, 0x0D, [1, 26, 1, 2, 2] + data)
print(f'set    Fn+P -> {data}: status={st}')
print('after  Fn+P      :', get(26, 1))
print('after  P normal  :', get(26, 0))
d.close()
