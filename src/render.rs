//! Rendering. The key idea: the on-screen preview of the selection *is* the
//! export image (`render_export`), blitted into the overlay frame. So what you
//! see is pixel-for-pixel what gets copied/saved.

use tiny_skia::{
    FillRule, IntRect, LineCap, LineJoin, Paint, PathBuilder, Pixmap, PixmapPaint, Stroke,
    Transform,
};

use crate::annotation::{Annotation, Shape};
use crate::geom::{Pt, Rect};

/// Render the selected region of `bg` plus annotations into a new pixmap of
/// the region's size.
pub fn render_export<'a>(
    bg: &Pixmap,
    region: IntRect,
    annotations: impl IntoIterator<Item = &'a Annotation>,
) -> Option<Pixmap> {
    let region = region.intersect(&IntRect::from_xywh(0, 0, bg.width(), bg.height())?)?;
    let mut out = Pixmap::new(region.width(), region.height())?;
    copy_region(bg, region, &mut out, 0, 0);
    let off = Pt::new(-region.x() as f32, -region.y() as f32);
    for a in annotations {
        draw_annotation(&mut out, a, off);
    }
    Some(out)
}

/// Copy `src[region]` into `dst` at `(dx, dy)` with plain row memcpy.
pub fn copy_region(src: &Pixmap, region: IntRect, dst: &mut Pixmap, dx: i32, dy: i32) {
    let Some(dst_rect) = IntRect::from_xywh(dx, dy, region.width(), region.height())
        .and_then(|r| r.intersect(&IntRect::from_xywh(0, 0, dst.width(), dst.height())?))
    else {
        return;
    };
    let sx = (region.x() + dst_rect.x() - dx) as usize;
    let sy = (region.y() + dst_rect.y() - dy) as usize;
    let w = dst_rect.width() as usize * 4;
    let src_stride = src.width() as usize * 4;
    let dst_stride = dst.width() as usize * 4;
    let src_data = src.data();
    let dst_data = dst.data_mut();
    for row in 0..dst_rect.height() as usize {
        let s = (sy + row) * src_stride + sx * 4;
        let d = (dst_rect.y() as usize + row) * dst_stride + dst_rect.x() as usize * 4;
        dst_data[d..d + w].copy_from_slice(&src_data[s..s + w]);
    }
}

/// Draw `src` into `dst` at window position `(x, y)` scaled by `scale`.
pub fn blit_scaled(dst: &mut Pixmap, src: &Pixmap, x: f32, y: f32, scale: f32) {
    if (scale - 1.0).abs() < f32::EPSILON && x.fract() == 0.0 && y.fract() == 0.0 {
        let r = IntRect::from_xywh(0, 0, src.width(), src.height()).unwrap();
        copy_region(src, r, dst, x as i32, y as i32);
    } else {
        let paint = PixmapPaint {
            quality: tiny_skia::FilterQuality::Bilinear,
            ..Default::default()
        };
        dst.draw_pixmap(
            0,
            0,
            src.as_ref(),
            &paint,
            Transform::from_row(scale, 0.0, 0.0, scale, x, y),
            None,
        );
    }
}

/// Darken a copy of `src` (used for the area outside the selection).
pub fn dimmed(src: &Pixmap, factor: f32) -> Pixmap {
    let mut out = src.clone();
    let f = (factor * 256.0) as u16;
    for px in out.data_mut().as_chunks_mut::<4>().0 {
        px[0] = ((px[0] as u16 * f) >> 8) as u8;
        px[1] = ((px[1] as u16 * f) >> 8) as u8;
        px[2] = ((px[2] as u16 * f) >> 8) as u8;
    }
    out
}

pub fn paint(color: [u8; 4]) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(color[0], color[1], color[2], color[3]);
    p.anti_alias = true;
    p
}

fn round_stroke(width: f32) -> Stroke {
    Stroke {
        width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    }
}

