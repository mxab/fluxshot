//! Annotation model. All coordinates are in *image pixels* of the monitor
//! capture the annotation belongs to, so export is exact on HiDPI screens.

use crate::geom::{Pt, Rect};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    /// Move / resize the selection region.
    Select,
    Pen,
    Line,
    Arrow,
    Rect,
    Ellipse,
    Marker,
    Pixelate,
}

impl Tool {
    pub const DRAWING: [Tool; 7] = [
        Tool::Pen,
        Tool::Line,
        Tool::Arrow,
        Tool::Rect,
        Tool::Ellipse,
        Tool::Marker,
        Tool::Pixelate,
    ];
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Pen(Vec<Pt>),
    Marker(Vec<Pt>),
    Line(Pt, Pt),
    Arrow(Pt, Pt),
    Rect(Pt, Pt),
    Ellipse(Pt, Pt),
    Pixelate(Pt, Pt),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    pub shape: Shape,
    /// Straight (non-premultiplied) RGBA.
    pub color: [u8; 4],
    /// Stroke width in image pixels.
    pub width: f32,
}

impl Annotation {
    /// Start a new annotation for `tool` at `p`. Returns `None` for `Tool::Select`.
    pub fn begin(tool: Tool, p: Pt, color: [u8; 4], width: f32) -> Option<Self> {
        let shape = match tool {
            Tool::Select => return None,
            Tool::Pen => Shape::Pen(vec![p]),
            Tool::Marker => Shape::Marker(vec![p]),
            Tool::Line => Shape::Line(p, p),
            Tool::Arrow => Shape::Arrow(p, p),
            Tool::Rect => Shape::Rect(p, p),
            Tool::Ellipse => Shape::Ellipse(p, p),
            Tool::Pixelate => Shape::Pixelate(p, p),
        };
        Some(Self {
            shape,
            color,
            width,
        })
    }

    /// Extend the annotation while the pointer is dragged.
    pub fn update(&mut self, p: Pt) {
        match &mut self.shape {
            Shape::Pen(pts) | Shape::Marker(pts) => {
                if pts.last().is_none_or(|l| l.dist(p) >= 1.0) {
                    pts.push(p);
                }
            }
            Shape::Line(_, b)
            | Shape::Arrow(_, b)
            | Shape::Rect(_, b)
            | Shape::Ellipse(_, b)
            | Shape::Pixelate(_, b) => *b = p,
        }
    }

    /// Whether the annotation would draw anything (filters accidental clicks).
    pub fn is_meaningful(&self) -> bool {
        match &self.shape {
            // A single click with the pen leaves a dot — that's intentional.
            Shape::Pen(pts) | Shape::Marker(pts) => !pts.is_empty(),
            Shape::Line(a, b) | Shape::Arrow(a, b) => a.dist(*b) >= 2.0,
            Shape::Rect(a, b) | Shape::Ellipse(a, b) | Shape::Pixelate(a, b) => {
                let r = Rect::from_points(*a, *b);
                r.width() >= 2.0 && r.height() >= 2.0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_without_drag_is_discarded_for_shapes() {
        let a = Annotation::begin(Tool::Rect, Pt::new(5.0, 5.0), [0; 4], 3.0).unwrap();
        assert!(!a.is_meaningful());
        let p = Annotation::begin(Tool::Pen, Pt::new(5.0, 5.0), [0; 4], 3.0).unwrap();
        assert!(p.is_meaningful());
    }

    #[test]
    fn pen_skips_subpixel_moves() {
        let mut a = Annotation::begin(Tool::Pen, Pt::new(0.0, 0.0), [0; 4], 3.0).unwrap();
        a.update(Pt::new(0.3, 0.0));
        a.update(Pt::new(2.0, 0.0));
        let Shape::Pen(pts) = &a.shape else { panic!() };
        assert_eq!(pts.len(), 2);
    }
}
