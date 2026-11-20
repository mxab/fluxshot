//! winit overlay: one borderless, always-on-top window per monitor showing the
//! frozen capture. Rendering is CPU (tiny-skia) → softbuffer.

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Instant;

use anyhow::Result;
use tiny_skia::{Pixmap, Transform};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::monitor::MonitorHandle;
use winit::window::{CursorIcon, Fullscreen, Window, WindowId, WindowLevel};

use crate::annotation::Tool;
use crate::capture::MonitorShot;
use crate::geom::{Pt, Rect};
use crate::render::{self, blit_scaled, render_export};
use crate::state::{Handle, Hover, PALETTE, State};
use crate::toolbar::{self, Button};

const HANDLE_RADIUS: f32 = 4.0; // logical
const HANDLE_GRAB: f32 = 10.0; // logical
const ACCENT: [u8; 4] = [0, 122, 255, 255];

pub struct Options {
    pub t0: Instant,
    pub trace: bool,
}

struct View {
    window: Rc<Window>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    /// Frame buffer in window pixels.
    frame: Pixmap,
    /// Dimmed capture, pre-scaled to window size.
    dimmed: Pixmap,
    /// Window pixels per image pixel (1.0 unless the OS sized the window oddly).
    scale: f32,
    /// Window pixels per logical point.
    ui: f32,
    cursor: Pt,
    hovered: Option<Button>,
}

#[derive(Clone, Copy)]
enum Finish {
    Copy,
    Save,
}

/// What the user asked for; the caller performs the actual output.
pub enum Outcome {
    Copy(Pixmap),
    Save(Pixmap),
}

struct App {
    shots: Vec<MonitorShot>,
    views: Vec<View>,
    state: State,
    mods: ModifiersState,
    opts: Options,
    /// Last view the pointer was over (target for "no selection" export).
    last_view: usize,
    outcome: Option<Result<Outcome>>,
}

/// Show the overlay and block until the user copies, saves or cancels.
/// Returns the exported image and what to do with it, or `None` on cancel.
pub fn run(shots: Vec<MonitorShot>, opts: Options) -> Result<Option<Outcome>> {
    #[allow(unused_mut)]
    let mut builder = EventLoop::builder();
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        // No Dock icon / menu bar takeover.
        builder.with_activation_policy(ActivationPolicy::Accessory);
    }
    let event_loop = builder.build()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let last_view = shots.iter().position(|s| s.primary).unwrap_or(0);
    let mut app = App {
        shots,
        views: Vec::new(),
        state: State::default(),
        mods: ModifiersState::empty(),
        opts,
        last_view,
        outcome: None,
    };
    event_loop.run_app(&mut app)?;
    app.outcome.transpose()
}

impl App {
    fn create_views(&mut self, el: &ActiveEventLoop) -> Result<()> {
        let linux_monitors: Vec<Option<MonitorHandle>> = if cfg!(target_os = "linux") {
            let handles: Vec<MonitorHandle> = el.available_monitors().collect();
            let infos: Vec<MonitorInfo> = handles
                .iter()
                .map(|m| MonitorInfo {
                    name: m.name().unwrap_or_default(),
                    x: m.position().x,
                    y: m.position().y,
                    width: m.size().width,
                    height: m.size().height,
                })
                .collect();
            match_monitors(&self.shots, &infos)
                .into_iter()
                .map(|j| j.map(|j| handles[j].clone()))
                .collect()
        } else {
            Vec::new()
        };
        for (i, shot) in self.shots.iter().enumerate() {
            let attrs = Window::default_attributes()
                .with_title("FluxShot")
                .with_decorations(false)
                .with_resizable(false)
                .with_window_level(WindowLevel::AlwaysOnTop)
                .with_visible(false);
            // xcap reports logical points on macOS and physical pixels on Windows.
            // On Linux its coordinates are divided by a global scale factor and
            // Wayland can't position windows at all, so instead go borderless
            // fullscreen on the matching output (also covers panels/docks).
            let attrs = if cfg!(target_os = "macos") {
                attrs
                    .with_position(LogicalPosition::new(shot.x, shot.y))
                    .with_inner_size(LogicalSize::new(shot.width, shot.height))
            } else if let Some(Some(m)) = linux_monitors.get(i) {
                // Also start at the monitor's geometry: X11 only applies
                // fullscreen once the window is mapped.
                attrs
                    .with_position(m.position())
                    .with_inner_size(m.size())
                    .with_fullscreen(Some(Fullscreen::Borderless(Some(m.clone()))))
            } else {
                attrs
                    .with_position(PhysicalPosition::new(shot.x, shot.y))
                    .with_inner_size(PhysicalSize::new(shot.image.width(), shot.image.height()))
            };
            let window = Rc::new(el.create_window(attrs)?);
            #[cfg(target_os = "macos")]
            crate::platform_macos::raise_overlay(&window);
            window.set_cursor(CursorIcon::Crosshair);

            let size = window.inner_size();
            let (frame, dimmed, scale) = fit_to_window(&shot.image, size.width, size.height);
            let context =
                softbuffer::Context::new(window.clone()).map_err(|e| anyhow::anyhow!("{e}"))?;
            let surface = softbuffer::Surface::new(&context, window.clone())
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            self.views.push(View {
                ui: window.scale_factor() as f32,
                window,
                surface,
                frame,
                dimmed,
                scale,
                cursor: Pt::default(),
                hovered: None,
            });
        }
        // Paint before showing so there is no flash of an empty window.
        for i in 0..self.views.len() {
            self.render(i);
            self.views[i].window.set_visible(true);
        }
        if let Some(v) = self.views.get(self.last_view) {
            v.window.focus_window();
        }
        if self.opts.trace {
            eprintln!("[trace] overlay visible after {:?}", self.opts.t0.elapsed());
        }
        Ok(())
    }

