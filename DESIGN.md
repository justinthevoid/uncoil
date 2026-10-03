---
name: uncoil
description: Dark, quiet, friendly controls; your desk's real light is the colour.
colors:
  ground: "#0b0b0b"
  raised: "#111111"
  surface: "#161616"
  surface-2: "#1e1e1e"
  surface-3: "#282828"
  ink: "#f2f2f2"
  ink-2: "#b4b4b4"
  ink-3: "#8a8a8a"
  ink-4: "#5c5c5c"
  seam: "#262626"
  seam-2: "#3d3d3d"
  accent: "#e2372c"
  ok: "#3fb950"
  warn: "#e9c31b"
typography:
  page-title:
    fontFamily: "Archivo Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "26px"
    fontWeight: 500
    lineHeight: 1.1
    letterSpacing: "-0.01em"
  section-title:
    fontFamily: "Archivo Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 600
  label:
    fontFamily: "Archivo Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 500
  body:
    fontFamily: "Archivo Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.5
  keycap:
    fontFamily: "Archivo Variable, Segoe UI, system-ui, sans-serif"
    fontSize: "clamp(9px, 1.75cqw, 15px)"
    fontWeight: 500
rounded:
  sm: "4px"
  md: "6px"
  lg: "10px"
spacing:
  xs: "6px"
  sm: "10px"
  md: "16px"
  lg: "24px"
components:
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.ground}"
    rounded: "{rounded.md}"
    height: "36px"
    padding: "0 16px"
  button-quiet:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    rounded: "{rounded.md}"
    height: "36px"
    padding: "0 16px"
  card:
    backgroundColor: "{colors.raised}"
    rounded: "{rounded.lg}"
    padding: "18px 20px"
  nav-item-active:
    backgroundColor: "{colors.surface-3}"
    textColor: "{colors.ink}"
    rounded: "{rounded.md}"
    height: "38px"
  keycap:
    backgroundColor: "{colors.surface-2}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sm}"
---

# Design System: uncoil

## Overview

**Creative North Star: "The Dark Desk"**

uncoil sits next to a desk full of RGB, so the app stays dark, quiet and out of the way, and lets the devices' real light be the only colour that matters. Where the app shows colour, it is the actual colour your keyboard, mouse and mat are showing right now, drawn on a to-scale picture of your desk.

Everything else is plain: sentence-case words, standard controls with small radii, and one red accent for "selected" or "on". There are no codes, numbers or labels for decoration. A first-time user should know what every control does by reading it.

**Key Characteristics:**
- Near-black ground with raised dark cards and four grey text steps
- The desk drawn to scale and lit with the live effect (Lighting, Devices)
- Keyboards drawn as solid keycaps with real legends (Keys)
- Red is the only accent: selected nav icon, toggle on, the dial's tip, Fn actions on keycaps
- Archivo throughout; width axis slightly widened for titles
- One expo-out ease; nothing bounces except the direction dial's needle

## Colors

### Primary
- **Accent red** (accent): selection and "on". The active nav icon, the toggle track when on, the dial tip, the radio dot, and the second legend on a keycap that does something different with Fn held.

### Secondary
- **OK green** (ok): status lamps for a running engine and connected devices.
- **Warning yellow** (warn): device errors, the engine being stopped, and failed results.

### Neutral
- **Ground** (ground): the window background.
- **Raised** (raised): cards and the navigation sidebar.
- **Surface 1-3**: control tracks, keycaps, hover and selected fills, in that order.
- **Ink 1-4**: primary text, secondary text, hints, disabled/placeholder.
- **Seam / Seam 2**: card borders and dividers; control outlines.

### Named Rules
**The Real Light Rule.** Colour in the content area is either the accent or the devices' actual LED colours. Never decorate with colour.

**The One Accent Rule.** Red marks selection or "on", once per control. Green and yellow are reserved for status.

## Typography

**Font:** Archivo Variable (self-hosted), falling back to Segoe UI.

### Hierarchy
- **Page title** (500, 26px, width 112%): one per screen, plain words ("Lighting", "Keys").
- **Section title** (600, 15px): card headings.
- **Label** (500, 13px, ink-2): field labels, sentence case.
- **Body** (400, 13px): descriptions and hints; hints in ink-3 at 12px, at most ~60ch.
- **Small caps** (500, 11px, tracked 0.08em): only for tiny status words where space is tight. Not for labels.

### Named Rules
**The Plain Words Rule.** Every label says what the control does, in sentence case. No codes, catalog numbers, section numbers or kickers above headings.

## Layout

A 200px sidebar (icon plus label per section, engine status at the foot) and a content area with 20-24px padding. Each screen is a page title with a one-line description, then stacked cards. The Keys screen puts the keyboard full width with the editor in a three-column card beneath it. Below 820px of content width (a container query), grids collapse to one column and the screen scrolls. The window's minimum is 900 × 600.

## Elevation & Depth

Mostly flat. Cards are raised by a lighter fill and a 1px seam border, not shadows. Real depth is used only where it explains something physical: keycaps have an inset bottom edge, the keyboard and mouse cast a soft shadow onto the desk, and lit LEDs glow.

## Shapes

Small radii: 4px for keycaps and list items, 6px for buttons and inputs, 10px for cards, full pills for toggles and status chips. The mouse is drawn as a rounded body.

## Components

### Buttons
- **Primary** (filled ink, dark text, 36px): the main action of a card, such as "Save to keyboard".
- **Quiet** (outline, 36px): secondary actions such as "Restore original" or "Pause preview".

### Inputs / Fields
- **Segmented control:** equal segments on a dark track; a raised pill slides to the choice.
- **Slider:** rounded 4px track filled in ink up to a round thumb; label and value above.
- **Toggle:** pill switch; turns red when on.
- **Option list:** radio rows with a ring that fills red when chosen; "On device" tags what the hardware holds now.
- **Search + list:** the Keys editor's "Change to" picker, grouped and filterable.

### Navigation
Sidebar entries are an icon (lucide, 1.75 stroke) and a label. A filled highlight slides between entries; the active icon turns red. Ctrl+1-6 switch sections.

### Keyboard (signature)
The keyboard drawn from its real geometry as solid keycaps with their legends. On the Fn layer, keys that do something different get a tinted cap and their Fn action in red under the legend. Keys that can't be remapped (Win, Fn) are dimmed. In colour mode the caps and underglow take the live LED colours.

### Desk preview
The desk to scale: the mat's edge glows its colour, the keyboard lights up per key, the mouse's LEDs shine. Devices that aren't connected are dimmed.

### Onboard write
Saving to a device's memory takes a second click: the button opens a confirm panel saying what will be saved and how to undo it. The result is reported with a status lamp and the device's read-back.

## Do's and Don'ts

### Do:
- **Do** name things plainly and describe what will happen before it happens.
- **Do** show the devices' real colours on a to-scale desk when colour helps.
- **Do** keep red for selection and "on"; use green and yellow only for status.
- **Do** route JS transition durations through `ms()` in `src/lib/motion.ts` so reduced motion turns them off.

### Don't:
- **Don't** add codes, catalog numbers, section numbers or kicker labels.
- **Don't** use gradient text, glass, or decorative colour.
- **Don't** draw outline-only or line-art versions of physical things people need to read (keyboards, buttons).
- **Don't** animate width, height, padding or margin; use transform and opacity.
- **Don't** name SVG classes after Tailwind utilities (`outline`, `border`, `ring`).
- **Don't** imitate Razer's branding, green or product imagery.
