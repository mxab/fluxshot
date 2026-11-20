//! Hand-drawn toolbar. Pure layout + painting in window pixels; no widget toolkit.

use tiny_skia::{FillRule, LineCap, LineJoin, PathBuilder, Pixmap, Stroke, Transform};

use crate::annotation::Tool;
use crate::geom::{Pt, Rect};
use crate::render::paint;
use crate::state::{PALETTE, State};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Tool(Tool),
    Color(usize),
    Undo,
    Redo,
    Copy,
    Save,
    Close,
}

const BTN: f32 = 32.0;
const GAP: f32 = 2.0;
const SEP: f32 = 10.0;
const PAD: f32 = 5.0;
const MARGIN: f32 = 8.0;

fn groups() -> [Vec<Button>; 3] {
    let mut tools = vec![Button::Tool(Tool::Select)];
    tools.extend(Tool::DRAWING.map(Button::Tool));
    [
        tools,
        (0..PALETTE.len()).map(Button::Color).collect(),
        vec![
            Button::Undo,
            Button::Redo,
            Button::Copy,
            Button::Save,
            Button::Close,
        ],
    ]
}

pub struct Layout {
    pub bar: Rect,
    pub buttons: Vec<(Button, Rect)>,
}

/// Place the toolbar below the selection (window px), else above, else inside.
pub fn layout(sel: Rect, win_w: f32, win_h: f32, ui: f32) -> Layout {
    let groups = groups();
    let n: usize = groups.iter().map(Vec::len).sum();
    let inner_w =
        n as f32 * BTN + (n - groups.len()) as f32 * GAP + (groups.len() - 1) as f32 * SEP;
    let (w, h) = ((inner_w + 2.0 * PAD) * ui, (BTN + 2.0 * PAD) * ui);
    let m = MARGIN * ui;

    let y = if sel.y1 + m + h <= win_h {
        sel.y1 + m
    } else if sel.y0 - m - h >= 0.0 {
        sel.y0 - m - h
    } else {
        (sel.y1 - m - h).max(0.0)
    };
    let cx = (sel.x0 + sel.x1) / 2.0;
    let x = (cx - w / 2.0).clamp(0.0, (win_w - w).max(0.0));

    let mut buttons = Vec::with_capacity(n);
    let mut bx = x + PAD * ui;
    for (gi, g) in groups.iter().enumerate() {
        if gi > 0 {
            bx += (SEP - GAP) * ui;
        }
        for &b in g {
            buttons.push((b, Rect::from_xywh(bx, y + PAD * ui, BTN * ui, BTN * ui)));
            bx += (BTN + GAP) * ui;
        }
    }
    Layout {
        bar: Rect::from_xywh(x, y, w, h),
        buttons,
    }
}

impl Layout {
    pub fn hit(&self, p: Pt) -> Option<Button> {
        self.buttons
            .iter()
            .find(|(_, r)| r.contains(p))
            .map(|(b, _)| *b)
    }

    pub fn contains(&self, p: Pt) -> bool {
        self.bar.contains(p)
    }

    pub fn draw(&self, pm: &mut Pixmap, state: &State, hovered: Option<Button>, ui: f32) {
        fill_round_rect(pm, self.bar, 8.0 * ui, [32, 33, 36, 240]);
        for &(b, r) in &self.buttons {
            let active = match b {
                Button::Tool(t) => state.tool == t,
                Button::Color(i) => state.color == i,
                _ => false,
            };
            if active && !matches!(b, Button::Color(_)) {
                fill_round_rect(pm, r, 6.0 * ui, [0, 122, 255, 255]);
            } else if hovered == Some(b) {
                fill_round_rect(pm, r, 6.0 * ui, [70, 72, 78, 255]);
            }
            let enabled = match b {
                Button::Undo => state.can_undo(),
                Button::Redo => state.can_redo(),
                _ => true,
            };
            let fg = if enabled {
                [235, 235, 240, 255]
            } else {
                [110, 110, 115, 255]
            };
            draw_icon(pm, b, r, fg, active, state.width, ui);
        }
    }
}

