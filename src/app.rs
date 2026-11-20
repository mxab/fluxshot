//! winit overlay: one borderless, always-on-top window per monitor showing the
//! frozen capture. Rendering is CPU (tiny-skia) → softbuffer.

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use anyhow::Result;
use tiny_skia::{Pixmap, Transform};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{CursorIcon, Window, WindowId, WindowLevel};

use crate::annotation::Tool;
use crate::capture::MonitorShot;
use crate::geom::{Pt, Rect};
use crate::output;
use crate::render::{self, blit_scaled, render_export};
use crate::state::{Handle, Hover, PALETTE, State};
use crate::toolbar::{self, Button};

const HANDLE_RADIUS: f32 = 4.0; // logical
const HANDLE_GRAB: f32 = 10.0; // logical
const ACCENT: [u8; 4] = [0, 122, 255, 255];

pub struct Options {
    pub save_dir: PathBuf,
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

struct App {
    shots: Vec<MonitorShot>,
    views: Vec<View>,
    state: State,
    mods: ModifiersState,
    opts: Options,
    /// Last view the pointer was over (target for "no selection" export).
    last_view: usize,
    outcome: Option<Result<String>>,
}

/// Show the overlay and block until the user copies, saves or cancels.
/// Returns a human-readable message describing what happened, or `None` on cancel.
pub fn run(shots: Vec<MonitorShot>, opts: Options) -> Result<Option<String>> {
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
        for shot in &self.shots {
            let attrs = Window::default_attributes()
                .with_title("FluxShot")
                .with_decorations(false)
                .with_resizable(false)
                .with_window_level(WindowLevel::AlwaysOnTop)
                .with_visible(false);
            // xcap reports logical points on macOS and physical pixels elsewhere.
            let attrs = if cfg!(target_os = "macos") {
                attrs
                    .with_position(LogicalPosition::new(shot.x, shot.y))
                    .with_inner_size(LogicalSize::new(shot.width, shot.height))
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
            let (w, h) = (size.width.max(1), size.height.max(1));
            let scale = w as f32 / shot.image.width() as f32;
            let dimmed_img = render::dimmed(&shot.image, 0.45);
            let dimmed = if (scale - 1.0).abs() < f32::EPSILON && h == shot.image.height() {
                dimmed_img
            } else {
                let mut d = Pixmap::new(w, h).expect("window size");
                blit_scaled(&mut d, &dimmed_img, 0.0, 0.0, scale);
                d
            };
            let context =
                softbuffer::Context::new(window.clone()).map_err(|e| anyhow::anyhow!("{e}"))?;
            let surface = softbuffer::Surface::new(&context, window.clone())
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            self.views.push(View {
                ui: window.scale_factor() as f32,
                window,
                surface,
                frame: Pixmap::new(w, h).expect("window size"),
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
        // Get out of the way immediately; encoding happens after.
        for v in &self.views {
            v.window.set_visible(false);
        }
        let (w, h) = (img.width(), img.height());
        self.outcome = Some(match how {
            Finish::Copy => {
                output::to_clipboard(&img).map(|_| format!("Copied {w}×{h} to clipboard"))
            }
            Finish::Save => {
                let path = output::timestamped_path(&self.opts.save_dir);
                output::save_png(&img, &path)
                    .map(|_| format!("Saved {w}×{h} to {}", path.display()))
            }
        });
        el.exit();
    }
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
