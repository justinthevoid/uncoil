---
name: uncoil
description: Factory catalog sleeve: matte black, plot-white ink, colour as code.
colors:
  ground: "#0b0b0b"
  raised: "#111111"
  ink: "#f2f2f2"
  ink-2: "#a8a8a8"
  ink-3: "#8a8a8a"
  ink-4: "#5c5c5c"
  seam: "#262626"
  seam-2: "#3d3d3d"
  fac-red: "#e2372c"
  fac-blue: "#2f63e8"
  fac-yellow: "#e9c31b"
  fac-grey: "#6b6b6b"
  fac-white: "#f2f2f2"
typography:
  display:
    fontFamily: "Archivo Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "44px"
    fontWeight: 380
    lineHeight: 0.95
    letterSpacing: "-0.02em"
  caps:
    fontFamily: "Archivo Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "11px"
    fontWeight: 500
    lineHeight: 1.4
    letterSpacing: "0.22em"
  caps-sm:
    fontFamily: "Archivo Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "10px"
    fontWeight: 500
    letterSpacing: "0.2em"
  readout:
    fontFamily: "Archivo Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 400
    letterSpacing: "0.06em"
  body:
    fontFamily: "Archivo Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.6
rounded:
  none: "0px"
spacing:
  xs: "6px"
  sm: "10px"
  md: "16px"
  lg: "24px"
  xl: "32px"
components:
  button-outline:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    rounded: "{rounded.none}"
    padding: "0 34px 0 16px"
    height: "38px"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.ink-2}"
    rounded: "{rounded.none}"
    padding: "8px 10px 8px 14px"
  nav-item-active:
    textColor: "{colors.ink}"
    rounded: "{rounded.none}"
    height: "40px"
  segment-active:
    textColor: "{colors.ink}"
    rounded: "{rounded.none}"
  toggle-on:
    backgroundColor: "{colors.fac-red}"
    rounded: "{rounded.none}"
---

# Design System: uncoil

## Overview

**Creative North Star: "The Factory Catalog Sleeve"**

uncoil looks like a record sleeve from an industrial label: matte black, ruled hairlines, wide engraved capitals, and every object filed under a catalog number. The lighting engine is the record. The app is its sleeve, and the sleeve's job is to print what the engine is doing with the precision of a spec sheet.

The world lends the app four things only: its palette, its lettering, its density, and one signature move (the pulse plot). Controls stay standard controls; the world dresses them in hairlines and catalog codes but never disguises them.

Colour is code. The interface itself is monochrome; the four FAC hues mark state (red is live and active, yellow is a warning, blue and grey are reserved), and real LED colour appears only where the app is decoding something the devices are actually showing.

**Key Characteristics:**
- Matte near-black ground with plot-white ink and four grey steps for hierarchy
- One-pixel hairline rules in place of cards, shadows, or fills
- Every section and option carries a catalog code (FAC 101, 201, 02)
- Wide variable grotesk: wide and light for display numerals, wide and tracked caps for labels
- Zero radius everywhere
- Motion is a single expo-out curve; nothing bounces except the dial needle

## Colors

A monochrome instrument with a four-colour code. Greys do the hierarchy; hues carry meaning.

### Primary
- **Live Red** (fac-red): the only accent with a job in every screen. Marks the active catalog code, the "live" state, the engine lamp, the toggle's on-square, the slider thumb while dragging, the dial tip, and the caret.

### Secondary
- **Caution Yellow** (fac-yellow): device errors and warnings only.
- **Catalog Blue** (fac-blue) and **Catalog Grey** (fac-grey): reserved FAC hues. Grey fills empty code-strip cells; blue is unused in the app and held for the site.

### Neutral
- **Matte Ground** (ground): the page field everywhere.
- **Raised** (raised): reserved for overlays; currently unused in the app.
- **Plot White** (ink): headings, values, active labels.
- **Ink 2 / Ink 3 / Ink 4**: secondary text, field labels, and footnotes, in that order of quietness.
- **Seam / Seam 2**: hairline rules (seam) and stronger rules or inactive control borders (seam-2).

### Named Rules
**The Colour Is Code Rule.** A hue appears only when it means something: red for live or active, yellow for trouble. Decorative colour is never used. LED colours are data and appear only where they decode real output.

**The One Red Rule.** At most one red mark per control. The active nav item gets a red code, not a red code plus a red border.

## Typography

**Display Font:** Archivo Variable (with Segoe UI, system-ui)
**Body Font:** Archivo Variable, same family, using its width axis (62 to 125%) instead of a second face.

**Character:** One grotesk stretched to two voices: wide, light, tight numerals for catalog numbers, and wide, heavily tracked engraved capitals for every label. Body copy sits at normal width and stays quiet.

