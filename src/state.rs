//! Interaction state machine, independent of windowing so it can be unit-tested.
//! All points are image pixels of monitor `active`.

use crate::annotation::{Annotation, Tool};
use crate::geom::{Pt, Rect};

pub const PALETTE: [[u8; 4]; 6] = [
    [235, 64, 52, 255],   // red
    [255, 176, 0, 255],   // amber
    [46, 204, 64, 255],   // green
    [0, 122, 255, 255],   // blue
    [20, 20, 20, 255],    // black
    [255, 255, 255, 255], // white
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
}

impl Handle {
    pub const ALL: [Handle; 8] = [
        Handle::NW,
        Handle::N,
        Handle::NE,
        Handle::E,
        Handle::SE,
        Handle::S,
        Handle::SW,
        Handle::W,
    ];

    pub fn pos(self, r: &Rect) -> Pt {
        let (mx, my) = ((r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0);
        match self {
            Handle::NW => Pt::new(r.x0, r.y0),
            Handle::N => Pt::new(mx, r.y0),
            Handle::NE => Pt::new(r.x1, r.y0),
            Handle::E => Pt::new(r.x1, my),
            Handle::SE => Pt::new(r.x1, r.y1),
            Handle::S => Pt::new(mx, r.y1),
            Handle::SW => Pt::new(r.x0, r.y1),
            Handle::W => Pt::new(r.x0, my),
        }
    }

    fn apply(self, orig: &Rect, p: Pt) -> Rect {
        let (mut x0, mut y0, mut x1, mut y1) = (orig.x0, orig.y0, orig.x1, orig.y1);
        match self {
            Handle::N => y0 = p.y,
            Handle::S => y1 = p.y,
            Handle::E => x1 = p.x,
            Handle::W => x0 = p.x,
            Handle::NE => (y0, x1) = (p.y, p.x),
            Handle::NW => (y0, x0) = (p.y, p.x),
            Handle::SE => (y1, x1) = (p.y, p.x),
            Handle::SW => (y1, x0) = (p.y, p.x),
        }
        Rect::from_points(Pt::new(x0, y0), Pt::new(x1, y1))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hover {
    Outside,
    Inside,
    Handle(Handle),
}

#[derive(Clone, Debug)]
enum Drag {
    None,
    NewSelection { anchor: Pt },
    Move { grab: Pt, orig: Rect },
    Resize { handle: Handle, orig: Rect },
    Draw,
}

pub struct State {
    /// Monitor index the selection/annotations live on.
    pub active: Option<usize>,
    pub selection: Option<Rect>,
    pub annotations: Vec<Annotation>,
    redo: Vec<Annotation>,
    /// Annotation currently being drawn.
    pub drawing: Option<Annotation>,
    pub tool: Tool,
    pub color: usize,
    /// Stroke width in logical points; scaled per monitor on use.
    pub width: f32,
    drag: Drag,
    /// Size of the active monitor's image, for clamping.
    bounds: (f32, f32),
}

impl Default for State {
    fn default() -> Self {
        Self {
            active: None,
            selection: None,
            annotations: Vec::new(),
            redo: Vec::new(),
            drawing: None,
            tool: Tool::Select,
            color: 0,
            width: 3.0,
            drag: Drag::None,
            bounds: (0.0, 0.0),
        }
    }
}

impl State {
    pub fn is_dragging(&self) -> bool {
        !matches!(self.drag, Drag::None)
    }

    pub fn is_selecting(&self) -> bool {
        matches!(self.drag, Drag::NewSelection { .. })
    }

    /// What the pointer is over; `tol` is the handle grab radius in image px.
    pub fn hover(&self, monitor: usize, p: Pt, tol: f32) -> Hover {
        let Some(sel) = self.selection.filter(|_| self.active == Some(monitor)) else {
            return Hover::Outside;
        };
        if let Some(h) = Handle::ALL.into_iter().find(|h| h.pos(&sel).dist(p) <= tol) {
            return Hover::Handle(h);
        }
        if sel.contains(p) {
            Hover::Inside
        } else {
            Hover::Outside
        }
    }

    /// Pointer pressed on `monitor` at image point `p`.
    /// `bounds` is that monitor's image size, `px_scale` its image px per logical pt.
    pub fn press(&mut self, monitor: usize, p: Pt, tol: f32, bounds: (f32, f32), px_scale: f32) {
        let same_monitor = self.active == Some(monitor);
        if same_monitor && self.selection.is_some() && self.tool != Tool::Select {
            let width = self.width * px_scale;
            self.drawing = Annotation::begin(self.tool, p, PALETTE[self.color], width);
            self.drag = Drag::Draw;
            return;
        }
        self.drag = match (self.selection, self.hover(monitor, p, tol)) {
            (Some(orig), Hover::Handle(handle)) => Drag::Resize { handle, orig },
            (Some(orig), Hover::Inside) => Drag::Move { grab: p, orig },
            _ => {
                if !same_monitor {
                    // Annotations are in this monitor's pixel space; they don't carry over.
                    self.annotations.clear();
                    self.redo.clear();
                }
                self.active = Some(monitor);
                self.bounds = bounds;
                self.selection = None;
                Drag::NewSelection { anchor: p }
            }
        };
    }

    pub fn drag_to(&mut self, p: Pt) {
        let (bw, bh) = self.bounds;
        let p = Pt::new(p.x.clamp(0.0, bw), p.y.clamp(0.0, bh));
        match &self.drag {
            Drag::None => {}
            Drag::NewSelection { anchor } => {
                self.selection = Some(Rect::from_points(*anchor, p));
            }
            Drag::Move { grab, orig } => {
                let dx = (p.x - grab.x).clamp(-orig.x0, bw - orig.x1);
                let dy = (p.y - grab.y).clamp(-orig.y0, bh - orig.y1);
                self.selection = Some(orig.translate(dx, dy));
            }
            Drag::Resize { handle, orig } => {
                self.selection = Some(handle.apply(orig, p));
            }
            Drag::Draw => {
                if let Some(a) = &mut self.drawing {
                    a.update(p);
                }
            }
        }
    }

    pub fn release(&mut self, p: Pt) {
        self.drag_to(p);
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::Draw => {
                if let Some(a) = self.drawing.take().filter(Annotation::is_meaningful) {
                    self.annotations.push(a);
                    self.redo.clear();
                }
            }
            Drag::NewSelection { .. } | Drag::Resize { .. } => {
                // Drop degenerate selections (plain click).
                if self
                    .selection
                    .is_some_and(|s| s.width() < 2.0 || s.height() < 2.0)
                {
                    self.selection = None;
                }
            }
            Drag::Move { .. } | Drag::None => {}
        }
    }

    /// Esc: abort the current gesture if any. Returns `false` if there was
    /// nothing to cancel (caller should then quit).
    pub fn cancel(&mut self) -> bool {
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::None => false,
            Drag::Draw => {
                self.drawing = None;
                true
            }
            Drag::Move { orig, .. } | Drag::Resize { orig, .. } => {
                self.selection = Some(orig);
                true
            }
            Drag::NewSelection { .. } => {
                self.selection = None;
                true
            }
        }
    }

    pub fn undo(&mut self) {
        if let Some(a) = self.annotations.pop() {
            self.redo.push(a);
        }
    }

    pub fn redo(&mut self) {
        if let Some(a) = self.redo.pop() {
            self.annotations.push(a);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.annotations.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn adjust_width(&mut self, delta: f32) {
        self.width = (self.width + delta).clamp(1.0, 40.0);
    }

    /// Finished + in-progress annotations, in paint order.
    pub fn all_annotations(&self) -> impl Iterator<Item = &Annotation> {
        self.annotations.iter().chain(self.drawing.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: (f32, f32) = (1000.0, 800.0);

    fn select(s: &mut State, a: Pt, b: Pt) {
        s.press(0, a, 8.0, B, 1.0);
        s.drag_to(b);
        s.release(b);
    }

    #[test]
    fn drag_creates_normalized_selection() {
        let mut s = State::default();
        select(&mut s, Pt::new(300.0, 200.0), Pt::new(100.0, 50.0));
        assert_eq!(
            s.selection,
            Some(Rect::from_xywh(100.0, 50.0, 200.0, 150.0))
        );
    }

    #[test]
    fn click_without_drag_clears_selection() {
        let mut s = State::default();
        select(&mut s, Pt::new(10.0, 10.0), Pt::new(10.0, 10.0));
        assert_eq!(s.selection, None);
    }

    #[test]
    fn move_is_clamped_to_bounds() {
        let mut s = State::default();
        select(&mut s, Pt::new(100.0, 100.0), Pt::new(200.0, 200.0));
        s.press(0, Pt::new(150.0, 150.0), 8.0, B, 1.0);
        s.release(Pt::new(-500.0, 150.0));
        assert_eq!(s.selection, Some(Rect::from_xywh(0.0, 100.0, 100.0, 100.0)));
    }

    #[test]
    fn resize_via_corner_handle() {
        let mut s = State::default();
        select(&mut s, Pt::new(100.0, 100.0), Pt::new(200.0, 200.0));
        s.press(0, Pt::new(201.0, 199.0), 8.0, B, 1.0);
        s.release(Pt::new(300.0, 250.0));
        assert_eq!(
            s.selection,
            Some(Rect::from_xywh(100.0, 100.0, 200.0, 150.0))
        );
    }

    #[test]
    fn esc_restores_selection_mid_move() {
        let mut s = State::default();
        select(&mut s, Pt::new(100.0, 100.0), Pt::new(200.0, 200.0));
        let before = s.selection;
        s.press(0, Pt::new(150.0, 150.0), 8.0, B, 1.0);
        s.drag_to(Pt::new(400.0, 400.0));
        assert!(s.cancel());
        assert_eq!(s.selection, before);
        assert!(!s.cancel(), "nothing left to cancel → quit");
    }

    #[test]
    fn draw_undo_redo() {
        let mut s = State::default();
        select(&mut s, Pt::new(0.0, 0.0), Pt::new(500.0, 500.0));
        s.tool = Tool::Arrow;
        s.press(0, Pt::new(10.0, 10.0), 8.0, B, 2.0);
        s.drag_to(Pt::new(100.0, 100.0));
        assert!(s.drawing.is_some());
        s.release(Pt::new(100.0, 100.0));
        assert_eq!(s.annotations.len(), 1);
        assert_eq!(s.annotations[0].width, 6.0, "width scaled to image px");
        s.undo();
        assert!(s.annotations.is_empty() && s.can_redo());
        s.redo();
        assert_eq!(s.annotations.len(), 1);
    }

    #[test]
    fn new_drawing_clears_redo() {
        let mut s = State::default();
        select(&mut s, Pt::new(0.0, 0.0), Pt::new(500.0, 500.0));
        s.tool = Tool::Line;
        for _ in 0..2 {
            s.press(0, Pt::new(10.0, 10.0), 8.0, B, 1.0);
            s.release(Pt::new(50.0, 50.0));
        }
        s.undo();
        s.press(0, Pt::new(10.0, 10.0), 8.0, B, 1.0);
        s.release(Pt::new(60.0, 60.0));
        assert!(!s.can_redo());
    }

    #[test]
    fn switching_monitor_drops_annotations() {
        let mut s = State::default();
        select(&mut s, Pt::new(0.0, 0.0), Pt::new(500.0, 500.0));
        s.tool = Tool::Line;
        s.press(0, Pt::new(10.0, 10.0), 8.0, B, 1.0);
        s.release(Pt::new(50.0, 50.0));
        s.press(1, Pt::new(10.0, 10.0), 8.0, B, 1.0);
        assert!(s.annotations.is_empty());
        assert_eq!(s.active, Some(1));
    }
}
