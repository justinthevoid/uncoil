# Keyboard cases for the app from top-down product photos: writes apps/uncoil/src/lib/art/keyboards/<id>.json.
#
# The keys come from the device file's layout; the photo gives what is round them. For each keyboard in
# keyboards.json (device id -> photo URL) the photo is fetched into a cache folder (never into the
# repository), and:
#   - the keyboard is the opaque part of the photo, cut off where an attached wrist rest begins (the step
#     from the case's shadowed bottom edge to the rest's lit top edge);
#   - the lit keycaps give the key block, whose width matched to the layout's gives the key pitch (keys are
#     square; the block's depth is blurred by edge glow and dial LEDs);
#   - the case and a front lip are written in key units from the first key's top-left corner.
#
#   python tools/art/keyboard.py CACHE_DIR [ids...] [--overlay]
#
# --overlay writes CACHE_DIR/<id>-overlay.png: the layout's keys and the measured case over the photo, to check
# both the fit and the device file's rows. Needs opencv-python and numpy.
import cv2, numpy as np, json, sys, os, re, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, '..', '..')
OUT = os.path.join(ROOT, 'apps', 'uncoil', 'src', 'lib', 'art', 'keyboards')
UA = {'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/130 Safari/537.36'}
INSET = 0.12  # a lit keycap starts about this far inside its key unit
r2 = lambda v: round(float(v), 2)


def fetch(url, cache):
    path = os.path.join(cache, url.split('/')[-1])
    if not os.path.exists(path):
        open(path, 'wb').write(urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=30).read())
    return path


def layout(id_):
    """The device file's key rectangles (x, y, w, h) in key units. Rows are read directly: each row's keys
    array is JSON as well as TOML, which spares a TOML parser that trips over key names like "\\" and ","."""
    for d in ('devices', os.path.join('devices', 'experimental')):
        p = os.path.join(ROOT, d, f'{id_}.toml')
        if not os.path.exists(p):
            continue
        text = open(p, encoding='utf-8').read()
        keys = []
        for block in re.split(r'^\[\[layout\.rows\]\]\s*$', text, flags=re.M)[1:]:
            block = re.split(r'^\[', block, flags=re.M)[0]
            y = float(re.search(r'^y\s*=\s*([-\d.]+)', block, re.M).group(1))
            mx = re.search(r'^x\s*=\s*([-\d.]+)', block, re.M)
            x = float(mx.group(1)) if mx else 0.0
            arr = json.loads(re.search(r'^keys\s*=\s*(\[.*?\])\s*$', block, re.M | re.S).group(1))
            for k in arr:
                name, wd = k.rsplit(':', 1)
                # LED strips laid out as rows (a wrist rest's) are not keys
                if name != 'gap' and not re.match(r'^(WR|LU|RU)\d+$', name):
                    keys.append((x, y, float(wd), 1.0))
                x += float(wd)
        return keys
    raise SystemExit(f'no device file for {id_}')


