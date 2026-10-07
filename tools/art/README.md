# Device drawings

The app draws each device from above as flat line art of the real product. The source is the vendor's own
top-down product photo. Nothing from the photo ships except geometry derived from it, and vendor logos are
never traced: uncoil's spiral or wordmark goes where a logo is.

## Mice: `basilisk.py`

1. Find a straight top-down photo with a transparent background on the product page. Razer's sit under
   `assets2.razerzone.com/images/pnx.assets/<hash>/<product>.webp`; the page's HTML lists them.
2. The silhouette is traced from the alpha channel: closed, the largest contour, smoothed and fitted to
   Béziers.
3. Lay a 10 px grid over a contrast-boosted copy of the photo and read the seams off it: the button split,
   where the buttons end, the wheel well, buttons, grips. Write them into `SEAMS`, `REGION` and `GRIPS` as
   clean curves. Automatic edge and tone tracing were tried and rejected: on glossy black plastic they trace
   reflections, not parts.
4. Run with an overlay and check the drawing against the photo:
   `python tools/art/basilisk.py photo.webp overlay.svg`
5. The script writes `apps/uncoil/src/lib/art/basilisk.ts`. LED colours map by the device file's shape
   names; the underglow strip is split into equal runs round the outline in the device file's order.

That is the hand-drawn route, worth it for a mouse people will look at closely.

## Every other mouse: `mouse.py` and `mice.json`

`mice.json` maps a device id to its store photo (Razer's product pages carry a 500 × 500 top-down PNG with
a transparent background in their store gallery). `python tools/art/mouse.py CACHE_DIR --overlay` traces each
silhouette, drops the cable and anything bundled beside the mouse, finds a lit wheel and logo (or places them
where they usually are), places the button split, the button back edge and the side buttons by proportion,
and writes `apps/uncoil/src/lib/art/mice/<id>.json`. The app picks those up for the Buttons page and the
desk. Check each overlay: the outline is traced, the seams are only proportions.

## Keyboards: `keyboard.py`, `keyboards.json` and `apps/uncoil/src/lib/art/keyboards.ts`

The keys already come from the device file's layout. The photo gives what is round them, in key units from
the first key's top-left corner. `python tools/art/keyboard.py CACHE_DIR --overlay` does it for every
keyboard in `keyboards.json`: the lit key block's width against the layout's gives the pitch (keys are
square), the case is the opaque board cut off where an attached wrist rest begins, and a deep case gets a
front lip. It writes `apps/uncoil/src/lib/art/keyboards/<id>.json`.

The overlay draws the layout's keys on the photo, which makes it a check on the device file too. The
BlackWidow V4 Pro 75%'s F-row turned out to have no gaps; the BlackWidow V4 Pro's experimental layout (extra
row, macro column) does not match its photo yet, so it is left out until `tools/devices/overrides.json`
corrects it. Screens, dials and side buttons are measured by hand (`KEYBOARD_ART` in `keyboards.ts`).

## Sources used

| Device | Photo |
|---|---|
| Basilisk V3 Pro | `assets2.razerzone.com/images/pnx.assets/62dd52710c7316e57f1107a6d8a0a14d/razer-basilisk-v3-pro.webp` |
| BlackWidow V4 Pro 75% | `assets2.razerzone.com/pages/blackwidow-v4-pro-75/full-keyboard-unlit.jpg` |
| Other mice and keyboards | `mice.json`, `keyboards.json` |

No photo found yet: DeathAdder V2, Naga V2 Pro (its page now shows the V3 Pro), BlackWidow V3 Tenkeyless,
Huntsman Mini, Huntsman V2 and V2 Tenkeyless. The Ornata V3 has no key layout in its device file.
