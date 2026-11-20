//! Small geometry helpers shared by state, rendering and hit-testing.

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

impl Pt {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn dist(self, o: Pt) -> f32 {
        (self.x - o.x).hypot(self.y - o.y)
    }
}

/// Axis-aligned rectangle, always normalized (`x0 <= x1`, `y0 <= y1`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Rect {
    pub fn from_points(a: Pt, b: Pt) -> Self {
        Self {
            x0: a.x.min(b.x),
            y0: a.y.min(b.y),
            x1: a.x.max(b.x),
            y1: a.y.max(b.y),
        }
    }

    pub fn from_xywh(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self::from_points(Pt::new(x, y), Pt::new(x + w, y + h))
    }

    pub fn width(&self) -> f32 {
        self.x1 - self.x0
    }

    pub fn height(&self) -> f32 {
        self.y1 - self.y0
    }

    pub fn contains(&self, p: Pt) -> bool {
        p.x >= self.x0 && p.x <= self.x1 && p.y >= self.y0 && p.y <= self.y1
    }

    pub fn translate(&self, dx: f32, dy: f32) -> Self {
        Self {
            x0: self.x0 + dx,
            y0: self.y0 + dy,
            x1: self.x1 + dx,
            y1: self.y1 + dy,
        }
    }

    pub fn scale(&self, s: f32) -> Self {
        Self {
            x0: self.x0 * s,
            y0: self.y0 * s,
            x1: self.x1 * s,
            y1: self.y1 * s,
        }
    }

    /// Clamp into `[0, w] x [0, h]`.
    pub fn clamp_to(&self, w: f32, h: f32) -> Self {
        Self {
            x0: self.x0.clamp(0.0, w),
            y0: self.y0.clamp(0.0, h),
            x1: self.x1.clamp(0.0, w),
            y1: self.y1.clamp(0.0, h),
        }
    }

    /// Integer pixel rect (rounded), `None` if empty.
    pub fn to_int(self) -> Option<tiny_skia::IntRect> {
        let x0 = self.x0.round() as i32;
        let y0 = self.y0.round() as i32;
        let x1 = self.x1.round() as i32;
        let y1 = self.y1.round() as i32;
        tiny_skia::IntRect::from_ltrb(x0, y0, x1, y1)
    }

    pub fn to_skia(self) -> Option<tiny_skia::Rect> {
        tiny_skia::Rect::from_ltrb(self.x0, self.y0, self.x1, self.y1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_normalizes() {
        let r = Rect::from_points(Pt::new(10.0, 20.0), Pt::new(2.0, 5.0));
        assert_eq!((r.x0, r.y0, r.x1, r.y1), (2.0, 5.0, 10.0, 20.0));
    }

    #[test]
    fn empty_rect_has_no_int() {
        let r = Rect::from_points(Pt::new(3.0, 3.0), Pt::new(3.2, 9.0));
        assert!(r.to_int().is_none());
    }
}
