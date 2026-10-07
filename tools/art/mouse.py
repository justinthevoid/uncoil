# Mouse line art for the app from top-down product photos: writes apps/uncoil/src/lib/art/mice/<id>.json.
#
# For each mouse in mice.json (device id -> photo URL), the photo is fetched into a cache folder (never into
# the repository), and:
#   - the silhouette is traced from its alpha channel: the cable and anything bundled beside the mouse
#     (a dongle, a dock) are dropped, the outline smoothed and fitted to Béziers;
#   - the wheel and the logo are found from their lit green, or placed where they usually are;
#   - the main buttons' split and back edge and the two side buttons are placed by proportion;
#   - the outline's run round the sides and back is kept for underglow strips.
# Vendor logos are never traced: the app draws uncoil's spiral at the logo position.
#
#   python tools/art/mouse.py CACHE_DIR [ids...] [--overlay]
#
# --overlay writes CACHE_DIR/<id>-overlay.png (the drawing over the photo) to check the fit. Needs
# opencv-python, numpy and scipy. Hand-drawn seams (like the Basilisk V3 Pro's, basilisk.py) beat these
# proportions; this gets every mouse a real outline first.
import cv2, numpy as np, json, sys, os, urllib.request
from scipy.ndimage import gaussian_filter1d

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, '..', '..', 'apps', 'uncoil', 'src', 'lib', 'art', 'mice')
UA = {'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/130 Safari/537.36'}
f1 = lambda v: round(float(v), 1)


def fetch(url, cache):
    path = os.path.join(cache, url.split('/')[-1])
    if not os.path.exists(path):
        open(path, 'wb').write(urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=30).read())
    return path


def resample(p, step, closed=True):
    q = np.vstack([p, p[:1]]) if closed else p
    d = np.r_[0, np.cumsum(np.hypot(*np.diff(q, axis=0).T))]
    n = max(int(d[-1] / step), 6)
    t = np.linspace(0, d[-1], n, endpoint=not closed)
    return np.c_[np.interp(t, d, q[:, 0]), np.interp(t, d, q[:, 1])]


def curve(P, closed=True):
    """Catmull-Rom through the points, as SVG cubic Béziers."""
    n = len(P)
    out = [f'M{f1(P[0][0])} {f1(P[0][1])}']
    for i in (range(n) if closed else range(n - 1)):
        p0 = P[(i - 1) % n] if (closed or i) else P[0]
        p1, p2 = P[i], P[(i + 1) % n]
        p3 = P[(i + 2) % n] if (closed or i + 2 < n) else P[-1]
        c1 = (p1[0] + (p2[0] - p0[0]) / 6, p1[1] + (p2[1] - p0[1]) / 6)
        c2 = (p2[0] - (p3[0] - p1[0]) / 6, p2[1] - (p3[1] - p1[1]) / 6)
        out.append(f'C{f1(c1[0])} {f1(c1[1])} {f1(c2[0])} {f1(c2[1])} {f1(p2[0])} {f1(p2[1])}')
    return ' '.join(out) + (' Z' if closed else '')


def rrect(x, y, w, h, r):
    r = min(r, w / 2, h / 2)
    return (f'M{f1(x + r)} {f1(y)} H{f1(x + w - r)} Q{f1(x + w)} {f1(y)} {f1(x + w)} {f1(y + r)} V{f1(y + h - r)} '
            f'Q{f1(x + w)} {f1(y + h)} {f1(x + w - r)} {f1(y + h)} H{f1(x + r)} Q{f1(x)} {f1(y + h)} {f1(x)} {f1(y + h - r)} '
            f'V{f1(y + r)} Q{f1(x)} {f1(y)} {f1(x + r)} {f1(y)} Z')


