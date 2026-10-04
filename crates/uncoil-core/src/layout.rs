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
    for (i, d) in devices.iter().enumerate() {
        if out[i].is_some() || configured.contains_key(&d.id) {
            continue;
        }
        let placed: Vec<(Kind, &PlacedDevice)> =
            devices.iter().zip(&out).filter_map(|(d, o)| o.as_ref().map(|(_, p)| (d.kind, p))).collect();
        let at = auto_place(d, &placed);
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
}
