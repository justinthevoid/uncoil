"""Angled rainbow wave across the Razer keyboard, mouse and mat as one shared desk space.

Razer devices are driven directly over USB HID (same custom-frame reports Synapse/OpenRGB use),
so no OpenRGB server has to stay running. Motherboard / GPU / RAM get their own hardware rainbow
once at startup via a one-shot OpenRGB CLI run (needs admin for the RAM; skipped while iCUE runs).

Like Synapse, lighting fades out when Windows turns the display off, dims with the display, and
fades back in when it wakes.

Run by the scheduled task "OpenRGB (elevated)" (logon, resume, Start menu "Reapply RGB").
Replugged Razer devices are picked up again automatically within a few seconds.
Physical LED positions live in rgb_layout.py.  preview.py renders a GIF of the current effect.

Tuning lives in rgb_effect.json next to this file and is re-read every second:
  angle_deg   wave angle (0 = sweeps left->right, 90 = top->bottom, 35 = diagonal)
  period_s    seconds for one full colour cycle to pass a point (higher = slower)
  wavelength  width of one full rainbow in key-widths (higher = wider bands)
  fps, brightness (0-1), saturation (0-1), reverse
  dim_level   brightness while Windows has dimmed the display (0-1)
"""
import ctypes, json, math, os, subprocess, threading, time
from ctypes import wintypes
import hid

import rgb_layout as layout

HERE = os.path.dirname(os.path.abspath(__file__))
CFG_PATH = os.path.join(HERE, 'rgb_effect.json')
LOG_PATH = os.path.join(HERE, 'rgb_effect.log')
DEFAULTS = dict(angle_deg=35, period_s=14, wavelength=26, fps=30, brightness=1.0, saturation=1.0,
                reverse=False, dim_level=0.35)
HEARTBEAT_S = 600
RESCAN_S = 5
FADE_S = 1.2            # display on/off fade time
NO_WINDOW = 0x08000000
OPENRGB_EXE = r'C:\Program Files\OpenRGB\OpenRGB.exe'

RAZER_VID = 0x1532
# kind, pid, interface, usage_page, usage, transaction_id, rows, cols
RAZER = [
    ('keyboard', 0x02B3, 3, 0x0C, 0x01, 0x1F, 6, 18),   # BlackWidow V4 Pro 75% (wired)
    ('mouse',    0x00AA, 0, 0x01, 0x02, 0x1F, 1, 13),   # Basilisk V3 Pro (wired)
    ('mouse',    0x00AB, 0, 0x01, 0x02, 0x1F, 1, 13),   # Basilisk V3 Pro (HyperSpeed dongle)
    ('mat',      0x0C02, 0, 0x01, 0x02, 0x3F, 1, 1),    # Goliathus Chroma Extended
]

# Hardware rainbow for everything we don't animate ourselves (OpenRGB device-name substring -> mode)
HW_TARGETS = [('ASUS ROG STRIX', 'rainbow'), ('GeForce', 'wave'), ('Vengeance', 'rainbow wave')]


def log(msg):
    try:
        with open(LOG_PATH, 'a', encoding='utf-8') as f:
            f.write(time.strftime('%Y-%m-%d %H:%M:%S  ') + msg + '\n')
        if os.path.getsize(LOG_PATH) > 256 * 1024:
            lines = open(LOG_PATH, encoding='utf-8').readlines()[-300:]
            open(LOG_PATH, 'w', encoding='utf-8').writelines(lines)
    except OSError:
        pass


class Config:
    def __init__(self):
        self.mtime = None
        self.v = dict(DEFAULTS)

    def refresh(self):
        try:
            m = os.path.getmtime(CFG_PATH)
            if m != self.mtime:
                with open(CFG_PATH, encoding='utf-8') as f:
                    self.v = {**DEFAULTS, **json.load(f)}
                self.mtime = m
        except (OSError, ValueError):
            pass
        return self.v


def proc_running(image):
    out = subprocess.run(['tasklist', '/FI', f'IMAGENAME eq {image}', '/NH'], capture_output=True, text=True,
                         creationflags=NO_WINDOW).stdout
    return image.lower() in out.lower()


