"""Driver-mode capture: put the BlackWidow in Synapse's driver mode, log every input report from every
openable HID collection plus what Windows sees as keystrokes, then ALWAYS restore normal mode."""
import sys, time, threading, hid
from pynput import keyboard
sys.path.insert(0, __import__('os').path.dirname(__import__('os').path.abspath(__file__)))
from rgb_effect import razer_report

# The log holds every key typed, so it goes in the repository's git-ignored captures/ folder (or anywhere
# outside the repository); a path elsewhere inside the repository is refused so it can't be committed.
_repo = __import__('os').path.abspath(__import__('os').path.join(__import__('os').path.dirname(__file__), '..', '..'))
_captures = __import__('os').path.join(_repo, 'captures')
if len(sys.argv) < 2:
    sys.exit('usage: drv_capture.py NAME [SECONDS]   (written to captures/NAME unless NAME is a path outside the repo)')
OUT = sys.argv[1] if __import__('os').path.isabs(sys.argv[1]) else __import__('os').path.join(_captures, sys.argv[1])
OUT = __import__('os').path.abspath(OUT)
if OUT.startswith(_repo + __import__('os').sep) and not OUT.startswith(_captures + __import__('os').sep):
    sys.exit(f'refusing to write keystrokes inside the repository outside captures/: {OUT}')
__import__('os').makedirs(__import__('os').path.dirname(OUT), exist_ok=True)
DURATION = int(sys.argv[2]) if len(sys.argv) > 2 else 240
t0 = time.time(); lock = threading.Lock()
def log(m):
    with lock, open(OUT, 'a', encoding='utf-8') as f:
        f.write(f"{time.time()-t0:7.2f}  {m}\n")

ctrl_info = next(i for i in hid.enumerate(0x1532, 0x02B3) if (i['interface_number'], i['usage_page'], i['usage']) == (3, 0x0C, 0x01))
ctrl = hid.device(); ctrl.open_path(ctrl_info['path'])
def set_mode(m):
    ctrl.send_feature_report(razer_report(0x1F, 0x00, 0x04, [m, 0])); time.sleep(0.005)
    r = ctrl.get_feature_report(0, 91); log(f"SET DEVICE MODE {m} -> status {r[1]}")

def reader(info):
    tag = f"if{info['interface_number']} {info['usage_page']:#06x}/{info['usage']:#04x}"
    try:
        d = hid.device(); d.open_path(info['path']); d.set_nonblocking(True)
    except Exception as e:
        log(f"HID {tag} cannot open ({e})"); return
    while time.time() - t0 < DURATION:
        data = d.read(64)
        if data and any(data):
            log(f"HID {tag}  {bytes(data).hex(' ')}")
        time.sleep(0.001)

# Privacy: never record what is typed. Only special (non-character) keys are logged by name.
def on_press(k):
    if hasattr(k, 'char') and k.char is not None:
        log("KEY down <char>")
    else:
        log(f"KEY down {k}")
def on_release(k):
    pass

open(OUT, 'w').close()
try:
    for info in hid.enumerate(0x1532, 0x02B3):
        threading.Thread(target=reader, args=(info,), daemon=True).start()
    keyboard.Listener(on_press=on_press, on_release=on_release).start()
    time.sleep(0.5)
    set_mode(3)                                    # driver mode
    log("READY")
    time.sleep(DURATION - 1)
finally:
    set_mode(0)                                    # always back to normal mode
    log("done (normal mode restored)")
