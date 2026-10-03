---
version: 1
slug: "apps-uncoil"
primary_target: "apps/uncoil"
related_targets: []
---

# Surface brief: uncoil desktop app (apps/uncoil)

Scope: the Tauri desktop app (Lighting, Devices, Display, About). Mode: **Operate**. The same world is
the shared design system for the Astro site (Persuade) and Starlight docs (Read), which get their own briefs.

Audience and job: Razer owners on Windows, enthusiasts first, opening the app occasionally to set the
effect, check that devices are live, and close it again. Proof on hand: measured daemon footprint
(3.0 MB RAM, 0.8% of one core, 651 KB executable); live device state; the real effect.
Constraints: standard controls and navigation (Operate); readable state without colour; reduced motion.

## Direction contract

THESIS: uncoil is catalogued like a Factory Records release: matte black, one white hairline plot owning
the field, every artifact given a catalog number, colour used as code. It refuses the gamer-RGB dashboard
(neon glow on dark cards) and the generic settings panel.

OWN-WORLD: matte black ground #0b0b0b, plot-white ink #f2f2f2, industrial greys for seams and secondary
ink, and the FAC code hues (grey, blue, yellow, red, white) used only as code: catalog strips, state, and
the red active block. Zero radius, one-pixel hairlines, wide-tracked engraved caps for labels, a wide
geometric grotesk for display numerals. Live LED colour appears only where something is decoded.

STORY: the visitor sees their desk's effect as a pulse plot, reads its exact parameters beside it, knows
every device is live (or away) from its coded strip, adjusts, and leaves; the app's lightness is stated
as data, never as hype.

FIRST VIEWPORT: left catalog rail (wordmark; FAC 01 Lighting, 02 Devices, 03 Display, 04 About, active
item boxed with a red catalog number; engine readout pinned at the bottom: uncoild running, memory, cpu,
size). Main: the plot stage across the top two-thirds, a stack of white hairline lines where each line is
a slice of the desk and ridges are the wave's bands (angle, width and speed visible), amplitude only where
LEDs exist, one slice decoded in live colour under the pointer; a "transmission data" column to its right
with the effect's parameters. Below: effect catalog entries (FAC 101 Wave, 102 Spectrum, 103 Static,
104 Off) and plain sliders; devices as rows with coded hue strips of their live colours, away rows greyed.
Signature move: the pulse plot of the effect field, with the decoded slice.

FORM: Factory Records catalog identity (Peter Saville), dealt challenger brand-identity-canon-saville-catalog-sleeve,
chosen over the assigned Night Desk Photography; seed key 59e7ab65 (re-roll 1).

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