    /// The compositor/WM may size the window after creation (X11 applies
    /// fullscreen on map, Wayland configures asynchronously): re-fit buffers.
    fn resized(&mut self, vi: usize, w: u32, h: u32) {
        let v = &mut self.views[vi];
        if (w.max(1), h.max(1)) == (v.frame.width(), v.frame.height()) {
            return;
        }
        (v.frame, v.dimmed, v.scale) = fit_to_window(&self.shots[vi].image, w, h);
        v.window.request_redraw();
    }

    fn to_image(&self, vi: usize, p: Pt) -> Pt {
        let s = self.views[vi].scale;
        Pt::new(p.x / s, p.y / s)
    }

    fn toolbar(&self, vi: usize) -> Option<toolbar::Layout> {
        if self.state.active != Some(vi) || self.state.is_selecting() {
            return None;
        }
        let v = &self.views[vi];
        let sel = self.state.selection?.scale(v.scale);
        Some(toolbar::layout(
            sel,
            v.frame.width() as f32,
            v.frame.height() as f32,
            v.ui,
        ))
    }

    fn redraw_all(&self) {
        for v in &self.views {
            v.window.request_redraw();
        }
    }

    fn redraw_active(&self) {
        match self.state.active.and_then(|i| self.views.get(i)) {
            Some(v) => v.window.request_redraw(),
            None => self.redraw_all(),
        }
    }

