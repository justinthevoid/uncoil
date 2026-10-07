# Basilisk V3 Pro line art for the app (apps/uncoil/src/lib/art/basilisk.ts).
#
# The silhouette is traced from Razer's top-down product photo: its alpha mask, smoothed and fitted to
# Béziers. The panel seams and buttons are redrawn over it as clean curves, at coordinates read off a 10 px
# grid laid over the same photo. The photo itself is not in the repository; fetch it and pass its path:
#
#   https://assets2.razerzone.com/images/pnx.assets/62dd52710c7316e57f1107a6d8a0a14d/razer-basilisk-v3-pro.webp
#   python tools/art/basilisk.py razer-basilisk-v3-pro.webp [overlay.svg]
#
# Needs opencv-python, numpy and scipy. The optional second argument writes the drawing over the photo, to
# check the fit by eye. The logo is never traced: uncoil's spiral is drawn at the logo LED instead.
import cv2, numpy as np, json, sys, os
from scipy.ndimage import gaussian_filter1d

PHOTO = sys.argv[1]
OVERLAY = sys.argv[2] if len(sys.argv) > 2 else None
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', 'apps', 'uncoil', 'src', 'lib', 'art', 'basilisk.ts')
img = cv2.imread(PHOTO, cv2.IMREAD_UNCHANGED)
h, w = img.shape[:2]
body = cv2.morphologyEx((img[..., 3] >= 250).astype(np.uint8) * 255, cv2.MORPH_CLOSE, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (9, 9)))
cnts, _ = cv2.findContours(body, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_NONE)
outer = max(cnts, key=cv2.contourArea).reshape(-1, 2).astype(float)
bx, by, bw, bh = cv2.boundingRect(outer.astype(np.int32))

def resample(p, step, closed=True):
    q = np.vstack([p, p[:1]]) if closed else p
    d = np.r_[0, np.cumsum(np.hypot(*np.diff(q, axis=0).T))]
    n = max(int(d[-1] / step), 6)
    t = np.linspace(0, d[-1], n, endpoint=not closed)
    return np.c_[np.interp(t, d, q[:, 0]), np.interp(t, d, q[:, 1])]
f = lambda v: f'{v:.1f}'
def cr(P, closed=True):
    n = len(P); out = [f'M{f(P[0][0])} {f(P[0][1])}']
    for i in (range(n) if closed else range(n - 1)):
        p0 = P[(i - 1) % n] if (closed or i) else P[0]; p1 = P[i]; p2 = P[(i + 1) % n]
        p3 = P[(i + 2) % n] if (closed or i + 2 < n) else P[-1]
        c1 = (p1[0] + (p2[0] - p0[0]) / 6, p1[1] + (p2[1] - p0[1]) / 6); c2 = (p2[0] - (p3[0] - p1[0]) / 6, p2[1] - (p3[1] - p1[1]) / 6)
        out.append(f'C{f(c1[0])} {f(c1[1])} {f(c2[0])} {f(c2[1])} {f(p2[0])} {f(p2[1])}')
    return ' '.join(out) + (' Z' if closed else '')

sil = resample(outer, 2)
sil = np.c_[gaussian_filter1d(sil[:, 0], 2.4, mode='wrap'), gaussian_filter1d(sil[:, 1], 2.4, mode='wrap')]
sil = resample(sil, 7)
BODY = cr(sil.tolist())

# seams, in photo pixels (read off a 10 px grid over the photo)
SEAMS = {
    'channel_left': 'M218 140 L220 389',
    'channel_right': 'M260 131 L260 359',
    'channel_front': 'M221 158 Q240 156 259 160',
    'button_back_right': 'M357 292 L260 360',
    'button_back_left': 'M220 390 L137 432',
    'palm_front': 'M154 449 Q236 412 323 377',
    'left_button_side': 'M134 200 Q129 300 133 430',
    'right_button_side': 'M346 190 Q352 240 356 292',
    'left_grip': 'M84 336 L118 318 Q124 395 140 470 Q146 515 151 552',
    'right_grip': 'M330 313 L358 300 M330 313 Q325 400 333 480 Q337 525 344 555',
    'palm_left': 'M154 449 Q146 505 154 562',
    'palm_right': 'M323 377 Q328 470 337 562',
}
GRIPS = [
    'M0 300 L84 336 L118 318 Q124 395 140 470 Q146 515 151 552 L151 700 L0 700 Z',
    'M448 288 L358 300 L330 313 Q325 400 333 480 Q337 525 344 555 L344 700 L448 700 Z',
]
REGION = {
    'LEFT_CLICK': 'M0 0 H219 V389 L137 432 H0 Z',
    'RIGHT_CLICK': 'M261 0 H448 V291 H357 L261 359 Z',
    'WHEEL_CLICK': 'M233 212 H248 Q253 212 253 217 V298 Q253 303 248 303 H233 Q228 303 228 298 V217 Q228 212 233 212 Z',
    'WHEEL_UP': 'M233 201 L240 193 L247 201 Z',
    'WHEEL_DOWN': 'M235 306 L240 312 L245 306 Z',
    'WHEEL_LEFT': 'M212 240 L204 247 L212 254 Z',
    'WHEEL_RIGHT': 'M268 240 L276 247 L268 254 Z',
    'SCROLL_MODE': 'M233 317 H249 Q253 317 253 321 V338 Q253 342 249 342 H233 Q229 342 229 338 V321 Q229 317 233 317 Z',
    'DPI_BUTTON': 'M233 348 H249 Q253 348 253 352 V372 Q253 376 249 376 H233 Q229 376 229 372 V352 Q229 348 233 348 Z',
    'FORWARD': 'M110 226 Q110 222 114 222 H123 V262 H114 Q110 262 110 258 Z',
    'BACK': 'M108 268 Q108 264 112 264 H121 V304 H112 Q108 304 108 300 Z',
    'CLUTCH': 'M84 336 L104 325 L110 337 L91 349 Q85 349 84 343 Z',
}
WELL = 'M226 208 H255 Q260 208 260 213 V300 Q260 305 255 305 H226 Q221 305 221 300 V213 Q221 208 226 208 Z'
TREAD = ' '.join(f'M231 {y} H250' for y in range(218, 300, 7))
LOGO = (234.0, 540.0)

