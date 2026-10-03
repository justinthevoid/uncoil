"""Set Razer hardware wave with a custom speed (OpenRGB hard-codes 0x28).

Usage: python razer_wave.py [speed] [direction]
  speed:     1-255, higher = slower (firmware default 0x28 = 40)
  direction: 1 or 2 (left/right)

Same 90-byte feature report OpenRGB/OpenRazer send: class 0x0F, id 0x02,
args = [storage, led_id, effect=0x04 wave, direction, speed].
"""
import sys
import hid

RAZER_VID = 0x1532
# pid -> (interface, usage_page, usage, transaction_id, led_id)
DEVICES = {
    0x02B3: (3, 0x0C, 0x01, 0x1F, 0x00),  # BlackWidow V4 Pro 75% (wired)
    0x00AA: (0, 0x01, 0x02, 0x1F, 0x00),  # Basilisk V3 Pro (wired)
    0x00AB: (0, 0x01, 0x02, 0x1F, 0x00),  # Basilisk V3 Pro (HyperSpeed dongle)
}

def build_report(tid, cls, cid, args):
    r = bytearray(91)          # [0] report id, then the 90-byte Razer report
    r[2] = tid                 # transaction id
    r[6] = len(args)           # data size
    r[7] = cls
    r[8] = cid
    r[9:9 + len(args)] = bytes(args)
    crc = 0
    for b in r[3:89]:
        crc ^= b
    r[89] = crc
    return bytes(r)

def main():
    speed = int(sys.argv[1], 0) if len(sys.argv) > 1 else 0x28
    direction = int(sys.argv[2]) if len(sys.argv) > 2 else 1
    speed = max(1, min(255, speed))
    for info in hid.enumerate(RAZER_VID):
        spec = DEVICES.get(info['product_id'])
        if not spec:
            continue
        iface, page, usage, tid, led = spec
        if (info['interface_number'], info['usage_page'], info['usage']) != (iface, page, usage):
            continue
        dev = hid.device()
        try:
            dev.open_path(info['path'])
            rep = build_report(tid, 0x0F, 0x02, [0x00, led, 0x04, direction, speed])
            dev.send_feature_report(rep)
            resp = dev.get_feature_report(0, 91)
            status = resp[1] if len(resp) > 1 else None
            print(f"{info['product_id']:04X}: wave dir={direction} speed={speed} status={status:#04x}" if status is not None else f"{info['product_id']:04X}: no response")
        except Exception as e:
            print(f"{info['product_id']:04X}: failed ({e})")
        finally:
            dev.close()

if __name__ == '__main__':
    main()
