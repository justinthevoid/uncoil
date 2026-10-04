//! Physical placement: where each LED of each device sits on the desk, in key units (1u = 19.05 mm),
//! x to the right, y toward the user.

use crate::device::{DeviceDef, LayoutDef};
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
        crate::device::Kind::Keyboard => Placement { x: 0.0, y: 0.0 },
        crate::device::Kind::Mouse => Placement { x: 20.75, y: 3.1 },
        crate::device::Kind::Mousemat => Placement { x: 11.125, y: 3.375 },
        _ => Placement { x: 0.0, y: 8.0 },
    }
}

fn parse_key(spec: &str) -> (&str, f32) {
    match spec.rsplit_once(':') {
        Some((name, w)) => (name, w.parse().unwrap_or(1.0)),
        None => (spec, 1.0),
    }
}

pub fn place(def: &DeviceDef, at: Placement) -> PlacedDevice {
    let m = &def.matrix;
    let mut positions = vec![vec![None; m.cols]; m.rows];
    let mut shapes = Vec::new();
    let (bx, by, bw, bh);

    match &def.layout {
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

    PlacedDevice { id: def.id.clone(), name: def.name.clone(), x: bx, y: by, w: bw, h: bh, positions, shapes }
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

    #[test]
    fn keyboard_every_named_slot_is_placed() {
        let defs = builtin();
        let kb = defs.iter().find(|d| d.id == "razer-blackwidow-v4-pro-75").unwrap();
        let p = place(kb, Placement { x: 0.0, y: 0.0 });
        let named = kb.matrix.names.iter().flatten().filter(|n| !n.is_empty()).count();
        let placed = p.positions.iter().flatten().filter(|x| x.is_some()).count();
        assert_eq!(named, 99);
        assert_eq!(placed, named, "every named LED must have a physical position");
    }

    #[test]
    fn hidden_underglow_slots_land_on_the_sides() {
        let defs = builtin();
        let kb = defs.iter().find(|d| d.id == "razer-blackwidow-v4-pro-75").unwrap();
        let p = place(kb, Placement { x: 0.0, y: 0.0 });
        let (lx, _) = p.positions[0][14].unwrap(); // LU1, stored in the top-right of the matrix
        let (rx, _) = p.positions[5][5].unwrap(); // RU1, stored next to the spacebar
        assert!(lx < 0.0, "LU1 should be left of the keyboard, got x={lx}");
        assert!(rx > 16.25, "RU1 should be right of the keyboard, got x={rx}");
    }

    #[test]
    fn default_desk_bounds_match_the_effect_default() {
        let placed: Vec<PlacedDevice> = builtin().iter().map(|d| place(d, default_placement(d))).collect();
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
}