/// Draw one annotation into `pm`, offsetting all coordinates by `off`.
pub fn draw_annotation(pm: &mut Pixmap, a: &Annotation, off: Pt) {
    let ts = Transform::from_translate(off.x, off.y);
    let paint = paint(a.color);
    match &a.shape {
        Shape::Pen(pts) => {
            if let Some(path) = smooth_path(pts) {
                pm.stroke_path(&path, &paint, &round_stroke(a.width), ts, None);
            }
        }
        Shape::Marker(pts) => {
            // Highlighter: wide, translucent. One path stroke so overlaps
            // don't double the alpha.
            let mut c = a.color;
            c[3] = 90;
            if let Some(path) = smooth_path(pts) {
                pm.stroke_path(
                    &path,
                    &self::paint(c),
                    &round_stroke(a.width * 5.0),
                    ts,
                    None,
                );
            }
        }
        Shape::Line(p, q) => {
            let mut pb = PathBuilder::new();
            pb.move_to(p.x, p.y);
            pb.line_to(q.x, q.y);
            if let Some(path) = pb.finish() {
                pm.stroke_path(&path, &paint, &round_stroke(a.width), ts, None);
            }
        }
        Shape::Arrow(p, q) => draw_arrow(pm, *p, *q, a.width, &paint, ts),
        Shape::Rect(p, q) => {
            if let Some(r) = Rect::from_points(*p, *q).to_skia() {
                let stroke = Stroke {
                    width: a.width,
                    line_join: LineJoin::Round,
                    ..Default::default()
                };
                pm.stroke_path(&PathBuilder::from_rect(r), &paint, &stroke, ts, None);
            }
        }
        Shape::Ellipse(p, q) => {
            if let Some(path) = Rect::from_points(*p, *q)
                .to_skia()
                .and_then(PathBuilder::from_oval)
            {
                pm.stroke_path(&path, &paint, &round_stroke(a.width), ts, None);
            }
        }
        Shape::Pixelate(p, q) => {
            let r = Rect::from_points(*p, *q).translate(off.x, off.y);
            pixelate(pm, r, (a.width * 3.0).max(6.0) as u32);
        }
    }
}

/// Catmull-Rom-ish smoothing via quadratic curves through midpoints.
fn smooth_path(pts: &[Pt]) -> Option<tiny_skia::Path> {
    let first = *pts.first()?;
    let mut pb = PathBuilder::new();
    pb.move_to(first.x, first.y);
    if pts.len() == 1 {
        // Zero-length segments don't render; nudge to make a round dot.
        pb.line_to(first.x + 0.01, first.y);
    } else if pts.len() == 2 {
        pb.line_to(pts[1].x, pts[1].y);
    } else {
        for w in pts.windows(2).skip(1) {
            let (c, n) = (w[0], w[1]);
            pb.quad_to(c.x, c.y, (c.x + n.x) / 2.0, (c.y + n.y) / 2.0);
        }
        let last = pts[pts.len() - 1];
        pb.line_to(last.x, last.y);
    }
    pb.finish()
}

/// Arrow geometry: shaft ends at the base of a filled triangular head.
pub fn arrow_head(p: Pt, q: Pt, width: f32) -> (Pt, [Pt; 3]) {
    let len = p.dist(q).max(0.001);
    let (ux, uy) = ((q.x - p.x) / len, (q.y - p.y) / len);
    let head_len = (width * 4.0 + 6.0).min(len);
    let half = head_len * 0.55;
    let base = Pt::new(q.x - ux * head_len, q.y - uy * head_len);
    let left = Pt::new(base.x - uy * half, base.y + ux * half);
    let right = Pt::new(base.x + uy * half, base.y - ux * half);
    (base, [q, left, right])
}

fn draw_arrow(pm: &mut Pixmap, p: Pt, q: Pt, width: f32, paint: &Paint, ts: Transform) {
    let (base, tri) = arrow_head(p, q, width);
    let mut pb = PathBuilder::new();
    pb.move_to(p.x, p.y);
    pb.line_to(base.x, base.y);
    if let Some(path) = pb.finish() {
        let s = Stroke {
            width,
            line_cap: LineCap::Round,
            ..Default::default()
        };
        pm.stroke_path(&path, paint, &s, ts, None);
    }
    let mut pb = PathBuilder::new();
    pb.move_to(tri[0].x, tri[0].y);
    pb.line_to(tri[1].x, tri[1].y);
    pb.line_to(tri[2].x, tri[2].y);
    pb.close();
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, paint, FillRule::Winding, ts, None);
    }
}