def trace(path):
    img = cv2.imread(path, cv2.IMREAD_UNCHANGED)
    h, w = img.shape[:2]
    alpha = img[..., 3]
    hsv = cv2.cvtColor(img[..., :3], cv2.COLOR_BGR2HSV)
    raw = (alpha >= 250).astype(np.uint8) * 255
    raw = cv2.morphologyEx(raw, cv2.MORPH_CLOSE, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (7, 7)))
    # drop the cable (thin) and bundled items (separate): open wide, keep the largest part, then restore detail
    k = max(5, int(w * 0.035) | 1)
    opened = cv2.morphologyEx(raw, cv2.MORPH_OPEN, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (k, k)))
    n, lab, stats, _ = cv2.connectedComponentsWithStats(opened)
    big = 1 + int(np.argmax(stats[1:, cv2.CC_STAT_AREA]))
    body = (lab == big).astype(np.uint8) * 255
    body = cv2.bitwise_and(raw, cv2.dilate(body, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (k // 2 | 1, k // 2 | 1))))
    cs, _ = cv2.findContours(body, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_NONE)
    outer = max(cs, key=cv2.contourArea).reshape(-1, 2).astype(float)
    bx, by, bw, bh = cv2.boundingRect(outer.astype(np.int32))
    mask = np.zeros((h, w), np.uint8)
    cv2.drawContours(mask, [outer.astype(np.int32).reshape(-1, 1, 2)], -1, 255, -1)

    sil = resample(outer, 2)
    sil = np.c_[gaussian_filter1d(sil[:, 0], 2.2, mode='wrap'), gaussian_filter1d(sil[:, 1], 2.2, mode='wrap')]
    sil = resample(sil, max(4, bh / 70))

    def edges_at(y):
        """Left and right edge of the body on row y."""
        row = np.nonzero(mask[int(np.clip(y, 0, h - 1))])[0]
        return (float(row.min()), float(row.max())) if len(row) else (bx, bx + bw)

    # lit green inside the body: the wheel in the front half, the logo behind
    lit = ((hsv[..., 1] > 110) & (hsv[..., 2] > 110) & (hsv[..., 0] > 35) & (hsv[..., 0] < 95) & (mask > 0)).astype(np.uint8)
    ys, xs = np.nonzero(lit)
    front = ys < by + 0.45 * bh
    back = ys > by + 0.55 * bh
    L = bh
    mid = lambda y: sum(edges_at(y)) / 2
    if front.sum() > 15:
        wx0, wx1, wy0, wy1 = xs[front].min(), xs[front].max(), ys[front].min(), ys[front].max()
        cx = (wx0 + wx1) / 2
        ww = max(wx1 - wx0, 0.06 * bw)
        wheel = (cx - ww / 2, wy0, ww, max(wy1 - wy0, 0.1 * L))
        lit_wheel = True
    else:
        cx = mid(by + 0.18 * L)
        ww = 0.085 * bw
        wheel = (cx - ww / 2, by + 0.1 * L, ww, 0.15 * L)
        lit_wheel = False
    logo = (float(xs[back].mean()), float(ys[back].mean())) if back.sum() > 15 else (mid(by + 0.72 * L), by + 0.72 * L)

    wx, wy, wwid, wht = wheel
    yb = by + max(0.40 * L, (wy + wht - by) + 0.1 * L)  # where the main buttons end
    le, re = edges_at(yb)
    seams = [f'M{f1(cx)} {f1(by)} L{f1(cx)} {f1(wy - 3)}', f'M{f1(cx)} {f1(wy + wht + 3)} L{f1(cx)} {f1(yb + 0.02 * L)}',
             f'M{f1(le - 4)} {f1(yb)} Q{f1(cx)} {f1(yb + 0.06 * L)} {f1(re + 4)} {f1(yb)}']
    big_box = 2 * max(w, h)
    region = {
        'LEFT_CLICK': f'M{-big_box} {-big_box} H{f1(cx - 1)} V{f1(yb + 0.03 * L)} Q{f1((le + cx) / 2)} {f1(yb + 0.03 * L)} {f1(le - 4)} {f1(yb)} H{-big_box} Z',
        'RIGHT_CLICK': f'M{f1(cx + 1)} {-big_box} H{big_box} V{f1(yb)} H{f1(re + 4)} Q{f1((re + cx) / 2)} {f1(yb + 0.03 * L)} {f1(cx + 1)} {f1(yb + 0.03 * L)} Z',
        'WHEEL_CLICK': rrect(wx, wy, wwid, wht, wwid / 2),
        'WHEEL_UP': f'M{f1(cx - 5)} {f1(wy - 5)} L{f1(cx)} {f1(wy - 11)} L{f1(cx + 5)} {f1(wy - 5)} Z',
        'WHEEL_DOWN': f'M{f1(cx - 5)} {f1(wy + wht + 5)} L{f1(cx)} {f1(wy + wht + 11)} L{f1(cx + 5)} {f1(wy + wht + 5)} Z',
        'WHEEL_LEFT': f'M{f1(wx - 5)} {f1(wy + wht / 2 - 6)} L{f1(wx - 11)} {f1(wy + wht / 2)} L{f1(wx - 5)} {f1(wy + wht / 2 + 6)} Z',
        'WHEEL_RIGHT': f'M{f1(wx + wwid + 5)} {f1(wy + wht / 2 - 6)} L{f1(wx + wwid + 11)} {f1(wy + wht / 2)} L{f1(wx + wwid + 5)} {f1(wy + wht / 2 + 6)} Z',
    }
    side = []
    for name, (a, b) in (('FORWARD', (0.30, 0.38)), ('BACK', (0.39, 0.47))):
        y0, y1 = by + a * L, by + b * L
        x = min(edges_at(y0)[0], edges_at(y1)[0])
        pw = max(5.0, 0.045 * bw)
        region[name] = rrect(x - pw * 0.55, y0, pw, y1 - y0, pw / 2)
        side.append((name, (x - pw * 0.55 - 6, (y0 + y1) / 2)))
    tread = ' '.join(f'M{f1(wx + wwid * 0.2)} {f1(y)} H{f1(wx + wwid * 0.8)}' for y in np.arange(wy + wht * 0.12, wy + wht * 0.9, max(3.0, wht / 10)))
    # the outline's run round the sides and back, front left first, for underglow strips
    pts = resample(sil, 2)
    keep = np.nonzero(pts[:, 1] > by + 0.24 * L)[0]
    gap = np.argmax(np.diff(keep)) if len(keep) > 1 and np.diff(keep).max() > 1 else -1
    run = pts[np.r_[keep[gap + 1:], keep[: gap + 1]]] if gap >= 0 else pts[keep]
    if run[0, 0] > bx + bw / 2:
        run = run[::-1]
    run = resample(run, max(3, L / 120), closed=False)
    pad = 0.05 * bh
    return {
        'w': w, 'h': h,
        'view': {'x': f1(bx - pad - 0.12 * bw), 'y': f1(by - pad), 'w': f1(bw + 2 * pad + 0.24 * bw), 'h': f1(bh + 2 * pad)},
        'body_box': {'x': bx, 'y': by, 'w': bw, 'h': bh},
        'body': curve(sil.tolist()),
        'seams': seams,
        'region': region,
        'well': rrect(wx - 3, wy - 2, wwid + 6, wht + 4, 4),
        'tread': tread,
        'lit_wheel': lit_wheel,
        'logo': [f1(logo[0]), f1(logo[1])],
        'strip': [[f1(x), f1(y)] for x, y in run],
        'callouts': {'WHEEL_CLICK': [f1(wx + wwid + 2), f1(wy + wht * 0.4)], **{n: [f1(p[0]), f1(p[1])] for n, p in side}},
    }


def overlay(path, art, out):
    img = cv2.imread(path, cv2.IMREAD_UNCHANGED)
    a = img[..., 3:4] / 255.0
    comp = (img[..., :3] * a + 255 * (1 - a)).astype(np.uint8)
    S = 2
    big = cv2.resize(comp, None, fx=S, fy=S, interpolation=cv2.INTER_CUBIC)
    svg = f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {art["w"]} {art["h"]}" width="{art["w"] * S}" height="{art["h"] * S}">'
    svg += f'<path d="{art["body"]}" fill="none" stroke="#00b8e6" stroke-width="1.2"/>'
    svg += ''.join(f'<path d="{d}" fill="none" stroke="#ff2e8a" stroke-width="1.2"/>' for d in art['seams'])
    svg += ''.join(f'<path d="{d}" fill="none" stroke="#e6a800" stroke-width="1"/>' for k, d in art['region'].items() if k not in ('LEFT_CLICK', 'RIGHT_CLICK'))
    svg += f'<circle cx="{art["logo"][0]}" cy="{art["logo"][1]}" r="9" fill="none" stroke="#e6a800"/></svg>'
    tmp = out + '.svg'
    open(tmp, 'w').write(svg)
    os.system(f'magick -background none -density 96 "{tmp}" "{out}.lines.png"')
    cv2.imwrite(out, big)
    os.system(f'magick "{out}" "{out}.lines.png" -composite "{out}"')


def main():
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    cache, ids = args[0], args[1:]
    manifest = json.load(open(os.path.join(HERE, 'mice.json')))
    os.makedirs(OUT, exist_ok=True)
    for id_, src in manifest.items():
        if ids and id_ not in ids:
            continue
        path = fetch(src['photo'], cache)
        art = trace(path)
        art = {'source': src['photo'], **({'note': src['note']} if 'note' in src else {}), **art}
        json.dump(art, open(os.path.join(OUT, f'{id_}.json'), 'w', newline='\n'), indent=1)
        if '--overlay' in sys.argv:
            overlay(path, art, os.path.join(cache, f'{id_}-overlay.png'))
        print(id_, 'body', art['body_box'], 'lit wheel' if art['lit_wheel'] else 'wheel placed')


if __name__ == '__main__':
    main()