def set_hardware_modes():
    """One-shot OpenRGB CLI run for motherboard / GPU / RAM, then OpenRGB exits."""
    if proc_running('OpenRGB.exe'):
        subprocess.run(['taskkill', '/F', '/IM', 'OpenRGB.exe'], capture_output=True, creationflags=NO_WINDOW)
        time.sleep(2)
    targets = [t for t in HW_TARGETS if not (t[0] == 'Vengeance' and proc_running('iCUE.exe'))]
    args = [OPENRGB_EXE, '--noautoconnect']
    for name, mode in targets:
        args += ['-d', name, '-m', mode]
    try:
        r = subprocess.run(args, capture_output=True, timeout=90, creationflags=NO_WINDOW)
        log(f'hardware modes set ({", ".join(n for n, _ in targets)}), exit={r.returncode}')
    except Exception as e:
        log(f'hardware modes failed: {e}')


# ---- colour -----------------------------------------------------------------------------------------

def rainbow(h, s=1.0, v=1.0):
    """FastLED's 'rainbow' hue map (hsv2rgb_rainbow): tuned for LEDs so yellow/orange get a fair share
    of the wheel and no hue band looks wider or brighter than the others. h in [0,1). Returns 0-255."""
    h8 = (h % 1.0) * 256.0
    section, off = int(h8 // 32) & 7, (h8 % 32) * 8.0          # off: 0..256 within the section
    third, two = off / 3.0, off * 2.0 / 3.0
    if section == 0:   r, g, b = 255 - third, third, 0                  # red -> orange
    elif section == 1: r, g, b = 171, 85 + third, 0                     # orange -> yellow
    elif section == 2: r, g, b = 171 - two, 170 + third, 0              # yellow -> green
    elif section == 3: r, g, b = 0, 255 - third, third                  # green -> aqua
    elif section == 4: r, g, b = 0, 171 - two, 85 + two                 # aqua -> blue
    elif section == 5: r, g, b = third, 0, 255 - third                  # blue -> purple
    elif section == 6: r, g, b = 85 + third, 0, 171 - third             # purple -> pink
    else:              r, g, b = 170 + third, 0, 85 - third             # pink -> red
    if s < 1.0:                                                         # desaturate toward white
        r, g, b = (c * s + 255 * (1 - s) for c in (r, g, b))
    return [max(0, min(255, int(c * v + 0.5))) for c in (r, g, b)]


def make_color_fn(t, cfg, level=1.0):
    a = math.radians(float(cfg['angle_deg']))
    ux, uy = math.cos(a), math.sin(a)
    wl = max(1.0, float(cfg['wavelength']))
    phase = (-1.0 if cfg.get('reverse') else 1.0) * t / max(0.5, float(cfg['period_s']))
    s = max(0.0, min(1.0, float(cfg['saturation'])))
    v = max(0.0, min(1.0, float(cfg['brightness']))) * max(0.0, min(1.0, level))

    def color_at(x, y):
        return rainbow((x * ux + y * uy) / wl - phase, s, v)
    return color_at


# ---- display power state (Windows tells us when it turns the screen off / dims it) ------------------

class _GUID(ctypes.Structure):
    _fields_ = [('Data1', wintypes.DWORD), ('Data2', wintypes.WORD), ('Data3', wintypes.WORD),
                ('Data4', ctypes.c_ubyte * 8)]


GUID_CONSOLE_DISPLAY_STATE = _GUID(0x6FE69556, 0x704A, 0x47A0,
                                   (ctypes.c_ubyte * 8)(0x8F, 0x24, 0xC2, 0x8D, 0x93, 0x6F, 0xDA, 0x47))
WM_POWERBROADCAST, PBT_POWERSETTINGCHANGE = 0x0218, 0x8013
LRESULT = ctypes.c_ssize_t
WNDPROC = ctypes.WINFUNCTYPE(LRESULT, wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM)


class _WNDCLASS(ctypes.Structure):
    _fields_ = [('style', wintypes.UINT), ('lpfnWndProc', WNDPROC), ('cbClsExtra', ctypes.c_int),
                ('cbWndExtra', ctypes.c_int), ('hInstance', wintypes.HINSTANCE), ('hIcon', wintypes.HANDLE),
                ('hCursor', wintypes.HANDLE), ('hbrBackground', wintypes.HANDLE),
                ('lpszMenuName', wintypes.LPCWSTR), ('lpszClassName', wintypes.LPCWSTR)]


class _POWERBROADCAST_SETTING(ctypes.Structure):
    _fields_ = [('PowerSetting', _GUID), ('DataLength', wintypes.DWORD), ('Data', ctypes.c_ubyte * 4)]


def watch_display(state):
    """Message-only window that receives display on/off/dimmed notifications. Sets state['display']
    to 1 (on), 0 (off) or 2 (dimmed). Windows sends the current state right after registering."""
    user32, kernel32 = ctypes.windll.user32, ctypes.windll.kernel32
    user32.DefWindowProcW.restype = LRESULT
    user32.DefWindowProcW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
    user32.CreateWindowExW.restype = wintypes.HWND
    user32.CreateWindowExW.argtypes = [wintypes.DWORD, wintypes.LPCWSTR, wintypes.LPCWSTR, wintypes.DWORD,
                                       ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, wintypes.HWND,
                                       wintypes.HMENU, wintypes.HINSTANCE, wintypes.LPVOID]
    user32.RegisterPowerSettingNotification.restype = wintypes.HANDLE
    user32.RegisterPowerSettingNotification.argtypes = [wintypes.HANDLE, ctypes.POINTER(_GUID), wintypes.DWORD]

    def wndproc(hwnd, msg, wparam, lparam):
        if msg == WM_POWERBROADCAST and wparam == PBT_POWERSETTINGCHANGE and lparam:
            pbs = ctypes.cast(lparam, ctypes.POINTER(_POWERBROADCAST_SETTING)).contents
            if bytes(pbs.PowerSetting) == bytes(GUID_CONSOLE_DISPLAY_STATE):
                new = pbs.Data[0]
                if new != state.get('display'):
                    log(f"display {['off', 'on', 'dimmed'][new] if new < 3 else new}")
                state['display'] = new
            return 1
        return user32.DefWindowProcW(hwnd, msg, wparam, lparam)

    proc = WNDPROC(wndproc)
    state['_wndproc'] = proc                                    # keep the callback alive
    hinst = kernel32.GetModuleHandleW(None)
    wc = _WNDCLASS(lpfnWndProc=proc, hInstance=hinst, lpszClassName='RgbEffectDisplayWatcher')
    if not user32.RegisterClassW(ctypes.byref(wc)):
        log('display watcher: RegisterClass failed'); return
    hwnd = user32.CreateWindowExW(0, wc.lpszClassName, 'rgb-effect', 0, 0, 0, 0, 0,
                                  wintypes.HWND(-3), None, hinst, None)       # HWND_MESSAGE
    if not hwnd or not user32.RegisterPowerSettingNotification(hwnd, ctypes.byref(GUID_CONSOLE_DISPLAY_STATE), 0):
        log('display watcher: could not register for display notifications'); return
    msg = wintypes.MSG()
    while user32.GetMessageW(ctypes.byref(msg), None, 0, 0) > 0:
        user32.TranslateMessage(ctypes.byref(msg))
        user32.DispatchMessageW(ctypes.byref(msg))


# ---- Razer HID ------------------------------------------------------------------------------------

def razer_report(tid, cls, cid, args):
    r = bytearray(91)          # [0] = report id 0, then the 90-byte Razer report
    r[2] = tid
    r[6] = len(args)
    r[7] = cls
    r[8] = cid
    r[9:9 + len(args)] = bytes(args)
    crc = 0
    for b in r[3:89]:
        crc ^= b
    r[89] = crc
    return bytes(r)


class RazerDevice:
    """One Razer device, rendered by its own thread so a slow/absent device can't stall the rest.

    Protocol notes learned the hard way on this hardware:
      * put the device in normal (hardware) mode, NOT Synapse's driver mode, or Fn/media keys and
        mouse buttons stop being handled by the firmware;
      * send the 'custom effect' command ONCE, then stream frame rows. Re-sending it every frame
        makes the BlackWidow freeze on the first frame;
      * mouse/mat rows are fire-and-forget (~4 ms each); the keyboard needs each row's reply read
        back or it silently stops applying frames (and now and then answers 'busy' -> retry).
    """

    def __init__(self, spec, path):
        self.kind, self.pid, _, _, _, self.tid, self.rows, self.cols = spec
        self.path = path
        self.dev = hid.device()
        self.dev.open_path(path)
        if self.kind == 'keyboard':
            self.pos = layout.keyboard_positions()               # None where the matrix slot is empty
        elif self.kind == 'mouse':
            self.pos = [layout.mouse_positions(self.cols)]
        else:
            self.pos = [[layout.MAT_POS] * self.cols]
        self.alive = True
        self.busy = self.bad = 0
        self.last_status = self._ask(razer_report(self.tid, 0x00, 0x04, [0x00, 0x00]))       # normal mode
        if self.last_status == 0x02:
            self._ask(razer_report(self.tid, 0x0F, 0x02, [0x00, 0x00, 0x08] + [0] * 9))      # custom effect, once

    def _ask(self, report):
        self.dev.send_feature_report(report)
        time.sleep(0.002)
        resp = self.dev.get_feature_report(0, 91)
        return resp[1] if len(resp) > 1 else 0

    def send_frame(self, color_at):
        for r in range(self.rows):
            rgb = []
            for p in self.pos[r]:
                rgb += color_at(*p) if p is not None else [0, 0, 0]
            report = razer_report(self.tid, 0x0F, 0x03, [0x00, 0x00, r, 0, self.cols - 1] + rgb)
            if self.kind != 'keyboard':
                self.dev.send_feature_report(report)
                continue
            for _ in range(4):
                self.dev.send_feature_report(report)
                time.sleep(0.001)
                status = self.dev.get_feature_report(0, 91)[1]
                if status != 0x01:
                    break
                self.busy += 1
                time.sleep(0.002)
            if status not in (0x01, 0x02):
                self.bad += 1

    def render_loop(self, state):
        idle_frames = 0
        while self.alive and state['running']:
            start = time.perf_counter()
            dark = state['level'] <= 0.0
            # when faded out, send a few black frames then just idle (the device holds the last frame)
            if not dark or idle_frames < 3:
                try:
                    self.send_frame(state['color_at'])
                except Exception as e:
                    if self.pid != 0x00AB:
                        log(f'lost {self.kind} {self.pid:04X} ({e})')
                    self.alive = False
                    break
            idle_frames = idle_frames + 1 if dark else 0
            fps = state['fps'] if not dark else 4
            time.sleep(max(0.0, 1.0 / fps - (time.perf_counter() - start)))
        self.close()

    def close(self):
        try:
            self.dev.close()
        except Exception:
            pass


def find_razer(open_paths):
    found = []
    for info in hid.enumerate(RAZER_VID):
        for spec in RAZER:
            _, pid, iface, page, usage = spec[:5]
            if (info['product_id'], info['interface_number'], info['usage_page'], info['usage']) == (pid, iface, page, usage) \
                    and info['path'] not in open_paths:
                try:
                    d = RazerDevice(spec, info['path'])
                except Exception as e:
                    log(f'open {spec[0]} {pid:04X} failed: {e}')
                    continue
                if d.last_status == 0x02:
                    found.append(d)
                    log(f'opened {spec[0]} {pid:04X}')
                else:
                    # e.g. the HyperSpeed dongle while the mouse is on its cable (status 4 = no answer);
                    # leave it closed, the next rescan tries again
                    d.close()
    return found


# ---- main loop ----------------------------------------------------------------------------------------

def start(devs, state):
    for d in devs:
        threading.Thread(target=d.render_loop, args=(state,), daemon=True).start()


def run():
    set_hardware_modes()
    cfg = Config()
    c = cfg.refresh()
    state = {'running': True, 'display': 1, 'level': 1.0, 'fps': max(5, min(60, int(c['fps'])))}
    state['color_at'] = make_color_fn(0.0, c)
    threading.Thread(target=watch_display, args=(state,), daemon=True).start()

    devices = find_razer(set())
    start(devices, state)
    t0 = last_scan = last_cfg = last_beat = prev = time.perf_counter()
    while True:
        now = time.perf_counter()
        dt, prev = now - prev, now
        if now - last_cfg > 1.0:
            c, last_cfg = cfg.refresh(), now
            state['fps'] = max(5, min(60, int(c['fps'])))
        # ease brightness toward what the display state calls for (on / dimmed / off)
        target = {0: 0.0, 2: float(c.get('dim_level', 0.35))}.get(state['display'], 1.0)
        step = dt / FADE_S
        lvl = state['level']
        state['level'] = min(target, lvl + step) if lvl < target else max(target, lvl - step)
        # publish the current frame function; each device thread renders it at its own pace
        state['color_at'] = make_color_fn(now - t0, c, state['level'])
        if now - last_scan > RESCAN_S:
            last_scan = now
            devices = [d for d in devices if d.alive]
            new = find_razer({d.path for d in devices})
            start(new, state)
            devices += new
        if now - last_beat > HEARTBEAT_S:
            log('heartbeat: ' + ', '.join(f'{d.kind} {d.pid:04X} busy={d.busy} bad={d.bad}' for d in devices if d.alive)
                + f"; display={state['display']} level={state['level']:.2f}")
            for d in devices:
                d.busy = d.bad = 0
            last_beat = now
        time.sleep(1.0 / state['fps'] if state['level'] > 0 else 0.25)


if __name__ == '__main__':
    log('start (direct HID)')
    try:
        run()
    except Exception as e:
        log(f'crashed: {e!r}')
        raise