/// Replace `r` (target pixmap coordinates) with block averages of itself, so
/// earlier annotations underneath get pixelated too.
fn pixelate(pm: &mut Pixmap, r: Rect, block: u32) {
    let Some(r) = r.clamp_to(pm.width() as f32, pm.height() as f32).to_int() else {
        return;
    };
    let stride = pm.width() as usize * 4;
    let data = pm.data_mut();
    let block = block.max(1) as i32;
    let mut by = r.y();
    while by < r.bottom() {
        let bh = block.min(r.bottom() - by);
        let mut bx = r.x();
        while bx < r.right() {
            let bw = block.min(r.right() - bx);
            let mut sum = [0u32; 4];
            for y in by..by + bh {
                let row = y as usize * stride;
                for x in bx..bx + bw {
                    let i = row + x as usize * 4;
                    for c in 0..4 {
                        sum[c] += data[i + c] as u32;
                    }
                }
            }
            let n = (bw * bh) as u32;
            let avg = sum.map(|s| (s / n) as u8);
            for y in by..by + bh {
                let row = y as usize * stride;
                for x in bx..bx + bw {
                    let i = row + x as usize * 4;
                    data[i..i + 4].copy_from_slice(&avg);
                }
            }
            bx += block;
        }
        by += block;
    }
}

/// Convert an opaque premultiplied RGBA pixmap to softbuffer's `0RGB` u32s.
pub fn to_softbuffer(src: &[u8], dst: &mut [u32]) {
    for (d, s) in dst.iter_mut().zip(src.as_chunks::<4>().0) {
        *d = (s[0] as u32) << 16 | (s[1] as u32) << 8 | s[2] as u32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotation::Tool;

    fn solid(w: u32, h: u32, rgb: [u8; 3]) -> Pixmap {
        let mut p = Pixmap::new(w, h).unwrap();
        p.fill(tiny_skia::Color::from_rgba8(rgb[0], rgb[1], rgb[2], 255));
        p
    }

    fn px(p: &Pixmap, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * p.width() + x) * 4) as usize;
        p.data()[i..i + 4].try_into().unwrap()
    }

    #[test]
    fn export_crops_to_region() {
        let mut bg = solid(100, 100, [0, 0, 0]);
        // Mark pixel (40, 30) white.
        let i = (30 * 100 + 40) * 4;
        bg.data_mut()[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
        let out = render_export(&bg, IntRect::from_xywh(40, 30, 10, 10).unwrap(), []).unwrap();
        assert_eq!((out.width(), out.height()), (10, 10));
        assert_eq!(px(&out, 0, 0), [255, 255, 255, 255]);
        assert_eq!(px(&out, 1, 0), [0, 0, 0, 255]);
    }

    #[test]
    fn annotations_use_image_coordinates() {
        let bg = solid(100, 100, [0, 0, 0]);
        let mut a =
            Annotation::begin(Tool::Rect, Pt::new(50.0, 50.0), [255, 0, 0, 255], 4.0).unwrap();
        a.update(Pt::new(70.0, 70.0));
        let out = render_export(&bg, IntRect::from_xywh(40, 40, 40, 40).unwrap(), [&a]).unwrap();
        // Rect edge at image x=50 → export x=10.
        assert_eq!(px(&out, 10, 20), [255, 0, 0, 255]);
        // Interior untouched.
        assert_eq!(px(&out, 20, 20), [0, 0, 0, 255]);
    }

    #[test]
    fn region_outside_image_is_clamped() {
        let bg = solid(10, 10, [1, 2, 3]);
        let out = render_export(&bg, IntRect::from_xywh(5, 5, 50, 50).unwrap(), []).unwrap();
        assert_eq!((out.width(), out.height()), (5, 5));
    }

    #[test]
    fn pixelate_averages_blocks() {
        let mut pm = Pixmap::new(2, 1).unwrap();
        pm.data_mut()
            .copy_from_slice(&[0, 0, 0, 255, 200, 100, 50, 255]);
        pixelate(&mut pm, Rect::from_xywh(0.0, 0.0, 2.0, 1.0), 2);
        assert_eq!(px(&pm, 0, 0), [100, 50, 25, 255]);
        assert_eq!(px(&pm, 1, 0), [100, 50, 25, 255]);
    }

    #[test]
    fn arrow_head_points_at_target() {
        let (base, tri) = arrow_head(Pt::new(0.0, 0.0), Pt::new(100.0, 0.0), 2.0);
        assert_eq!(tri[0], Pt::new(100.0, 0.0));
        assert!(base.x < 100.0 && base.y.abs() < 1e-4);
        assert!((tri[1].y + tri[2].y).abs() < 1e-4, "head is symmetric");
    }
}
