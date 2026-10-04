//! Physical placement: where each LED of each device sits on the desk, in key units (1u = 19.05 mm),
//! x to the right, y toward the user.

use crate::device::{DeviceDef, Kind, LayoutDef};
use crate::effect::Bounds;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A key or LED shape for drawing previews.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shape {
    pub name: String,
    /// Matrix slot driving this shape.
    pub row: usize,
    pub col: usize,
    /// Centre on the desk.
    pub x: f32,
    pub y: f32,
    /// Size (keys); LED-only points use a small default.
    pub w: f32,
    pub h: f32,
    pub is_key: bool,
}

/// A device's LED geometry on the desk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacedDevice {
    pub id: String,
    pub name: String,
    /// Bounding box of the device body on the desk.
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Desk position of every matrix slot (None = no LED there).
    #[serde(skip)]
    pub positions: Vec<Vec<Option<(f32, f32)>>>,
    pub shapes: Vec<Shape>,
}

/// Where a device sits on the desk. For keyboards (x, y) is the top-left of the key area;
/// for point-layout devices it is the device centre.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub x: f32,
    pub y: f32,
}

/// Sensible default desk: keyboard at the origin, mouse to its right, mat underneath both.
pub fn default_placement(def: &DeviceDef) -> Placement {
    match def.kind {
        Kind::Keyboard => Placement { x: 0.0, y: 0.0 },
        Kind::Mouse => Placement { x: 20.75, y: 3.1 },
        Kind::Mousemat => Placement { x: 11.125, y: 3.375 },
        _ => Placement { x: 0.0, y: 8.0 },
    }
}

/// Space left between an auto-placed device and its neighbour, in key units.
const GAP: f32 = 1.0;

fn kind_rank(k: Kind) -> u8 {
    match k {
        Kind::Keyboard => 0,
        Kind::Mouse => 1,
        Kind::Mousemat => 2,
        Kind::Headset => 3,
        Kind::Other => 4,
    }
}

/// The devices the desk shows, in desk order (keyboards, mice, mats, the rest; file order within a kind):
/// every supported device with a layout, every device the config places, and every connected device with a
/// layout. Devices without a matrix and layout never appear.
pub fn desk_devices<'a>(
    defs: impl IntoIterator<Item = &'a DeviceDef>,
    configured: &HashMap<String, Placement>,
    connected: impl Fn(&str) -> bool,
) -> Vec<&'a DeviceDef> {
    let mut v: Vec<&DeviceDef> = defs
        .into_iter()
        .filter(|d| d.lit().is_some())
        .filter(|d| !d.is_experimental() || configured.contains_key(&d.id) || connected(&d.id))
        .collect();
    v.sort_by_key(|d| kind_rank(d.kind));
    v
}

/// Where each desk device sits. Devices the config places stay exactly there; every other device goes to
/// its kind's default spot when no device of that kind is on the desk yet (so the default desk never
/// changes), else next to the others of its kind: a second keyboard below the first, another mouse to the
/// right of the mice, another mat below the mats, anything else to the right of the desk.
pub fn arrange<'a>(
    devices: &[&'a DeviceDef],
    configured: &HashMap<String, Placement>,
) -> Vec<(&'a DeviceDef, Placement, PlacedDevice)> {
    let mut out: Vec<Option<(Placement, PlacedDevice)>> = vec![None; devices.len()];
    for (i, d) in devices.iter().enumerate() {
        if let Some(&at) = configured.get(&d.id) {
            out[i] = place(d, at).map(|p| (at, p));
        }
    }
    // the top of the next auto-placed external device in the PC column
    let mut column_y: Option<f32> = None;
    for (i, d) in devices.iter().enumerate() {
        if out[i].is_some() || configured.contains_key(&d.id) {
            continue;
        }
        let placed: Vec<(Kind, &PlacedDevice)> =
            devices.iter().zip(&out).filter_map(|(d, o)| o.as_ref().map(|(_, p)| (d.kind, p))).collect();
        let at = if is_external(&d.id) { column_place(d, &placed, &mut column_y) } else { auto_place(d, &placed) };
        out[i] = place(d, at).map(|p| (at, p));
    }
    devices.iter().zip(out).filter_map(|(d, o)| o.map(|(at, p)| (*d, at, p))).collect()
}