# the engine's LED points (device file, key units from the mouse centre) in photo pixels
K = bh / 6.4
CX, CY = bx + bw / 2, by + bh / 2
STRIP = [(-1.4, -2.6), (-1.4, -1.25), (-1.4, 0.1), (-1.4, 1.45), (-1.4, 3.0), (0, 3.0), (1.4, 2.8), (1.4, 1.45), (1.4, 0.1), (1.4, -1.25), (1.4, -2.6)]
leds = np.array([(CX + x * K * 1.35, CY + y * K) for x, y in STRIP])  # widened: the strip runs round the real outline
# the underglow runs round the lower edge: the outline below the front buttons, front-left to front-right
pts = resample(sil, 3)
keep = pts[:, 1] > by + 0.24 * bh
# rotate so the run starts at the first kept point after a gap
idx = np.nonzero(keep)[0]
gap = np.argmax(np.diff(idx)) if len(idx) > 1 and np.diff(idx).max() > 1 else -1
order = np.r_[idx[gap + 1:], idx[: gap + 1]] if gap >= 0 else idx
run = pts[order]
# 11 equal runs in the device file's order: Strip 1 at the front left, down the left, across the back, up the right
if run[0, 0] > CX:
    run = run[::-1]
d = np.r_[0, np.cumsum(np.hypot(*np.diff(run, axis=0).T))]
segs = []
for i in range(11):
    sel = run[(d >= d[-1] * i / 11 - 1e-6) & (d <= d[-1] * (i + 1) / 11 + 1e-6)]
    segs.append({'d': 'M' + ' L'.join(f'{f(x)} {f(y)}' for x, y in sel), 'led': i})
seams_js = json.dumps(list(SEAMS.values()), indent=1)
region_js = json.dumps(REGION, indent=1)
grips_js = json.dumps(GRIPS)
ts = f'''// The Basilisk V3 Pro seen from above. The silhouette is traced from Razer's top-down product photo (alpha
// mask, smoothed and fitted to Béziers by tools/art/basilisk.py); the panel seams are redrawn over it as
// clean lines, and uncoil's spiral sits where the logo LED is. Units: the photo's pixels, front at the top.
// Generated: edit tools/art/basilisk.py, not this file.

export type Pt = [number, number];

/** The drawing's box (the body plus a small margin) and the body's own bounds. */
export const VIEW = {{ x: {bx - 14}, y: {by - 10}, w: {bw + 28}, h: {bh + 20} }};
export const BODY_BOX = {{ x: {bx}, y: {by}, w: {bw}, h: {bh} }};

export const BODY = '{BODY}';

/** Panel seams. */
export const SEAMS: string[] = {seams_js};

/** Button regions (clip the main buttons to BODY when drawing them). */
export const REGION = {region_js} as const;

/** The side grips (clip to BODY): textured rubber. */
export const GRIPS: string[] = {grips_js};

export const WELL = '{WELL}';
export const TREAD = '{TREAD}';

/** Where the logo LED is: uncoil's spiral is drawn here. */
export const LOGO: Pt = [{LOGO[0]}, {LOGO[1]}];

/** The underglow strip as segments round the outline, each lit by strip LED `led` ("Strip led+1"). */
export const STRIP_SEGMENTS: {{ d: string; led: number }}[] = {json.dumps(segs)};
'''
open(OUT, 'w', encoding='utf-8', newline=chr(10)).write(ts)

# overlay check: drawing over the photo
over = f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="60 110 330 530" width="660" height="1060"><image href="{os.path.abspath(PHOTO)}" width="{w}" height="{h}"/>'
over += f'<path d="{BODY}" fill="none" stroke="#00e0ff" stroke-width="1.2"/>'
for d in SEAMS.values():
    over += f'<path d="{d}" fill="none" stroke="#ff3b9a" stroke-width="1.2"/>'
for d in list(REGION.values())[2:]:
    over += f'<path d="{d}" fill="none" stroke="#ffd400" stroke-width="1"/>'
over += f'<path d="{WELL}" fill="none" stroke="#ffd400" stroke-width="1"/><circle cx="{LOGO[0]}" cy="{LOGO[1]}" r="18" fill="none" stroke="#ffd400"/></svg>'
if OVERLAY:
    open(OVERLAY, 'w').write(over)
print('bbox', bx, by, bw, bh, 'strip segments', len(segs), 'body points', len(sil))