def measure(path, keys, rest=False):
    img = cv2.imread(path, cv2.IMREAD_UNCHANGED)
    h, w = img.shape[:2]
    alpha = img[..., 3]
    hsv = cv2.cvtColor(img[..., :3], cv2.COLOR_BGR2HSV)
    mask = cv2.morphologyEx((alpha >= 250).astype(np.uint8), cv2.MORPH_CLOSE, np.ones((5, 5), np.uint8))
    bright = (mask > 0) & (hsv[..., 2] > 110) & ((hsv[..., 1] > 50) | (hsv[..., 2] > 190))
    # edge-lit case sides are not keys: ignore a margin inside the opaque outline
    cols = np.nonzero(mask.mean(axis=0) > 0.1)[0]
    rows = np.nonzero(mask.mean(axis=1) > 0.1)[0]
    edge = int(0.015 * (cols.max() - cols.min()))
    bright[:, : cols.min() + edge] = False
    bright[:, cols.max() - edge:] = False
    bright[: rows.min() + edge] = False
    # the key block: across, the lit pixels' extent; down, the rows dense with lit keys (a dial's glow above
    # the F-row is not one)
    ys, xs = np.nonzero(bright)
    kx0, kx1 = np.percentile(xs, [0.3, 99.7])
    rc = bright.sum(axis=1)
    rr = np.nonzero(rc > 0.3 * rc.max())[0]
    ky0, ky1 = float(rr.min()), float(rr.max())
    W = max(x + kw for x, _, kw, _ in keys)
    H = max(y + kh for _, y, _, kh in keys)
    # keys are square: the pitch comes from the key block's width (edge glow and dial LEDs blur its depth),
    # anchored at the top row
    px = py = (kx1 - kx0) / (W - 2 * INSET)
    ox, oy = kx0 - INSET * px, ky0 - INSET * py
    keys_bottom = oy + H * py
    # the case: opaque rows down from the keys until the gap above a wrist rest (raw alpha: the gap is thin)
    opaque = (alpha >= 250)[:, int(kx0):int(kx1)].mean(axis=1)
    bottom = int(keys_bottom)
    while bottom + 1 < h and opaque[bottom + 1] > 0.9:
        bottom += 1
    # an attached wrist rest touches the case: its top edge catches the light right after the case's shadowed
    # bottom edge, so the join is the strongest step up in brightness below the keys with a long run after it
    gray = cv2.cvtColor(img[..., :3], cv2.COLOR_BGR2GRAY).astype(float)[:, int(kx0):int(kx1)].mean(axis=1)
    best, step = None, 20.0
    for y in range(int(keys_bottom), min(bottom - 3, int(keys_bottom + 2.5 * py))):
        d = gray[y + 1: y + 4].mean() - gray[max(0, y - 2): y + 1].mean()
        if d > step and bottom - y > 1.5 * py:
            best, step = y, d
    rest_box = None
    if rest:
        # a wrist rest with its own light: the join is the darkest row just below the keys
        end = bottom
        lo, hi = int(keys_bottom), min(end - 3, int(keys_bottom + 1.5 * py))
        best = lo + int(np.argmin(gray[lo:hi])) if hi > lo else None
        if best is not None:
            rows_r = (alpha >= 250)[best + 1: end + 1]
            xs_r = np.nonzero(rows_r.mean(axis=0) > 0.5)[0]
            rest_box = [(xs_r.min() - ox) / px, (best + 1 - oy) / py, (xs_r.max() + 1 - ox) / px, (end + 1 - oy) / py]
    if best is not None:
        bottom = best
    top = int(ky0)
    while top - 1 >= 0 and opaque[top - 1] > 0.9:
        top -= 1
    band = mask[top: bottom + 1]
    xs_case = np.nonzero(band.sum(axis=0) > (bottom - top) * 0.5)[0]
    cx0, cx1 = xs_case.min(), xs_case.max() + 1
    case = [(cx0 - ox) / px, (top - oy) / py, (cx1 - ox) / px, (bottom + 1 - oy) / py]
    art = {'case': [r2(v) for v in case], 'pitch': [r2(px), r2(py)]}
    if rest_box:
        art['rest'] = [r2(v) for v in rest_box]
    if case[3] - H > 1.0:
        art['lip'] = r2(H + 0.3)
    return art, (ox, oy, px, py, top, bottom, cx0, cx1)


def overlay(path, keys, geom, out):
    ox, oy, px, py, top, bottom, cx0, cx1 = geom
    img = cv2.imread(path, cv2.IMREAD_UNCHANGED)
    a = img[..., 3:4] / 255.0
    comp = (img[..., :3] * a + 255 * (1 - a)).astype(np.uint8)
    S = 2
    big = cv2.resize(comp, None, fx=S, fy=S, interpolation=cv2.INTER_CUBIC)
    for x, y, kw, kh in keys:
        p0 = (int((ox + (x + 0.06) * px) * S), int((oy + (y + 0.06) * py) * S))
        p1 = (int((ox + (x + kw - 0.06) * px) * S), int((oy + (y + kh - 0.06) * py) * S))
        cv2.rectangle(big, p0, p1, (255, 200, 0), 1)
    cv2.rectangle(big, (int(cx0 * S), int(top * S)), (int(cx1 * S), int(bottom * S)), (60, 60, 255), 2)
    cv2.imwrite(out, big)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    cache, ids = args[0], args[1:]
    manifest = json.load(open(os.path.join(HERE, 'keyboards.json')))
    os.makedirs(OUT, exist_ok=True)
    for id_, src in manifest.items():
        if ids and id_ not in ids:
            continue
        keys = layout(id_)
        path = fetch(src['photo'], cache)
        art, geom = measure(path, keys, src.get('rest', False))
        art = {'source': src['photo'], **({'note': src['note']} if 'note' in src else {}), **art}
        json.dump(art, open(os.path.join(OUT, f'{id_}.json'), 'w', newline='\n'), indent=1)
        if '--overlay' in sys.argv:
            overlay(path, keys, geom, os.path.join(cache, f'{id_}-overlay.png'))
        flag = ''
        print(f'{id_}: case {art["case"]} lip {art.get("lip")} pitch {art["pitch"]}{flag}')


if __name__ == '__main__':
    main()