fn auto_place(def: &DeviceDef, placed: &[(Kind, &PlacedDevice)]) -> Placement {
    let same: Vec<&PlacedDevice> = placed.iter().filter(|(k, _)| *k == def.kind).map(|(_, p)| *p).collect();
    let Some(first) = same.first() else { return default_placement(def) };
    let Some(origin) = place(def, Placement { x: 0.0, y: 0.0 }) else { return default_placement(def) };
    let bottom = |ps: &[&PlacedDevice]| ps.iter().map(|p| p.y + p.h).fold(f32::MIN, f32::max);
    let right = |ps: &[&PlacedDevice]| ps.iter().map(|p| p.x + p.w).fold(f32::MIN, f32::max);
    // top-left of the new device's body
    let (bx, by) = match def.kind {
        Kind::Keyboard | Kind::Mousemat => (first.x, bottom(&same) + GAP),
        Kind::Mouse => (right(&same) + GAP, first.center().1 - origin.h / 2.0),
        _ => {
            let all: Vec<&PlacedDevice> = placed.iter().map(|(_, p)| *p).collect();
            (right(&all) + GAP, first.y)
        }
    };
    Placement { x: bx - origin.x, y: by - origin.y }
}

/// External devices (driven through OpenRGB) have ids starting with this.
pub const EXTERNAL_PREFIX: &str = "openrgb:";

pub fn is_external(id: &str) -> bool {
    id.starts_with(EXTERNAL_PREFIX)
}

/// Space between devices stacked in the PC column.
const COLUMN_GAP: f32 = 0.5;

/// "The PC": a column left of the first keyboard, top-aligned with it; external devices stack top to bottom
/// in desk order (the daemon lists motherboards, then RAM, GPUs and the rest).
fn column_place(def: &DeviceDef, placed: &[(Kind, &PlacedDevice)], column_y: &mut Option<f32>) -> Placement {
    let Some(body) = place(def, Placement { x: 0.0, y: 0.0 }) else { return default_placement(def) };
    // the keyboard's body; the default keyboard's when the desk has none
    let (kx, ky) = placed.iter().find(|(k, _)| *k == Kind::Keyboard).map_or((-0.3, -0.3), |(_, p)| (p.x, p.y));
    let top = column_y.unwrap_or(ky);
    *column_y = Some(top + body.h + COLUMN_GAP);
    // body top-left at (kx - GAP - w, top); for a points layout `at` is the body's centre
    Placement { x: kx - GAP - body.w - body.x, y: top - body.y }
}

/// How a zone's LEDs are laid out (OpenRGB zone types: single, linear, matrix).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ZoneKind {
    Single,
    #[default]
    Linear,
    Matrix,
}

/// A matrix zone's grid: the zone's LED index in each cell, row by row (`None` where there is no LED).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneMatrix {
    pub width: u32,
    pub height: u32,
    pub map: Vec<Option<u32>>,
}

/// One zone of an external device: its LEDs are the next `leds` in the device's LED order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalZone {
    pub name: String,
    pub kind: ZoneKind,
    pub leds: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matrix: Option<ZoneMatrix>,
}

/// `name` as an id part: lowercase `[a-z0-9-]`, no repeated or edge dashes, at most 48 characters.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.truncate(48);
    let out = out.trim_end_matches('-');
    if out.is_empty() {
        "device".into()
    } else {
        out.into()
    }
}

/// LED names of an external device, from its zones: a zone's only LED takes the zone's name, others the
/// zone's name and a number from 1; a name used twice gets " (2)", " (3)", ...
pub fn external_led_names(zones: &[ExternalZone]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for z in zones {
        let zone = if z.name.is_empty() { "LED" } else { z.name.as_str() };
        for i in 0..z.leds {
            let base = if z.leds == 1 { zone.to_string() } else { format!("{zone} {}", i + 1) };
            let mut name = base.clone();
            let mut n = 2;
            while names.contains(&name) {
                name = format!("{base} ({n})");
                n += 1;
            }
            names.push(name);
        }
    }
    names
}

