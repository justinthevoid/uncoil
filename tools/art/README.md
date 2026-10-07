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

A new mouse is a copy of the script with its own photo, seams and regions, and an entry in `DIAGRAMS`
(`KeysView.svelte`) and `DeskPreview.svelte`.

## Keyboards: `apps/uncoil/src/lib/art/keyboards.ts`

The keys already come from the device file's layout. The photo gives what is round them, measured in key
units from the first key's top-left corner (find the pixel pitch from a row of 1u keys): the case, its front
lip, a screen, a side dial and side buttons, plus key groups that have their own frame. Check the device
file's rows against the photo while you are there: the BlackWidow V4 Pro 75%'s F-row turned out to have no
gaps.

## Sources used

| Device | Photo |
|---|---|
| Basilisk V3 Pro | `assets2.razerzone.com/images/pnx.assets/62dd52710c7316e57f1107a6d8a0a14d/razer-basilisk-v3-pro.webp` |
| BlackWidow V4 Pro 75% | `assets2.razerzone.com/pages/blackwidow-v4-pro-75/full-keyboard-unlit.jpg` |