    fn render(&mut self, vi: usize) {
        let toolbar = self.toolbar(vi);
        let v = &mut self.views[vi];
        v.frame.data_mut().copy_from_slice(v.dimmed.data());

        if let (Some(sel), true) = (self.state.selection, self.state.active == Some(vi)) {
            let shot = &self.shots[vi];
            // The preview of the selection IS the export image.
            if let Some(region) = sel.to_int()
                && let Some(img) = render_export(&shot.image, region, self.state.all_annotations())
            {
                blit_scaled(
                    &mut v.frame,
                    &img,
                    region.x() as f32 * v.scale,
                    region.y() as f32 * v.scale,
                    v.scale,
                );
            }
            draw_selection_chrome(
                v,
                sel.scale(v.scale),
                self.state.tool == Tool::Select && !self.state.is_selecting(),
            );
            if let Some(tb) = &toolbar {
                tb.draw(&mut v.frame, &self.state, v.hovered, v.ui);
            }
        }

        let (w, h) = (v.frame.width(), v.frame.height());
        let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) else {
            return;
        };
        if v.surface.resize(nw, nh).is_err() {
            return;
        }
        if let Ok(mut buf) = v.surface.buffer_mut() {
            render::to_softbuffer(v.frame.data(), &mut buf);
            let _ = buf.present();
        }
    }

    fn update_cursor(&mut self, vi: usize) {
        let toolbar = self.toolbar(vi);
        let v = &self.views[vi];
        let hovered = toolbar.as_ref().and_then(|t| t.hit(v.cursor));
        let over_bar = toolbar.as_ref().is_some_and(|t| t.contains(v.cursor));
        let icon = if over_bar {
            CursorIcon::Default
        } else if self.state.tool != Tool::Select
            && self.state.selection.is_some()
            && self.state.active == Some(vi)
        {
            CursorIcon::Crosshair
        } else {
            let tol = HANDLE_GRAB * v.ui / v.scale;
            match self.state.hover(vi, self.to_image(vi, v.cursor), tol) {
                Hover::Outside => CursorIcon::Crosshair,
                Hover::Inside => CursorIcon::Move,
                Hover::Handle(Handle::N | Handle::S) => CursorIcon::NsResize,
                Hover::Handle(Handle::E | Handle::W) => CursorIcon::EwResize,
                Hover::Handle(Handle::NW | Handle::SE) => CursorIcon::NwseResize,
                Hover::Handle(Handle::NE | Handle::SW) => CursorIcon::NeswResize,
            }
        };
        let v = &mut self.views[vi];
        v.window.set_cursor(icon);
        if v.hovered != hovered {
            v.hovered = hovered;
            v.window.request_redraw();
        }
    }

    fn button(&mut self, el: &ActiveEventLoop, b: Button) {
        match b {
            Button::Tool(t) => self.state.tool = t,
            Button::Color(i) => self.state.color = i,
            Button::Undo => self.state.undo(),
            Button::Redo => self.state.redo(),
            Button::Copy => return self.finish(el, Finish::Copy),
            Button::Save => return self.finish(el, Finish::Save),
            Button::Close => return el.exit(),
        }
        self.redraw_active();
    }

    fn key(&mut self, el: &ActiveEventLoop, ev: KeyEvent) {
        let cmd = self.mods.super_key() || self.mods.control_key();
        match ev.logical_key.as_ref() {
            Key::Named(NamedKey::Escape) => {
                if !self.state.cancel() {
                    el.exit();
                }
            }
            Key::Named(NamedKey::Enter) => return self.finish(el, Finish::Copy),
            Key::Character(c) => {
                let c = c.to_lowercase();
                match (cmd, c.as_str()) {
                    (true, "c") => return self.finish(el, Finish::Copy),
                    (true, "s") => return self.finish(el, Finish::Save),
                    (true, "z") if self.mods.shift_key() => self.state.redo(),
                    (true, "z") => self.state.undo(),
                    (true, "y") => self.state.redo(),
                    (false, "v") => self.state.tool = Tool::Select,
                    (false, "p") => self.state.tool = Tool::Pen,
                    (false, "l") => self.state.tool = Tool::Line,
                    (false, "a") => self.state.tool = Tool::Arrow,
                    (false, "r") => self.state.tool = Tool::Rect,
                    (false, "e") => self.state.tool = Tool::Ellipse,
                    (false, "m") => self.state.tool = Tool::Marker,
                    (false, "x") => self.state.tool = Tool::Pixelate,
                    (false, "+" | "=") => self.state.adjust_width(1.0),
                    (false, "-") => self.state.adjust_width(-1.0),
                    (false, d) => {
                        if let Some(i) = d
                            .parse::<usize>()
                            .ok()
                            .filter(|i| (1..=PALETTE.len()).contains(i))
                        {
                            self.state.color = i - 1;
                        }
                    }
                    _ => {}
                }
            }
            _ => return,
        }
        self.redraw_active();
    }

    fn finish(&mut self, el: &ActiveEventLoop, how: Finish) {
        // No selection → whole monitor under the pointer.
        let vi = self
            .state
            .active
            .filter(|_| self.state.selection.is_some())
            .unwrap_or(self.last_view);
        let shot = &self.shots[vi];
        let region = self
            .state
            .selection
            .filter(|_| self.state.active == Some(vi))
            .unwrap_or(Rect::from_xywh(
                0.0,
                0.0,
                shot.image.width() as f32,
                shot.image.height() as f32,
            ))
            .to_int();
        let Some(img) =
            region.and_then(|r| render_export(&shot.image, r, self.state.all_annotations()))
        else {
            return;
        };
        // Get out of the way immediately; output happens after the loop.
        for v in &self.views {
            v.window.set_visible(false);
        }
        self.outcome = Some(Ok(match how {
            Finish::Copy => Outcome::Copy(img),
            Finish::Save => Outcome::Save(img),
        }));
        el.exit();
    }
}