/// LED pitch in a strip or grid, the width of a strip (or a single LED's column), the tallest and shortest
/// a device's LED area gets, the widest a grid gets, and the margin around the LEDs (key units).
const PITCH: f32 = 0.3;
const STRIP_W: f32 = 0.6;
const MAX_H: f32 = 3.0;
const MIN_H: f32 = 0.6;
const MAX_GRID_W: f32 = 6.0;
const PAD: f32 = 0.3;

/// A desk device for an external device: one row of LEDs (in LED order) on a points layout. Zones sit side
/// by side, left to right: a single LED as a point, a linear zone as a vertical strip, a matrix zone as its
/// grid. Kind `other`, no USB endpoints: only the desk and the effects ever see it.
pub fn external_def(id: &str, name: &str, zones: &[ExternalZone]) -> DeviceDef {
    // per zone with LEDs: its column's width and its points relative to the column's top-left
    let mut columns: Vec<(f32, Vec<(f32, f32)>)> = Vec::new();
    for z in zones.iter().filter(|z| z.leds > 0) {
        let n = z.leds as usize;
        let col = match (&z.kind, &z.matrix) {
            (ZoneKind::Matrix, Some(m)) if m.width > 0 && m.height > 0 => {
                let cell = PITCH.min(MAX_H / m.height as f32).min(MAX_GRID_W / m.width as f32);
                // LEDs the map leaves out sit in the first cell
                let mut pts = vec![(cell / 2.0, cell / 2.0); n];
                for (i, led) in m.map.iter().enumerate() {
                    if let Some(led) = led.filter(|&l| (l as usize) < n) {
                        let (r, c) = (i / m.width as usize, i % m.width as usize);
                        pts[led as usize] = (cell * (c as f32 + 0.5), cell * (r as f32 + 0.5));
                    }
                }
                (cell * m.width as f32, pts)
            }
            _ => {
                let step = if n > 1 { PITCH.min(MAX_H / n as f32) } else { PITCH };
                (STRIP_W, (0..n).map(|i| (STRIP_W / 2.0, step * (i as f32 + 0.5))).collect())
            }
        };
        columns.push(col);
    }
    let height = |pts: &[(f32, f32)]| pts.iter().map(|p| p.1).fold(0.0f32, f32::max) + PITCH / 2.0;
    let inner_h = columns.iter().map(|(_, p)| height(p)).fold(0.0f32, f32::max).clamp(MIN_H, MAX_H);
    let inner_w = columns.iter().map(|(w, _)| *w).sum::<f32>().max(STRIP_W);
    let (width, depth) = (inner_w + 2.0 * PAD, inner_h + 2.0 * PAD);
    let mut points = Vec::new();
    let mut x0 = PAD;
    for (w, pts) in &columns {
        // each column centred vertically
        let y0 = PAD + (inner_h - height(pts).min(inner_h)) / 2.0;
        points.extend(pts.iter().map(|(x, y)| [x0 + x - width / 2.0, y0 + y - depth / 2.0]));
        x0 += w;
    }
    let mut names = external_led_names(zones);
    names.truncate(points.len());
    // through the device-file parser the daemon already has (no second deserialiser in the binary)
    let list = |items: Vec<String>| items.join(", ");
    let src = format!(
        "id = {}\nname = {}\nkind = \"other\"\nvendor_id = 0\nusb = []\n\
         [matrix]\nrows = 1\ncols = {}\nnames = [[{}]]\n\
         [layout]\ntype = \"points\"\nwidth = {width:?}\ndepth = {depth:?}\npoints = [{}]\n",
        toml_str(id),
        toml_str(name),
        names.len(),
        list(names.iter().map(|n| toml_str(n)).collect()),
        list(points.iter().map(|[x, y]| format!("[{x:?}, {y:?}]")).collect()),
    );
    toml::from_str(&src).expect("an external device definition")
}

/// `s` as a TOML basic string.
fn toml_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn parse_key(spec: &str) -> (&str, f32) {
    match spec.rsplit_once(':') {
        Some((name, w)) => (name, w.parse().unwrap_or(1.0)),
        None => (spec, 1.0),
    }
}

