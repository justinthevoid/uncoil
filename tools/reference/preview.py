"""Render what the effect looks like on the desk (keyboard keys + underglow, mouse LEDs, mat edge)
to an animated GIF, using the exact layout and colour code the engine uses.

    python preview.py [out.gif] [seconds]     (default: preview.gif, one full colour period)
"""
import sys, os
from PIL import Image, ImageDraw

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import rgb_layout as L
from rgb_effect import make_color_fn, Config

S = 44                                   # pixels per key unit
PAD = 0.6
X0, Y0, W, H = L.MAT_RECT
OX, OY = -X0 + PAD, -Y0 + PAD
IMG_W, IMG_H = int((W + 2 * PAD) * S), int((H + 2 * PAD) * S)


def px(x, y):
    return (x + OX) * S, (y + OY) * S


def frame(t, cfg):
    col = make_color_fn(t, cfg)
    im = Image.new('RGB', (IMG_W, IMG_H), (14, 14, 18))
    d = ImageDraw.Draw(im)
    # mat: dark cloth with its single-colour edge strip
    mx0, my0 = px(X0, Y0); mx1, my1 = px(X0 + W, Y0 + H)
    d.rounded_rectangle([mx0, my0, mx1, my1], radius=18, fill=(26, 26, 30), outline=tuple(col(*L.MAT_POS)), width=6)
    # keyboard case
    kx0, ky0 = px(-0.3, -0.3); kx1, ky1 = px(L.KB_WIDTH + 0.3, L.KB_DEPTH + 0.3)
    d.rounded_rectangle([kx0, ky0, kx1, ky1], radius=10, fill=(32, 32, 36))
    # underglow LEDs (drawn as glows along the sides)
    for row in L.KB_MATRIX:
        for name in row:
            if name and name[:2] in ('LU', 'RU'):
                x, y = L.led_position(name)
                cx, cy = px(x, y)
                d.ellipse([cx - 9, cy - 14, cx + 9, cy + 14], fill=tuple(col(x, y)))
    # keys
    for name, (x, y, w, h) in L.KEY_RECTS.items():
        a = px(x + 0.06, y + 0.06); b = px(x + w - 0.06, y + h - 0.06)
        cx, cy = x + w / 2, y + h / 2
        d.rounded_rectangle([a, b], radius=6, fill=tuple(col(cx, cy)))
    # mouse body + LEDs
    hw, hl = L.MOUSE_HALF_W, L.MOUSE_HALF_L
    a = px(L.MOUSE_CX - hw, L.MOUSE_CY - hl); b = px(L.MOUSE_CX + hw, L.MOUSE_CY + hl)
    d.rounded_ellipse = None
    d.ellipse([a, b], fill=(32, 32, 36))
    for i, (x, y) in enumerate(L.mouse_positions()):
        cx, cy = px(x, y)
        r = 12 if i < 2 else 8
        d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=tuple(col(x, y)))
    return im


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(os.path.abspath(__file__)), 'preview.gif')
    cfg = Config().refresh()
    secs = float(sys.argv[2]) if len(sys.argv) > 2 else float(cfg['period_s'])
    fps = 12
    frames = [frame(i / fps, cfg).resize((IMG_W // 2, IMG_H // 2), Image.LANCZOS) for i in range(int(secs * fps))]
    frames[0].save(out, save_all=True, append_images=frames[1:], duration=int(1000 / fps), loop=0, optimize=True)
    frame(0, cfg).save(os.path.splitext(out)[0] + '.png')
    print(out)


if __name__ == '__main__':
    main()
