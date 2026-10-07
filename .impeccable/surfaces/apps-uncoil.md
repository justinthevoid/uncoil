---
version: 1
slug: "apps-uncoil"
primary_target: "apps/uncoil"
related_targets: ["site"]
---

# Surface brief: uncoil desktop app (apps/uncoil)

Scope: the Tauri desktop app. Mode: **Operate**. The site and docs share palette and type later.

Audience and job: Razer owners on Windows, enthusiasts first, opening the app occasionally to set the desk
effect, remap a key, change the dial or screen, check devices are connected, and close it again. Proof on
hand: measured engine footprint; live device state; real effect colours; real Fn-layer data.
Constraints: device-first layout (Wooting / G Hub) with Linear/Raycast cleanliness; light and dark follow
Windows; no wide display face; no red accent; default window sized to the monitor (1440x900 on a 1440p
screen, min 1024x680); standard controls; readable state without colour; reduced motion.

## History

v1 "Factory Catalog Sleeve" (Saville, FAC numbers) rejected in use 2026-10-03. v2 "Dark Desk" (near-black,
red accent, wide Archivo, glow) rejected the same day as futuristic. v3 chosen from a decision round of
code-rendered Keys screens: the Swatch Book.

## Direction contract

THESIS: uncoil files every change like a lighting-gel swatch book: each kind of change carries a named colour
tab, on the device and in the list, so you can read what you changed at a glance. It refuses the gamer
dashboard (neon on black) and the grey settings panel with one accent.

OWN-WORLD: swatch-book pages: warm graphite (dark) or warm-white (light) grounds, warm grey ink, no brand
accent; colour appears only as named gel swatches with jobs (your change green, media and macros amber,
lighting magenta, system blue) and as the devices' real LED colour. Segoe UI Variable, sentence case, 8px
radii, hairline borders, flat surfaces.

STORY: open the app, pick the desk or a device along the top, see it large in the centre, change one thing
in the panel beside it, save it with one confirmation, and read back what the device now holds.

FIRST VIEWPORT: top bar with Desk plus one tab per device (status dot), engine status and settings at the
right; left rail with the selected tab's features; centre stage with the device drawn large (Desk: the
lit desk and the effect chosen from swatch cards; Keys: keycaps with gel tabs and a compact "What Fn
changes" list); right panel for the selected thing and its Save action.

FORM: a lighting-gel swatch book (Rosco/Lee style), chosen from seven candidate directions. Two ideas from
the others stayed: a compact, dense "What Fn changes" list, and saves that show the device's read-back,
like a stamped ticket.

FINISH: a change isn't finished until it has been reviewed and documented: a design review, its verdict,
DESIGN.md kept current, and every shipped image traceable to where it came from.