/// A device's LEDs and body on the desk; `None` for a device without a matrix and layout.
pub fn place(def: &DeviceDef, at: Placement) -> Option<PlacedDevice> {
    let (m, layout) = def.lit()?;
    let mut positions = vec![vec![None; m.cols]; m.rows];
    let mut shapes = Vec::new();
    let (bx, by, bw, bh);

    match layout {
        LayoutDef::Keyboard { width, depth, rows, underglow } => {
            // key rectangles by name
            let mut keys: HashMap<&str, (f32, f32, f32, f32)> = HashMap::new();
            for row in rows {
                let mut x = 0.0;
                for spec in &row.keys {
                    let (name, w) = parse_key(spec);
                    if name != "gap" {
                        keys.insert(name, (x, row.y, w, 1.0));
                    }
                    x += w;
                }
            }
            // underglow LEDs by name
            let mut glow: HashMap<String, (f32, f32)> = HashMap::new();
            if let Some(u) = underglow {
                for side in [&u.left, &u.right].into_iter().flatten() {
                    for i in 0..side.count {
                        let t = if side.count > 1 { i as f32 / (side.count - 1) as f32 } else { 0.5 };
                        glow.insert(format!("{}{}", side.prefix, i), (side.x, u.y_start + t * (u.y_end - u.y_start)));
                    }
                }
            }
            for (r, row) in m.names.iter().enumerate() {
                for (c, name) in row.iter().enumerate() {
                    if name.is_empty() {
                        continue;
                    }
                    if let Some(&(x, y, w, h)) = keys.get(name.as_str()) {
                        let (cx, cy) = (at.x + x + w / 2.0, at.y + y + h / 2.0);
                        positions[r][c] = Some((cx, cy));
                        shapes.push(Shape { name: name.clone(), row: r, col: c, x: cx, y: cy, w, h, is_key: true });
                    } else if let Some(&(x, y)) = glow.get(name) {
                        let (cx, cy) = (at.x + x, at.y + y);
                        positions[r][c] = Some((cx, cy));
                        shapes.push(Shape {
                            name: name.clone(),
                            row: r,
                            col: c,
                            x: cx,
                            y: cy,
                            w: 0.35,
                            h: 0.55,
                            is_key: false,
                        });
                    }
                }
            }
            bx = at.x - 0.3;
            by = at.y - 0.3;
            bw = width + 0.6;
            bh = depth + 0.6;
        }
        LayoutDef::Points { width, depth, points } => {
            for (i, p) in points.iter().enumerate() {
                let (r, c) = (i / m.cols, i % m.cols);
                let (cx, cy) = (at.x + p[0], at.y + p[1]);
                positions[r][c] = Some((cx, cy));
                shapes.push(Shape {
                    name: m.names[r][c].clone(),
                    row: r,
                    col: c,
                    x: cx,
                    y: cy,
                    w: 0.4,
                    h: 0.4,
                    is_key: false,
                });
            }
            bx = at.x - width / 2.0;
            by = at.y - depth / 2.0;
            bw = *width;
            bh = *depth;
        }
    }

    Some(PlacedDevice { id: def.id.clone(), name: def.name.clone(), x: bx, y: by, w: bw, h: bh, positions, shapes })
}

impl PlacedDevice {
    /// Centre of the device body on the desk.
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// Desk position of the LED with this shape name.
    pub fn shape_position(&self, name: &str) -> Option<(f32, f32)> {
        self.shapes.iter().find(|s| s.name == name).map(|s| (s.x, s.y))
    }
}

/// The desk as the daemon renders it and the app previews it: which devices it shows and where
/// ([`desk_devices`], [`arrange`]), its extent and the keyboard's centre (what the effects need), built
/// from the config's placements and the connected devices.
#[derive(Debug, Default)]
pub struct Desk {
    /// Every device on the desk, in desk order, with its kind.
    pub devices: Vec<(Kind, PlacedDevice)>,
    pub bounds: Option<Bounds>,
    pub keyboard_center: Option<(f32, f32)>,
    /// Where each desk device sits (configured, default or auto-placed).
    placements: HashMap<String, Placement>,
}

