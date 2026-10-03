---
name: uncoil
description: The Swatch Book. Warm, quiet pages; colour only as named gels with jobs and as your devices' real light.
colors:
  ground: "#f7f6f3"
  raised: "#f1efeb"
  surface: "#ffffff"
  surface-2: "#ebe8e3"
  surface-3: "#e2ded7"
  ink: "#1f1d1a"
  ink-2: "#514c45"
  ink-3: "#6b655d"
  ink-4: "#a39d94"
  seam: "#e3e0da"
  seam-2: "#cfcac2"
  gel-yours: "#2f8a4c"
  gel-media: "#b97a12"
  gel-light: "#b13a80"
  gel-system: "#3a63c4"
  ok: "#1f8f3f"
  warn: "#b45309"
  ground-dark: "#161514"
  raised-dark: "#1b1a18"
  surface-dark: "#22201e"
  surface-2-dark: "#2b2825"
  ink-dark: "#eeeae4"
  ink-2-dark: "#c2bbb1"
  ink-3-dark: "#948d83"
  seam-dark: "#2e2b28"
  gel-yours-dark: "#58b06c"
  gel-media-dark: "#e0a33e"
  gel-light-dark: "#cf5aa0"
  gel-system-dark: "#5b84e0"
typography:
  page-title:
    fontFamily: "Segoe UI Variable Display, Segoe UI, system-ui, sans-serif"
    fontSize: "20px"
    fontWeight: 600
    lineHeight: 1.2
    letterSpacing: "-0.01em"
  section-title:
    fontFamily: "Segoe UI Variable Display, Segoe UI, system-ui, sans-serif"
    fontSize: "14px"
    fontWeight: 600
  body:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.45
  label:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 500
  keycap:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "clamp(10px, 1.6cqw, 15px)"
    fontWeight: 600
rounded:
  sm: "5px"
  md: "8px"
  lg: "10px"
spacing:
  xs: "6px"
  sm: "10px"
  md: "16px"
  lg: "22px"
  xl: "28px"
components:
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.ground}"
    rounded: "{rounded.md}"
    height: "34px"
    padding: "0 14px"
  button-quiet:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    rounded: "{rounded.md}"
    height: "34px"
    padding: "0 14px"
  tab-active:
    backgroundColor: "{colors.surface-2}"
    textColor: "{colors.ink}"
    rounded: "{rounded.md}"
    height: "34px"
  card:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.lg}"
    padding: "18px"
  keycap:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sm}"
---

# Design System: uncoil

## Overview

**Creative North Star: "The Swatch Book"**

uncoil files every change like a lighting-gel swatch book. The pages are warm and quiet: warm white in light mode, warm graphite in dark mode, following Windows. There is no brand accent. Colour appears in two places only: as a named gel swatch that says what kind of change something is, and as the real colour your devices are showing.

The app is organised like a peripheral configurator. Along the top are the Desk and one tab per device. The left rail lists what that tab can do. The device is drawn large in the centre, and a details panel on the right holds the selected thing and its Save action. Everything is Segoe UI in sentence case, with standard controls.

**Key Characteristics:**
- Warm neutral pages in light and dark, with no brand accent colour
- Four gels, each with one job: your change (green), media and macros (amber), lighting (magenta), system (blue)
- Device-first shell: Desk and device tabs on top, feature rail on the left, device centre stage, details panel on the right
- Keyboards drawn as flat keycaps from real geometry, with a gel tab on each key that does something different
- Saves to a device always report what the device read back

## Colors

### Primary
There is no brand primary. Selection uses the ink colour itself: outlines, the chosen radio, the active segment. The primary button is filled ink.

### Secondary
- **Gel: your change** (gel-yours): keys and buttons the user remapped.
- **Gel: media & macros** (gel-media): media keys, macro record, game mode, dial actions.
- **Gel: lighting** (gel-light): backlight brighter and dimmer.
- **Gel: system** (gel-system): sleep, low power, profile and DPI actions.
- **OK** (ok) and **Warn** (warn): status dots and result lines only.

### Neutral
Ground, raised (top bar, rail, details panel), surface (cards, keycaps), surface-2 and surface-3 (hover, selected, segment track), four ink steps, and two seam greys for hairlines. Every neutral is warm-tinted. Dark-mode values are the `-dark` tokens.

### Named Rules
**The Gel Rule.** A colour in the interface is either a gel with its name next to it somewhere on the screen, or a device's real LED colour. Never decoration, never a brand accent.

