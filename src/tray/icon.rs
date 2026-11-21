//! The tray icon, drawn at startup so there is no image asset to ship.

use tiny_skia::{FillRule, PathBuilder, Pixmap, Transform};

use crate::render::paint;

pub const SIZE: u32 = 64;

/// Lightning bolt in a 24×24 design grid.
const BOLT: [(f32, f32); 6] = [
    (13.5, 1.5),
    (4.0, 13.5),
    (11.0, 13.5),
    (10.0, 22.5),
    (20.0, 9.5),
    (13.0, 9.5),
];

/// Straight-alpha RGBA. `template`: black glyph only, for the macOS menu bar
/// to tint; otherwise a white bolt on an accent-colored rounded square.
pub fn rgba(template: bool) -> Vec<u8> {
    let mut pm = Pixmap::new(SIZE, SIZE).expect("non-zero size");
    let s = SIZE as f32 / 24.0;
    if !template {
        let r = 5.0 * s;
        let w = SIZE as f32;
        let mut pb = PathBuilder::new();
        pb.move_to(r, 0.0);
        pb.line_to(w - r, 0.0);
        pb.quad_to(w, 0.0, w, r);
        pb.line_to(w, w - r);
        pb.quad_to(w, w, w - r, w);
        pb.line_to(r, w);
        pb.quad_to(0.0, w, 0.0, w - r);
        pb.line_to(0.0, r);
        pb.quad_to(0.0, 0.0, r, 0.0);
        pb.close();
        let bg = pb.finish().expect("valid path");
        pm.fill_path(
            &bg,
            &paint([0, 122, 255, 255]),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    let mut pb = PathBuilder::new();
    pb.move_to(BOLT[0].0 * s, BOLT[0].1 * s);
    for &(x, y) in &BOLT[1..] {
        pb.line_to(x * s, y * s);
    }
    pb.close();
    let bolt = pb.finish().expect("valid path");
    let fg = if template {
        [0, 0, 0, 255]
    } else {
        [255, 255, 255, 255]
    };
    pm.fill_path(
        &bolt,
        &paint(fg),
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    pm.pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_has_opaque_and_clear_pixels() {
        let px = rgba(true);
        assert_eq!(px.len(), (SIZE * SIZE * 4) as usize);
        let alphas: Vec<u8> = px.chunks(4).map(|c| c[3]).collect();
        assert!(alphas.contains(&255) && alphas.contains(&0));
        // Rounded corner is clear, the center of the badge is opaque.
        let badge = rgba(false);
        assert_eq!(badge[3], 0);
        let mid = ((SIZE / 2 * SIZE + 2) * 4 + 3) as usize;
        assert_eq!(badge[mid], 255);
    }
}