impl Desk {
    /// Supported devices, devices the config places (`configured`) and the `connected` ones, at their
    /// configured place or auto-placed next to their kind.
    pub fn new<'a>(
        defs: impl IntoIterator<Item = &'a DeviceDef>,
        configured: &HashMap<String, Placement>,
        connected: impl Fn(&str) -> bool,
    ) -> Desk {
        let shown = desk_devices(defs, configured, connected);
        let arranged = arrange(&shown, configured);
        let placements = arranged.iter().map(|(d, at, _)| (d.id.clone(), *at)).collect();
        let devices: Vec<(Kind, PlacedDevice)> = arranged.into_iter().map(|(d, _, p)| (d.kind, p)).collect();
        let bounds = desk_bounds(devices.iter().map(|(_, p)| p));
        let keyboard_center = devices.iter().find(|(k, _)| *k == Kind::Keyboard).map(|(_, p)| p.center());
        Desk { devices, bounds, keyboard_center, placements }
    }

    /// Where a device sits on the desk; its kind's default spot when the desk does not show it.
    pub fn placement(&self, def: &DeviceDef) -> Placement {
        self.placements.get(&def.id).copied().unwrap_or_else(|| default_placement(def))
    }

    /// Whether every device sits where it sits on `other`.
    pub fn same_places(&self, other: &Desk) -> bool {
        self.placements == other.placements
    }

    /// Where the key with this shape name sits: on a connected keyboard if one has it, else the first.
    pub fn key_position(&self, name: &str, connected: impl Fn(&str) -> bool) -> Option<(f32, f32)> {
        let keyboards = || self.devices.iter().filter(|(k, _)| *k == Kind::Keyboard).map(|(_, p)| p);
        keyboards()
            .filter(|kb| connected(&kb.id))
            .find_map(|kb| kb.shape_position(name))
            .or_else(|| keyboards().find_map(|kb| kb.shape_position(name)))
    }

    /// The effect inputs for this desk, with these key presses and audio level.
    pub fn inputs<'a>(&self, presses: &'a [crate::effect::Press], audio: f32) -> crate::effect::Inputs<'a> {
        crate::effect::Inputs { presses, audio, bounds: self.bounds, keyboard_center: self.keyboard_center }
    }
}

