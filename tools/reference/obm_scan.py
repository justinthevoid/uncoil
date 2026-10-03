"""Scan READ-ONLY (high-bit) command ids in class 0x02 on the BlackWidow to find the keyboard's
key-mapping getter. Prints the ones that answer OK and their reply bytes."""
import sys, time, hid, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from obm_probe import report  # noqa: E402  (same builder, tid 0x1F)

info = next(i for i in hid.enumerate(0x1532, 0x02B3) if (i['interface_number'], i['usage_page'], i['usage']) == (3, 0x0C, 0x01))
d = hid.device(); d.open_path(info['path'])

def ask(size, cls, cid, args):
    d.send_feature_report(report(size, cls, cid, list(args)))
    time.sleep(0.012)
    r = d.get_feature_report(0, 91)
    return r[1], r[6], list(r[9:9 + 24])

for cid in range(0x80, 0xA0):
    for args, label in [([1, 26, 1], 'prof1 key26(P) hyper'), ([1, 26, 0], 'prof1 key26(P) normal')]:
        st, n, data = ask(0x50, 0x02, cid, args)
        if st == 2:
            print(f'0x02/0x{cid:02X} {label:22s} OK len={n} data={data}')
        elif st not in (5,):
            print(f'0x02/0x{cid:02X} {label:22s} status={st}')
d.close()