/// Frame buffer, dimmed backdrop and window-pixels-per-image-pixel scale for
/// a window of `w`×`h` showing `image`.
fn fit_to_window(image: &Pixmap, w: u32, h: u32) -> (Pixmap, Pixmap, f32) {
    let (w, h) = (w.max(1), h.max(1));
    let scale = w as f32 / image.width() as f32;
    let dimmed_img = render::dimmed(image, 0.45);
    let dimmed = if (w, h) == (image.width(), image.height()) {
        dimmed_img
    } else {
        let mut d = Pixmap::new(w, h).expect("window size");
        blit_scaled(&mut d, &dimmed_img, 0.0, 0.0, scale);
        d
    };
    (Pixmap::new(w, h).expect("window size"), dimmed, scale)
}

fn draw_selection_chrome(v: &mut View, sel: Rect, handles: bool) {
    let ui = v.ui;
    let id = Transform::identity();
    if let Some(r) = sel.to_skia() {
        let stroke = tiny_skia::Stroke {
            width: 1.5 * ui,
            ..Default::default()
        };
        let path = tiny_skia::PathBuilder::from_rect(r);
        v.frame
            .stroke_path(&path, &render::paint(ACCENT), &stroke, id, None);
    }
    if !handles {
        return;
    }
    for h in Handle::ALL {
        let p = h.pos(&sel);
        if let Some(c) = tiny_skia::PathBuilder::from_circle(p.x, p.y, HANDLE_RADIUS * ui) {
            v.frame.fill_path(
                &c,
                &render::paint([255, 255, 255, 255]),
                tiny_skia::FillRule::Winding,
                id,
                None,
            );
            let s = tiny_skia::Stroke {
                width: 1.5 * ui,
                ..Default::default()
            };
            v.frame
                .stroke_path(&c, &render::paint(ACCENT), &s, id, None);
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.views.is_empty()
            && let Err(e) = self.create_views(el)
        {
            self.outcome = Some(Err(e));
            el.exit();
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(vi) = self.views.iter().position(|v| v.window.id() == id) else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::RedrawRequested => self.render(vi),
            WindowEvent::Resized(size) => self.resized(vi, size.width, size.height),
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.views[vi].ui = scale_factor as f32;
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                let p = Pt::new(position.x as f32, position.y as f32);
                self.views[vi].cursor = p;
                if self.state.is_dragging() {
                    // Drags stay on the monitor they started on.
                    if let Some(ai) = self.state.active {
                        let ip = if ai == vi {
                            self.to_image(vi, p)
                        } else {
                            self.to_image(ai, self.views[ai].cursor)
                        };
                        self.state.drag_to(ip);
                        self.views[ai].window.request_redraw();
                    }
                } else {
                    self.last_view = vi;
                    self.update_cursor(vi);
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let cursor = self.views[vi].cursor;
                if let Some(tb) = self.toolbar(vi)
                    && tb.contains(cursor)
                {
                    if let Some(b) = tb.hit(cursor) {
                        self.button(el, b);
                    }
                    return;
                }
                let v = &self.views[vi];
                let shot = &self.shots[vi];
                let tol = HANDLE_GRAB * v.ui / v.scale;
                let bounds = (shot.image.width() as f32, shot.image.height() as f32);
                let prev_active = self.state.active;
                self.state
                    .press(vi, self.to_image(vi, cursor), tol, bounds, shot.scale);
                if prev_active != self.state.active {
                    self.redraw_all();
                } else {
                    self.redraw_active();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                if self.state.is_dragging()
                    && let Some(ai) = self.state.active
                {
                    let p = self.to_image(ai, self.views[ai].cursor);
                    self.state.release(p);
                    self.update_cursor(ai);
                    self.views[ai].window.request_redraw();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let d = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => (p.y / 10.0) as f32,
                };
                if d != 0.0 {
                    self.state.adjust_width(d.signum());
                    self.redraw_active();
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.key(el, event);
            }
            _ => {}
        }
    }
}

/// Physical geometry of a winit monitor, decoupled from winit for testing.
struct MonitorInfo {
    name: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

/// Pair each capture with a winit monitor: by output name first, then by
/// pixel size and position (xcap positions are scaled down on Linux, so
/// compare against both raw and scale-adjusted coordinates), then by order.
fn match_monitors(shots: &[MonitorShot], monitors: &[MonitorInfo]) -> Vec<Option<usize>> {
    let mut taken = vec![false; monitors.len()];
    let mut out = vec![None; shots.len()];
    fn claim(out: &mut [Option<usize>], taken: &mut [bool], i: usize, j: usize) {
        out[i] = Some(j);
        taken[j] = true;
    }
    for (i, s) in shots.iter().enumerate() {
        if s.name.is_empty() {
            continue;
        }
        if let Some(j) = (0..monitors.len()).find(|&j| !taken[j] && monitors[j].name == s.name) {
            claim(&mut out, &mut taken, i, j);
        }
    }
    for (i, s) in shots.iter().enumerate() {
        if out[i].is_some() {
            continue;
        }
        let dist = |m: &MonitorInfo| {
            let (px, py) = ((s.x as f32 * s.scale) as i64, (s.y as f32 * s.scale) as i64);
            let d_scaled = (m.x as i64 - px).abs() + (m.y as i64 - py).abs();
            let d_raw = (m.x as i64 - s.x as i64).abs() + (m.y as i64 - s.y as i64).abs();
            d_scaled.min(d_raw)
        };
        let best = (0..monitors.len())
            .filter(|&j| !taken[j])
            .filter(|&j| {
                monitors[j].width == s.image.width() && monitors[j].height == s.image.height()
            })
            .min_by_key(|&j| dist(&monitors[j]));
        if let Some(j) = best {
            claim(&mut out, &mut taken, i, j);
        }
    }
    for i in 0..shots.len() {
        if out[i].is_none()
            && let Some(j) = (0..monitors.len()).find(|&j| !taken[j])
        {
            claim(&mut out, &mut taken, i, j);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot(name: &str, x: i32, y: i32, w: u32, h: u32, scale: f32) -> MonitorShot {
        MonitorShot {
            name: name.into(),
            image: Pixmap::new(w, h).unwrap(),
            x,
            y,
            width: (w as f32 / scale) as u32,
            height: (h as f32 / scale) as u32,
            scale,
            primary: false,
        }
    }

    fn mon(name: &str, x: i32, y: i32, width: u32, height: u32) -> MonitorInfo {
        MonitorInfo {
            name: name.into(),
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn matches_by_name() {
        let shots = [
            shot("HDMI-1", 0, 0, 100, 50, 1.0),
            shot("DP-1", 100, 0, 100, 50, 1.0),
        ];
        let mons = [mon("DP-1", 100, 0, 100, 50), mon("HDMI-1", 0, 0, 100, 50)];
        assert_eq!(match_monitors(&shots, &mons), vec![Some(1), Some(0)]);
    }

    #[test]
    fn falls_back_to_size_and_scaled_position() {
        // HiDPI: xcap reports origin 960 (logical), winit 1920 (physical).
        let shots = [
            shot("", 960, 0, 200, 100, 2.0),
            shot("", 0, 0, 200, 100, 2.0),
        ];
        let mons = [
            mon("XWAYLAND0", 0, 0, 200, 100),
            mon("XWAYLAND1", 1920, 0, 200, 100),
        ];
        assert_eq!(match_monitors(&shots, &mons), vec![Some(1), Some(0)]);
    }

    #[test]
    fn falls_back_to_order() {
        let shots = [shot("a", 0, 0, 10, 10, 1.0), shot("b", 10, 0, 10, 10, 1.0)];
        let mons = [mon("x", 0, 0, 30, 30), mon("y", 30, 0, 30, 30)];
        assert_eq!(match_monitors(&shots, &mons), vec![Some(0), Some(1)]);
    }

    #[test]
    fn more_shots_than_monitors() {
        let shots = [shot("a", 0, 0, 10, 10, 1.0), shot("b", 10, 0, 10, 10, 1.0)];
        let mons = [mon("b", 10, 0, 10, 10)];
        assert_eq!(match_monitors(&shots, &mons), vec![None, Some(0)]);
    }
}
