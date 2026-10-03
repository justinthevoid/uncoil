"""READ-ONLY probe of the features uncoil's daemon exposes (profiles, key map, OLED, dial, lighting regions).

Every command sent here is a "get" (command id with the high bit set); the script refuses to send anything
else, so it cannot change onboard memory. Replies are matched to the request (class/id echo) and retried, so
it tolerates other traffic on the same collection. Even so, prefer running it with uncoild stopped (see
CLAUDE.md, "one writer at a time"); the results in docs/PROTOCOL.md were read with the display off, when
uncoild sends nothing. With the new daemon, `uncoil keymap get/dump`, `uncoil oled get` etc. read the
same values through the daemon instead.

Usage: python readonly_probe.py [keyboard|mouse|all]"""
import sys, time, hid

KB = (0x02B3, 3, 0x0C, 0x01, 0x1F)
MOUSE = (0x00AA, 0, 0x01, 0x02, 0x1F)


def report(tid, size, cls, cid, args):
    assert cid & 0x80, f'refusing to send non-read command {cls:02X}/{cid:02X}'
    r = bytearray(91)
    r[2] = tid; r[6] = size; r[7] = cls; r[8] = cid
    r[9:9 + len(args)] = bytes(args)
    c = 0
    for b in r[3:89]:
        c ^= b
    r[89] = c
    return bytes(r)


def open_dev(pid, iface, page, usage):
    info = next((i for i in hid.enumerate(0x1532, pid)
                 if (i['interface_number'], i['usage_page'], i['usage']) == (iface, page, usage)), None)
    if not info:
        return None
    d = hid.device(); d.open_path(info['path'])
    return d


def ask(d, tid, size, cls, cid, args=(), n=24):
    for attempt in range(6):
        d.send_feature_report(report(tid, size, cls, cid, list(args)))
        time.sleep(0.012 + attempt * 0.01)
        r = d.get_feature_report(0, 91)
        st, rcls, rcid, rsz = r[1], r[7], r[8], r[6]
        if (rcls, rcid) != (cls, cid) or st == 1:
            continue  # someone else's reply, or busy: retry
        return st, rsz, list(r[9:9 + n])
    return None, 0, []


def show(label, res):
    st, sz, a = res
    name = {2: 'ok', 3: 'fail', 4: 'timeout', 5: 'unsupported', None: 'no matching reply'}.get(st, st)
    print(f'  {label:44s} {name:12s} size={sz:<3d} {a if st == 2 else ""}')


def keyboard():
    d = open_dev(*KB[:4]); tid = KB[4]
    if not d:
        print('keyboard not found'); return
    print('BlackWidow V4 Pro 75% (read-only):')
    show('05/8A max profiles', ask(d, tid, 0x01, 0x05, 0x8A, n=1))
    show('05/80 profile count', ask(d, tid, 0x01, 0x05, 0x80, n=1))
    show('05/81 profile ids', ask(d, tid, 0x50, 0x05, 0x81, n=8))
    for cid in (0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89):
        show(f'05/{cid:02X} (profile class, unknown)', ask(d, tid, 0x50, 0x05, cid, n=8))
    show('02/8D [1,26,0] P normal', ask(d, tid, 0x50, 0x02, 0x8D, [1, 26, 0], n=12))
    show('02/8D [1,26,1] Fn+P', ask(d, tid, 0x50, 0x02, 0x8D, [1, 26, 1], n=12))
    show('02/8D [1,76,1] Fn+Delete', ask(d, tid, 0x50, 0x02, 0x8D, [1, 76, 1], n=12))
    show('02/8D [1,120,1] Fn+F9', ask(d, tid, 0x50, 0x02, 0x8D, [1, 120, 1], n=12))
    show('02/8F [1,1,0] bulk get', ask(d, tid, 0x50, 0x02, 0x8F, [1, 1, 0], n=80))
    for cid in (0x84, 0x88):
        show(f'02/{cid:02X} key id list?', ask(d, tid, 0x50, 0x02, cid, n=40))
    show('17/80 [1] dial active mode?', ask(d, tid, 0x04, 0x17, 0x80, [1], n=8))
    show('17/82 OLED home screen', ask(d, tid, 0x07, 0x17, 0x82, n=7))
    show('17/83 OLED brightness', ask(d, tid, 0x01, 0x17, 0x83, n=1))
    show('17/84 time to home screen', ask(d, tid, 0x01, 0x17, 0x84, n=1))
    show('17/85 language', ask(d, tid, 0x01, 0x17, 0x85, n=1))
    show('17/86 time to dim', ask(d, tid, 0x01, 0x17, 0x86, n=1))
    show('17/8D home screen active item', ask(d, tid, 0x02, 0x17, 0x8D, n=2))
    show('17/92 low battery warning %', ask(d, tid, 0x01, 0x17, 0x92, n=1))
    show('17/93 OLED low power mode', ask(d, tid, 0x01, 0x17, 0x93, n=1))
    show('17/96 screensaver', ask(d, tid, 0x02, 0x17, 0x96, n=2))
    show('0F/80 lighting regions', ask(d, tid, 0x50, 0x0F, 0x80, n=30))
    show('0F/81 [5] backlight effects', ask(d, tid, 0x50, 0x0F, 0x81, [5], n=20))
    show('0F/81 [0] zero-led effects', ask(d, tid, 0x50, 0x0F, 0x81, [0], n=20))
    show('0F/84 [1,5] backlight brightness', ask(d, tid, 0x03, 0x0F, 0x84, [1, 5], n=3))
    show('00/84 device mode', ask(d, tid, 0x02, 0x00, 0x84, n=2))
    d.close()


def mouse():
    d = open_dev(*MOUSE[:4]); tid = MOUSE[4]
    if not d:
        print('mouse (wired) not found'); return
    print('Basilisk V3 Pro (read-only):')
    show('05/8A max profiles', ask(d, tid, 0x01, 0x05, 0x8A, n=1))
    show('05/80 profile count', ask(d, tid, 0x01, 0x05, 0x80, n=1))
    show('05/81 profile ids', ask(d, tid, 0x50, 0x05, 0x81, n=8))
    show('02/84 button ids', ask(d, tid, 0x50, 0x02, 0x84, n=20))
    for b in (1, 4, 14, 15, 96, 106):
        show(f'02/8C [1,{b},0]', ask(d, tid, 0x50, 0x02, 0x8C, [1, b, 0], n=10))
        show(f'02/8C [1,{b},1]', ask(d, tid, 0x50, 0x02, 0x8C, [1, b, 1], n=10))
    show('0F/80 lighting regions', ask(d, tid, 0x50, 0x0F, 0x80, n=30))
    show('0F/81 [0] effects', ask(d, tid, 0x50, 0x0F, 0x81, [0], n=20))
    show('00/84 device mode', ask(d, tid, 0x02, 0x00, 0x84, n=2))
    d.close()


if __name__ == '__main__':
    which = sys.argv[1] if len(sys.argv) > 1 else 'all'
    if which in ('keyboard', 'all'):
        keyboard()
    if which in ('mouse', 'all'):
        mouse()