fn fill_round_rect(pm: &mut Pixmap, r: Rect, rad: f32, color: [u8; 4]) {
    let rad = rad.min(r.width() / 2.0).min(r.height() / 2.0);
    let k = 0.4477 * rad; // bezier offset for a quarter circle
    let mut pb = PathBuilder::new();
    pb.move_to(r.x0 + rad, r.y0);
    pb.line_to(r.x1 - rad, r.y0);
    pb.cubic_to(r.x1 - k, r.y0, r.x1, r.y0 + k, r.x1, r.y0 + rad);
    pb.line_to(r.x1, r.y1 - rad);
    pb.cubic_to(r.x1, r.y1 - k, r.x1 - k, r.y1, r.x1 - rad, r.y1);
    pb.line_to(r.x0 + rad, r.y1);
    pb.cubic_to(r.x0 + k, r.y1, r.x0, r.y1 - k, r.x0, r.y1 - rad);
    pb.line_to(r.x0, r.y0 + rad);
    pb.cubic_to(r.x0, r.y0 + k, r.x0 + k, r.y0, r.x0 + rad, r.y0);
    pb.close();
    if let Some(p) = pb.finish() {
        pm.fill_path(
            &p,
            &paint(color),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

/// Icons are authored in a 24×24 box centered in the button.
fn draw_icon(pm: &mut Pixmap, b: Button, r: Rect, fg: [u8; 4], active: bool, width: f32, ui: f32) {
    let s = ui;
    let ox = (r.x0 + r.x1) / 2.0 - 12.0 * s;
    let oy = (r.y0 + r.y1) / 2.0 - 12.0 * s;
    let ts = Transform::from_row(s, 0.0, 0.0, s, ox, oy);
    let stroke = Stroke {
        width: 1.8,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..Default::default()
    };
    let fgp = paint(fg);
    let mut st = |pb: PathBuilder, w: f32, color: [u8; 4]| {
        if let Some(p) = pb.finish() {
            let s = Stroke {
                width: w,
                ..stroke.clone()
            };
            pm.stroke_path(&p, &paint(color), &s, ts, None);
        }
    };
    let mut pb = PathBuilder::new();
    match b {
        Button::Tool(Tool::Select) => {
            // Dashed-corner selection frame.
            for (x, y, dx, dy) in [
                (4., 4., 1., 1.),
                (20., 4., -1., 1.),
                (4., 20., 1., -1.),
                (20., 20., -1., -1.),
            ] {
                pb.move_to(x + 5.0 * dx, y);
                pb.line_to(x, y);
                pb.line_to(x, y + 5.0 * dy);
            }
            st(pb, 1.8, fg);
        }
        Button::Tool(Tool::Pen) => {
            pb.move_to(4.0, 17.0);
            pb.cubic_to(8.0, 5.0, 11.0, 5.0, 12.0, 12.0);
            pb.cubic_to(13.0, 19.0, 16.0, 19.0, 20.0, 7.0);
            st(pb, 1.8, fg);
        }
        Button::Tool(Tool::Line) => {
            pb.move_to(5.0, 19.0);
            pb.line_to(19.0, 5.0);
            st(pb, 1.8, fg);
        }
        Button::Tool(Tool::Arrow) => {
            pb.move_to(5.0, 19.0);
            pb.line_to(18.0, 6.0);
            pb.move_to(10.0, 5.5);
            pb.line_to(18.5, 5.5);
            pb.line_to(18.5, 14.0);
            st(pb, 1.8, fg);
        }
        Button::Tool(Tool::Rect) => {
            pb.push_rect(tiny_skia::Rect::from_xywh(4.5, 6.5, 15.0, 11.0).unwrap());
            st(pb, 1.8, fg);
        }
        Button::Tool(Tool::Ellipse) => {
            pb.push_oval(tiny_skia::Rect::from_xywh(4.0, 6.0, 16.0, 12.0).unwrap());
            st(pb, 1.8, fg);
        }
        Button::Tool(Tool::Marker) => {
            pb.move_to(5.0, 15.0);
            pb.line_to(19.0, 9.0);
            let mut c = fg;
            c[3] = 110;
            st(pb, 6.0, c);
        }
        Button::Tool(Tool::Pixelate) => {
            for (i, (x, y)) in [(5., 5.), (12., 5.), (5., 12.), (12., 12.)]
                .into_iter()
                .enumerate()
            {
                let mut c = fg;
                c[3] = if i == 0 || i == 3 { 255 } else { 110 };
                if let Some(p) = tiny_skia::Rect::from_xywh(x, y, 7.0, 7.0) {
                    pm.fill_rect(p, &paint(c), ts, None);
                }
            }
        }
        Button::Color(i) => {
            let c = PALETTE[i];
            // Swatch radius hints at the current stroke width when selected.
            let rad = if active {
                (4.0 + width * 0.6).min(10.0)
            } else {
                6.5
            };
            if let Some(p) = PathBuilder::from_circle(12.0, 12.0, rad) {
                pm.fill_path(&p, &paint(c), FillRule::Winding, ts, None);
                pm.stroke_path(
                    &p,
                    &paint([255, 255, 255, 90]),
                    &Stroke {
                        width: 1.0,
                        ..stroke.clone()
                    },
                    ts,
                    None,
                );
            }
            if active && let Some(p) = PathBuilder::from_circle(12.0, 12.0, 11.0) {
                pm.stroke_path(
                    &p,
                    &fgp,
                    &Stroke {
                        width: 1.5,
                        ..stroke.clone()
                    },
                    ts,
                    None,
                );
            }
        }
        Button::Undo | Button::Redo => {
            let flip = b == Button::Redo;
            let x = |v: f32| if flip { 24.0 - v } else { v };
            pb.move_to(x(9.0), 6.0);
            pb.line_to(x(5.0), 10.0);
            pb.line_to(x(9.0), 14.0);
            pb.move_to(x(5.0), 10.0);
            pb.line_to(x(14.0), 10.0);
            pb.cubic_to(x(21.0), 10.0, x(21.0), 19.0, x(14.0), 19.0);
            pb.line_to(x(10.0), 19.0);
            st(pb, 1.8, fg);
        }
        Button::Copy => {
            pb.push_rect(tiny_skia::Rect::from_xywh(8.5, 8.5, 11.0, 11.0).unwrap());
            pb.move_to(15.5, 5.0);
            pb.line_to(4.5, 5.0);
            pb.line_to(4.5, 15.5);
            st(pb, 1.8, fg);
        }
        Button::Save => {
            pb.move_to(12.0, 4.0);
            pb.line_to(12.0, 15.0);
            pb.move_to(7.5, 10.5);
            pb.line_to(12.0, 15.0);
            pb.line_to(16.5, 10.5);
            pb.move_to(4.5, 15.0);
            pb.line_to(4.5, 19.5);
            pb.line_to(19.5, 19.5);
            pb.line_to(19.5, 15.0);
            st(pb, 1.8, fg);
        }
        Button::Close => {
            pb.move_to(6.0, 6.0);
            pb.line_to(18.0, 18.0);
            pb.move_to(18.0, 6.0);
            pb.line_to(6.0, 18.0);
            st(pb, 1.8, fg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_goes_below_then_above_then_inside() {
        let below = layout(
            Rect::from_xywh(100.0, 100.0, 300.0, 200.0),
            2000.0,
            1000.0,
            1.0,
        );
        assert!(below.bar.y0 >= 300.0);
        let above = layout(
            Rect::from_xywh(100.0, 500.0, 300.0, 490.0),
            2000.0,
            1000.0,
            1.0,
        );
        assert!(above.bar.y1 <= 500.0);
        let inside = layout(
            Rect::from_xywh(0.0, 0.0, 2000.0, 1000.0),
            2000.0,
            1000.0,
            1.0,
        );
        assert!(inside.bar.y1 <= 1000.0 && inside.bar.y0 >= 0.0);
    }

    #[test]
    fn toolbar_stays_on_screen_horizontally() {
        let l = layout(Rect::from_xywh(0.0, 100.0, 10.0, 10.0), 2000.0, 1000.0, 2.0);
        assert!(l.bar.x0 >= 0.0);
        let first = l.buttons[0].1;
        assert_eq!(
            l.hit(Pt::new(first.x0 + 1.0, first.y0 + 1.0)),
            Some(Button::Tool(Tool::Select))
        );
    }
}
