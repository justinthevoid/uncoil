---
version: 2
slug: "apps-uncoil"
primary_target: "apps/uncoil"
related_targets: ["site"]
---

# Surface brief: uncoil desktop app (apps/uncoil)

Scope: the Tauri desktop app (Lighting, Keys, Dial & screen, Devices, Settings, About). Mode: **Operate**.
The site (Persuade) and docs (Read) share the palette and type.

Audience and job: Razer owners on Windows, enthusiasts first, opening the app occasionally to set the
effect, remap a key, check that devices are connected, and close it again. Proof on hand: measured engine
footprint; live device state; the real effect colours.
Constraints: standard controls and navigation; plain words; readable state without colour; reduced motion.

## History

Version 1 used a "Factory Catalog Sleeve" direction (Peter Saville / Factory Records: catalog numbers on
everything, hairline line plots, engraved caps). Justin chose it on 2026-10-02 from comps, then rejected it
in use on 2026-10-03: the FAC numbers meant nothing to users, and the outline keyboard was hard to read.
He chose "keep dark, drop gimmicks" and "solid keycaps, big legends". Don't bring the catalog devices back.

## Direction contract

THESIS: a dark, quiet control panel next to a desk full of light. The only colour that matters is the
devices' real colour, shown on a to-scale picture of the desk; everything else is plain and readable.

OWN-WORLD: near-black ground #0b0b0b with raised dark cards, four grey text steps, one red accent #e2372c
for selection and "on", green and yellow for status only. Archivo, sentence case, small radii (4/6/10px),
lucide icons. Solid keycaps and lit LEDs are the only places with physical depth.

STORY: open the app, see your desk lit exactly as it is, change the effect with ordinary controls, click a
key to change what it does, save it to the device with one confirmation, close the app.

FIRST VIEWPORT: icon sidebar (Lighting, Keys, Dial & screen, Devices, Settings, About; engine status at the
foot). Lighting: page title and one-line description, the desk preview lit live, connected-device chips,
then a card with the effect picker, its settings, and brightness/saturation.

Signature move: the desk drawn to scale and lit with the live effect; on Keys, the real keyboard as solid
keycaps with Fn actions written on them.

FINISH: reviewed against DESIGN.md (rewritten 2026-10-03), detector clean, screenshots in .impeccable/review/.