/// The desk's extent: the union of every device body. `None` for an empty desk.
pub fn desk_bounds<'a>(devices: impl IntoIterator<Item = &'a PlacedDevice>) -> Option<Bounds> {
    devices.into_iter().fold(None, |acc, d| {
        let b = Bounds { min_x: d.x, min_y: d.y, max_x: d.x + d.w, max_y: d.y + d.h };
        Some(match acc {
            None => b,
            Some(a) => Bounds {
                min_x: a.min_x.min(b.min_x),
                min_y: a.min_y.min(b.min_y),
                max_x: a.max_x.max(b.max_x),
                max_y: a.max_y.max(b.max_y),
            },
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::builtin;

    fn kb() -> DeviceDef {
        builtin().into_iter().find(|d| d.id == "razer-blackwidow-v4-pro-75").unwrap()
    }

    #[test]
    fn keyboard_every_named_slot_is_placed() {
        let kb = kb();
        let p = place(&kb, Placement { x: 0.0, y: 0.0 }).unwrap();
        let named = kb.matrix.as_ref().unwrap().names.iter().flatten().filter(|n| !n.is_empty()).count();
        let placed = p.positions.iter().flatten().filter(|x| x.is_some()).count();
        assert_eq!(named, 99);
        assert_eq!(placed, named, "every named LED must have a physical position");
    }

    #[test]
    fn hidden_underglow_slots_land_on_the_sides() {
        let p = place(&kb(), Placement { x: 0.0, y: 0.0 }).unwrap();
        let (lx, _) = p.positions[0][14].unwrap(); // LU1, stored in the top-right of the matrix
        let (rx, _) = p.positions[5][5].unwrap(); // RU1, stored next to the spacebar
        assert!(lx < 0.0, "LU1 should be left of the keyboard, got x={lx}");
        assert!(rx > 16.25, "RU1 should be right of the keyboard, got x={rx}");
    }

    #[test]
    fn default_desk_bounds_match_the_effect_default() {
        let defs = builtin();
        let shown = desk_devices(&defs, &HashMap::new(), |_| false);
        let desk = arrange(&shown, &HashMap::new());
        assert_eq!(
            desk.iter().map(|(d, _, _)| d.id.as_str()).collect::<Vec<_>>(),
            ["razer-blackwidow-v4-pro-75", "razer-basilisk-v3-pro", "razer-goliathus-chroma-extended"],
            "the default desk is the three supported devices, keyboard first"
        );
        for (d, at, _) in &desk {
            assert_eq!(*at, default_placement(d), "{}", d.id);
        }
        let placed: Vec<PlacedDevice> = desk.into_iter().map(|(_, _, p)| p).collect();
        let b = desk_bounds(&placed).unwrap();
        let d = Bounds::DEFAULT;
        for (got, want) in [(b.min_x, d.min_x), (b.min_y, d.min_y), (b.max_x, d.max_x), (b.max_y, d.max_y)] {
            assert!((got - want).abs() < 1e-4, "{b:?} vs {d:?}");
        }
        let kb = placed.iter().find(|p| p.id == "razer-blackwidow-v4-pro-75").unwrap();
        let (cx, cy) = kb.center();
        assert!((cx - 8.125).abs() < 1e-4 && (cy - 3.125).abs() < 1e-4);
        assert_eq!(kb.shape_position("Escape"), Some((0.5, 0.5)));
        assert!(desk_bounds(&[]).is_none());
    }

    #[test]
    fn the_default_desk_matches_the_effect_defaults() {
        let defs = builtin();
        let desk = Desk::new(&defs, &HashMap::new(), |_| false);
        assert_eq!(desk.devices.len(), 3);
        assert_eq!(desk.bounds, Some(Bounds::DEFAULT));
        let (cx, cy) = desk.keyboard_center.unwrap();
        assert!((cx - 8.125).abs() < 1e-4 && (cy - 3.125).abs() < 1e-4);
        assert_eq!(desk.key_position("Escape", |_| false), Some((0.5, 0.5)));
        assert_eq!(desk.key_position("No Such Key", |_| true), None);
        let kb = defs.iter().find(|d| d.id == "razer-blackwidow-v4-pro-75").unwrap();
        assert_eq!(desk.placement(kb), Placement { x: 0.0, y: 0.0 });
        assert!(desk.same_places(&Desk::new(&defs, &HashMap::new(), |_| false)));
        let inputs = desk.inputs(&[], 0.5);
        assert_eq!((inputs.bounds, inputs.keyboard_center, inputs.audio), (desk.bounds, desk.keyboard_center, 0.5));
    }

    /// A made-up experimental keyboard or mouse with a one-LED layout.
    fn extra(kind: &str, id: &str) -> DeviceDef {
        let (matrix, layout) = match kind {
            "keyboard" => (
                r#"names = [["Escape"]]"#,
                r#"type = "keyboard"
                   width = 15.0
                   depth = 5.0
                   rows = [{ y = 0.0, keys = ["Escape:1"] }]"#,
            ),
            _ => (
                r#"names = [["Logo"]]"#,
                r#"type = "points"
                   width = 2.5
                   depth = 5.0
                   points = [[0.0, 0.0]]"#,
            ),
        };
        let src = format!(
            r#"id = "{id}"
               name = "{id}"
               kind = "{kind}"
               vendor_id = 0x1532
               support = "experimental"
               [[usb]]
               product_id = 0x0001
               interface = 0
               usage_page = 1
               usage = 2
               transaction_id = 0x1F
               [matrix]
               rows = 1
               cols = 1
               {matrix}
               [layout]
               {layout}
            "#
        );
        DeviceDef::from_toml(&src).unwrap()
    }

    #[test]
    fn connected_devices_are_placed_next_to_their_kind() {
        let mut defs = builtin();
        defs.push(extra("keyboard", "test-kb"));
        defs.push(extra("mouse", "test-mouse"));
        // not connected and not configured: not on the desk
        let shown = desk_devices(&defs, &HashMap::new(), |_| false);
        assert!(shown.iter().all(|d| !d.id.starts_with("test-")));
        // connected: below the first keyboard / right of the first mouse; the three keep their places
        let shown = desk_devices(&defs, &HashMap::new(), |id| id.starts_with("test-"));
        let desk = arrange(&shown, &HashMap::new());
        let get = |id: &str| desk.iter().find(|(d, _, _)| d.id == id).unwrap();
        let (_, kb_at, kb) = get("razer-blackwidow-v4-pro-75");
        let (_, mouse_at, mouse) = get("razer-basilisk-v3-pro");
        assert_eq!(*kb_at, Placement { x: 0.0, y: 0.0 });
        assert_eq!(*mouse_at, Placement { x: 20.75, y: 3.1 });
        let (_, _, kb2) = get("test-kb");
        assert!((kb2.x - kb.x).abs() < 1e-4 && (kb2.y - (kb.y + kb.h + GAP)).abs() < 1e-4, "{kb2:?}");
        let (_, _, m2) = get("test-mouse");
        assert!((m2.x - (mouse.x + mouse.w + GAP)).abs() < 1e-4, "{m2:?}");
        assert!((m2.center().1 - mouse.center().1).abs() < 1e-4);
        // configured devices stay exactly where the config says
        let mut cfg = HashMap::new();
        cfg.insert("test-mouse".to_string(), Placement { x: -5.0, y: 1.0 });
        cfg.insert("razer-blackwidow-v4-pro-75".to_string(), Placement { x: 2.0, y: 2.0 });
        let shown = desk_devices(&defs, &cfg, |_| false);
        let desk = arrange(&shown, &cfg);
        let at = |id: &str| desk.iter().find(|(d, _, _)| d.id == id).map(|(_, at, _)| *at).unwrap();
        assert_eq!(at("test-mouse"), Placement { x: -5.0, y: 1.0 });
        assert_eq!(at("razer-blackwidow-v4-pro-75"), Placement { x: 2.0, y: 2.0 });
        assert!(!desk.iter().any(|(d, _, _)| d.id == "test-kb"));
        // the unconfigured Basilisk now goes right of the configured mouse
        let (_, _, b) = desk.iter().find(|(d, _, _)| d.id == "razer-basilisk-v3-pro").unwrap();
        assert!(b.x > -5.0);
    }

    #[test]
    fn feature_only_devices_are_never_placed() {
        let src = r#"id = "x"
                     name = "x"
                     kind = "mouse"
                     vendor_id = 0x1532
                     features = ["profiles"]
                     [[usb]]
                     product_id = 1
                     interface = 0
                     usage_page = 1
                     usage = 2
                     transaction_id = 0x1F
                  "#;
        let d = DeviceDef::from_toml(src).unwrap();
        assert!(place(&d, Placement { x: 0.0, y: 0.0 }).is_none());
        assert!(desk_devices([&d], &HashMap::new(), |_| true).is_empty());
    }

    fn zone(name: &str, kind: ZoneKind, leds: u32) -> ExternalZone {
        ExternalZone { name: name.into(), kind, leds, matrix: None }
    }

    /// A board like the maintainer's: a strip, a single LED, a 2x3 grid with two empty cells, an empty header.
    fn board() -> DeviceDef {
        let grid = ZoneMatrix { width: 3, height: 2, map: vec![Some(0), Some(1), None, Some(2), Some(3), None] };
        let zones = [
            zone("Aura Mainboard", ZoneKind::Linear, 3),
            zone("Logo", ZoneKind::Single, 1),
            ExternalZone { matrix: Some(grid), ..zone("Panel", ZoneKind::Matrix, 4) },
            zone("Addressable 1", ZoneKind::Linear, 0),
        ];
        external_def("openrgb:asus-rog-strix", "ASUS ROG STRIX", &zones)
    }

    #[test]
    fn external_ids_and_led_names() {
        assert_eq!(slug("ASUS ROG STRIX B550-F GAMING (WI-FI)"), "asus-rog-strix-b550-f-gaming-wi-fi");
        assert_eq!(slug("  GeForce   RTX 4070  "), "geforce-rtx-4070");
        assert_eq!(slug("¿¿"), "device");
        assert!(slug(&"Ab ".repeat(40)).len() <= 48 && !slug(&"Ab ".repeat(40)).ends_with('-'));
        let names = external_led_names(&[zone("DRAM", ZoneKind::Linear, 2), zone("DRAM 1", ZoneKind::Single, 1)]);
        assert_eq!(names, ["DRAM 1", "DRAM 2", "DRAM 1 (2)"]);
        // any name survives the trip through the device-file parser
        let odd = "Odd \"quoted\" \\ name \u{7} [x] = 1";
        let d = external_def("openrgb:odd", odd, &[zone(odd, ZoneKind::Linear, 2)]);
        assert_eq!(d.name, odd);
        assert_eq!(d.matrix.unwrap().names[0][1], format!("{odd} 2"));
    }

    #[test]
    fn external_devices_lay_out_their_zones() {
        let d = board();
        assert_eq!((d.kind, d.usb.len(), d.vendor_id), (Kind::Other, 0, 0));
        let p = place(&d, Placement { x: 0.0, y: 0.0 }).unwrap();
        let names: Vec<&str> = p.shapes.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Aura Mainboard 1",
                "Aura Mainboard 2",
                "Aura Mainboard 3",
                "Logo",
                "Panel 1",
                "Panel 2",
                "Panel 3",
                "Panel 4"
            ]
        );
        for s in &p.shapes {
            assert!(s.x > p.x && s.x < p.x + p.w && s.y > p.y && s.y < p.y + p.h, "{} outside its box", s.name);
        }
        let at = |n: &str| p.shape_position(n).unwrap();
        // the strip runs top to bottom, zones run left to right, the grid keeps its rows and columns
        assert!(at("Aura Mainboard 1").1 < at("Aura Mainboard 3").1);
        assert!((at("Aura Mainboard 1").0 - at("Aura Mainboard 3").0).abs() < 1e-5);
        assert!(at("Logo").0 > at("Aura Mainboard 1").0 && at("Panel 1").0 > at("Logo").0);
        assert!((at("Panel 1").1 - at("Panel 2").1).abs() < 1e-5 && at("Panel 2").0 > at("Panel 1").0);
        assert!((at("Panel 1").0 - at("Panel 3").0).abs() < 1e-5 && at("Panel 3").1 > at("Panel 1").1);
        // long strips stay within the height limit
        let strip = external_def("openrgb:strip", "Strip", &[zone("Strip", ZoneKind::Linear, 300)]);
        let p = place(&strip, Placement { x: 0.0, y: 0.0 }).unwrap();
        assert_eq!(p.shapes.len(), 300);
        assert!(p.h <= MAX_H + 2.0 * PAD + 1e-4);
    }

    #[test]
    fn external_devices_stack_left_of_the_keyboard() {
        let mut defs = builtin();
        defs.push(board());
        defs.push(external_def("openrgb:vengeance", "Corsair Vengeance", &[zone("DRAM", ZoneKind::Linear, 10)]));
        let desk = Desk::new(&defs, &HashMap::new(), |_| false);
        let get = |id: &str| &desk.devices.iter().find(|(_, d)| d.id == id).unwrap().1;
        let (kb, mb, ram) =
            (get("razer-blackwidow-v4-pro-75"), get("openrgb:asus-rog-strix"), get("openrgb:vengeance"));
        // the Razer devices keep their places
        let plain = Desk::new(&builtin(), &HashMap::new(), |_| false);
        for (_, d) in &plain.devices {
            let now = get(&d.id);
            assert_eq!((now.x, now.y), (d.x, d.y), "{} moved", d.id);
        }
        // a column left of the keyboard: right edges one gap left of it, the board on top, RAM below
        for d in [mb, ram] {
            assert!((d.x + d.w - (kb.x - GAP)).abs() < 1e-4, "{d:?}");
        }
        assert!((mb.y - kb.y).abs() < 1e-4);
        assert!((ram.y - (mb.y + mb.h + COLUMN_GAP)).abs() < 1e-4);
        // the desk now reaches the column; a configured place wins
        assert!(desk.bounds.unwrap().min_x <= ram.x);
        let mut cfg = HashMap::new();
        cfg.insert("openrgb:vengeance".to_string(), Placement { x: 30.0, y: 2.0 });
        let desk = Desk::new(&defs, &cfg, |_| false);
        let ram = &desk.devices.iter().find(|(_, d)| d.id == "openrgb:vengeance").unwrap().1;
        assert!((ram.center().0 - 30.0).abs() < 1e-4 && (ram.center().1 - 2.0).abs() < 1e-4);
    }
}