### Hierarchy
- **Display** (380 weight, 125% width, 44px, 0.95 line height): catalog numbers in section headers (FAC 101). Tabular figures.
- **Readout** (400 weight, 112% width, 15px, uppercase, 0.06em tracking): values in the transmission data list.
- **Caps** (500 weight, 115% width, 11px, 0.22em tracking): section names, field labels, button text.
- **Caps small** (500 weight, 112% width, 10px, 0.2em tracking): table headers, states, the engine readout.
- **Body** (400, 13px, 1.6 line height, at most 62ch): hints, notes, explanations.

### Named Rules
**The Engraved Label Rule.** Every label is engraved caps: small, wide, and tracked. Sentence case is for prose and hints only.

**The No Kicker Rule.** Nothing sits above a heading. The catalog code and name share one baseline (FAC 200 · DEVICES); the code is the heading.

## Layout

The app is a fixed 196px catalog rail plus a main sheet, with 18px of margin around the sheet. Every sheet is one hairline-bordered frame divided by internal hairlines into panes, like a spec sheet ruled into fields. Panes are padded 20 to 24px.

The Lighting sheet is a two-by-two grid: the stage (pulse plot) and transmission data on top, controls and device strips below, with a 236px right column. Below 820px of sheet width (a container query on the view), it collapses to one scrolling column and the transmission data steps aside, since it repeats what the controls show. Tables drop their least-needed columns (LEDs, PID) at the same width. The window's minimum is 900 × 600.

## Elevation & Depth

Flat. There are no shadows, blurs, glass, or gradients. Depth is expressed only by rule weight: seam for the grid, seam-2 for inactive control borders, plot white for the active outline.

### Named Rules
**The Hairline Rule.** If something needs separating, rule it with a 1px line. Never use a fill, a shadow, or a card.

## Shapes

Square-cornered throughout (0px radius). The only curves are functional geometry: the dial's polar rings, the mouse outline in the desk diagram, and the pulse plot's ridges. Small square blocks (7px lamp, red tip, code-strip cells) are the world's dots.

## Components

### Buttons
- **Outline** (plot-white 1px border, 38px, caps text): primary actions such as "Source code". A red block is set flush into the right edge and widens on hover via `scaleX`.
- **Ghost** (seam-2 border, ink-2 text, caps): secondary actions such as "Play preview" and "How it works". Hover brightens the text and border.

### Inputs / Fields
- **Slider:** a hairline track with a 3px vertical tick for a thumb, which turns red while dragged. The label and readout share a row above it; optional end words sit below in caps-sm.
- **Toggle:** a hairline rectangle holding a sliding square, with "On"/"Off" in caps beside it. The square turns red when on.
- **Segmented (catalog cells):** a row of hairline cells, each with a code (101) and a name. An outlined indicator slides to the active cell and carries a red block on its right edge.
- **Dial:** polar rings and spokes, a white needle, and a red square tip. It settles on a spring (the one place motion overshoots), and the spring becomes instant under reduced motion.

- **Catalog list:** a vertical radio index (code, name, optional note) for longer choices such as firmware
  effects and dial modes. The chosen row is boxed in plot white with a red code; "On device" marks what
  the hardware holds now.
- **Onboard write:** an outline button that, when pressed, becomes a hairline-framed confirm row saying what
  will be saved and how to undo it. Results report the device's read-back next to a 7px square lamp
  (grey when fine, yellow on trouble), never a side stripe.

### Navigation
A catalog rail. Entries are a code plus caps label (01 LIGHTING). An outlined box slides between entries on the shared ease; the active code turns red. Ctrl+1 to 4 jump between sections. The engine readout sits at the foot: a red lamp, then rows of caps-sm keys and tabular values.

### Pulse Plot (signature)
Thirty stacked lines, each a slice of the desk sampled across 240 points, with ridges drawn from the live effect field and filled to occlude the lines behind. The slice under the pointer (by default the keyboard's home row) is decoded in live LED colour and marked with a red square on the margin. The plot runs at about 40 fps and starts paused under reduced motion.

### Code Strip
A row of square-ish blocks showing a device's live colours. When the device is away or lighting is off, the blocks fall back to seam grey, so the state reads without colour.

## Do's and Don'ts

### Do:
- **Do** give every section, effect, and device a catalog code, and let the code be the heading.
- **Do** separate with 1px hairlines and nothing else.
- **Do** keep red for live, active, or in-hand states, one mark per control.
- **Do** route every transition through the shared expo-out ease and the 160 / 260 / 420 ms steps, and make them zero under reduced motion (JS transitions go through `ms()` in `src/lib/motion.ts`).
- **Do** show real LED colour only where it decodes actual output.

### Don't:
- **Don't** put kicker or eyebrow labels above headings.
- **Don't** use gradient text, glass, blur, shadows, or rounded corners.
- **Don't** use colour decoratively or invent a fifth hue.
- **Don't** animate layout properties (width, height, padding, margin); use `transform` and `opacity`.
- **Don't** name SVG classes after Tailwind utilities (`outline`, `border`, `ring`). Tailwind's rule applies to the SVG element and scales by the viewBox.
- **Don't** imitate Razer's branding, green, or product imagery. uncoil is its own label.