**The No Red Accent Rule.** Red appears only when a device is showing red. The interface never uses red for selection, emphasis or status.

## Typography

**Font:** Segoe UI Variable (Display for titles, Text for everything else), the Windows system face. No downloaded display face and no width-stretched type.

### Hierarchy
- **Page title** (Display, 600, 20px): one per screen.
- **Section title** (Display, 600, 14px): card and list headings.
- **Body** (Text, 400, 13px): controls, lists and prose; hints at 12px in ink-3.
- **Label** (Text, 500, 12px, ink-3): field labels and small headings, sentence case.

### Named Rules
**The Plain Words Rule.** Labels say what things do, in sentence case. No codes, catalog numbers or section numbers. Raw device codes are translated: "Low power mode", not "Razer key 11".

## Layout

The shell is a grid: a 52px top bar (brand, Desk and device tabs with connection dots, engine status, settings), a 208px rail, and the main area. Every feature screen uses the Workspace frame: a title row with its tools, a stage, and an optional details panel of 290-340px on the right. The window opens at about 56% × 62.5% of its monitor (1440 × 900 on 2560 × 1440), clamped between 1024 × 680 and 1600 × 1000. Below 720px of view width the panel stacks under the stage. Lists collapse to one column when their own container is under 600px. Keycap second lines hide on boards under 640px, where the gel tab and the list below still carry the meaning.

## Elevation & Depth

Flat. Regions are separated by hairlines and slightly different warm fills. Keycaps get a 1px edge and a small inset bottom edge so they read as keys. The segmented control's pill has a hairline and a faint shadow. There is no glow anywhere.

## Shapes

5px radius for keycaps and small chips, 8px for buttons, inputs, tabs and list rows, 10px for cards. Status dots are circles. Gel swatches are small upright rectangles, like gel chips.

## Components

### Buttons
- **Primary** (filled ink, ground-coloured text, 34px): the main action, such as "Save to keyboard".
- **Quiet** (hairline outline): secondary actions such as "Restore original" or "Play preview".
- **Onboard write:** saving to a device opens a confirm panel saying what will be saved and how to undo it. The result line has a status dot and quotes the device's read-back.

### Inputs / Fields
- **Segmented control:** equal segments on a surface-2 track; a white (or surface) pill slides to the choice.
- **Slider:** ink fill on a rounded track with a round ink thumb; label and value above, end words below in ink-3.
- **Toggle:** pill switch, filled ink when on.
- **Option list:** radio rows; the chosen ring fills with ink; "On device" tags what the hardware holds now.
- **Search and list:** the "Change to" picker in the Keys panel, grouped and filterable.

### Navigation
Top tabs pick the Desk or a device; each device tab shows a connection dot, and "Not connected" when away. The rail lists the selected tab's features. Ctrl+1-9 switch tabs and Ctrl+, opens settings.

### Keyboard (signature)
Flat keycaps from the device's real geometry. Keys that do something different on the shown layer carry a gel tab across the top and their action in small ink-3 text under the legend. Win and Fn are dimmed. Under the board, a compact "What Fn changes" list repeats each change with its key, its action and its named gel.

### Mouse
The mouse seen from above, with every remappable button as a clickable region (main buttons, wheel and tilt, scroll mode, side buttons, clutch), plus the two underside buttons as chips. Changed buttons get a gel outline.

### Desk preview
The desk to scale, lit with the live effect: the mat's edge ring takes its LED colour, keycaps and underglow light up, and the mouse's LEDs show. Devices that aren't connected are dimmed.

### Effect swatch cards
The effects (Wave, Spectrum, Static, Off) as cards, each with a swatch showing what it does. Static colours are offered as named gel chips (Warm white, Straw, Amber, Primary red, Steel blue and others) plus a custom picker.

## Do's and Don'ts

### Do:
- **Do** name every gel on screen wherever its colour appears in a list.
- **Do** draw the device large in the centre of its own screens.
- **Do** report what the device read back after every save.
- **Do** follow Windows for light and dark, and design both.
- **Do** route JS transition durations through `ms()` in `src/lib/motion.ts`.

### Don't:
- **Don't** add a brand accent colour, glow, gradients on chrome, or a width-stretched display face.
- **Don't** add catalog numbers, section numbers or kicker labels.
- **Don't** show raw protocol codes to users.
- **Don't** animate width, height, padding or margin.
- **Don't** imitate Razer's branding, green or product imagery.
