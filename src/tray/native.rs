//! macOS / Windows tray: tray-icon + muda on a winit event loop.

use std::sync::Mutex;
use std::time::Instant;

use anyhow::{Context, Result};
use global_hotkey::{GlobalHotKeyEvent, HotKeyState};
use tray_icon::menu::{
    CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu,
};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

use super::{Action, Controller, MenuModel, TICK, icon};

enum UserEvent {
    Menu(MenuId),
    Hotkey,
}

struct App {
    ctl: Controller,
    tray: Option<TrayIcon>,
    /// Menu item id → action, rebuilt with the menu.
    actions: Vec<(MenuId, Action)>,
    error: Option<anyhow::Error>,
}

pub fn run(ctl: Controller) -> Result<()> {
    #[allow(unused_mut)]
    let mut builder = EventLoop::<UserEvent>::with_user_event();
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        // Menu bar extra only: no Dock icon, no app menu.
        builder.with_activation_policy(ActivationPolicy::Accessory);
    }
    let event_loop = builder.build()?;

    let proxy = Mutex::new(event_loop.create_proxy());
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        let _ = proxy.lock().unwrap().send_event(UserEvent::Menu(e.id));
    }));
    let proxy = Mutex::new(event_loop.create_proxy());
    GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
        if e.state() == HotKeyState::Pressed {
            let _ = proxy.lock().unwrap().send_event(UserEvent::Hotkey);
        }
    }));

    let mut app = App {
        ctl,
        tray: None,
        actions: Vec::new(),
        error: None,
    };
    event_loop.run_app(&mut app)?;
    app.error.map_or(Ok(()), Err)
}

impl App {
    fn rebuild_menu(&mut self) {
        let Some(tray) = &self.tray else { return };
        match build_menu(&self.ctl.model()) {
            Ok((menu, actions)) => {
                tray.set_menu(Some(Box::new(menu)));
                self.actions = actions;
            }
            Err(e) => eprintln!("fluxshot: building tray menu: {e:#}"),
        }
    }

    fn init(&mut self) -> Result<()> {
        let template = cfg!(target_os = "macos");
        let icon =
            Icon::from_rgba(icon::rgba(template), icon::SIZE, icon::SIZE).context("tray icon")?;
        let builder = TrayIconBuilder::new().with_tooltip("FluxShot");
        // Template icons let the macOS menu bar tint the glyph for light/dark.
        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_templated(icon);
        #[cfg(not(target_os = "macos"))]
        let builder = builder.with_icon(icon);
        self.tray = Some(builder.build().context("creating tray icon")?);
        self.ctl.start();
        self.rebuild_menu();
        Ok(())
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn new_events(&mut self, el: &ActiveEventLoop, cause: StartCause) {
        match cause {
            // macOS requires the status item to be created once the app runs.
            StartCause::Init => {
                if let Err(e) = self.init() {
                    self.error = Some(e);
                    el.exit();
                    return;
                }
            }
            StartCause::ResumeTimeReached { .. } if self.ctl.tick() => {
                self.rebuild_menu();
            }
            _ => {}
        }
        if matches!(
            cause,
            StartCause::Init | StartCause::ResumeTimeReached { .. }
        ) {
            el.set_control_flow(ControlFlow::WaitUntil(Instant::now() + TICK));
        }
    }

    fn resumed(&mut self, _el: &ActiveEventLoop) {}

    fn window_event(&mut self, _el: &ActiveEventLoop, _id: WindowId, _event: WindowEvent) {}

    fn user_event(&mut self, el: &ActiveEventLoop, event: UserEvent) {
        let action = match event {
            UserEvent::Hotkey => Some(Action::Capture),
            UserEvent::Menu(id) => self
                .actions
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(_, a)| a.clone()),
        };
        let Some(action) = action else { return };
        let refresh = !matches!(
            action,
            Action::Capture | Action::Open(_) | Action::Reveal(_)
        );
        if self.ctl.handle(action) {
            el.exit();
            return;
        }
        if refresh {
            self.rebuild_menu();
        }
    }
}

type Actions = Vec<(MenuId, Action)>;

fn build_menu(m: &MenuModel) -> Result<(Menu, Actions)> {
    let mut actions: Actions = Vec::new();
    let mut item = |label: &str, enabled: bool, action: Action| {
        let id = MenuId::new(format!("a{}", actions.len()));
        actions.push((id.clone(), action));
        MenuItem::with_id(id, escape(label), enabled, None)
    };

    let menu = Menu::new();
    menu.append(&item(&m.capture_label, true, Action::Capture))?;

    let recent = Submenu::new("Recent", !m.recent.is_empty());
    for r in &m.recent {
        let sub = Submenu::new(escape(&r.label), true);
        sub.append_items(&[
            &item("Copy", true, Action::Copy(r.path.clone())),
            &item("Open", true, Action::Open(r.path.clone())),
            &item(REVEAL, true, Action::Reveal(r.path.clone())),
            &PredefinedMenuItem::separator(),
            &item(TRASH, true, Action::Trash(r.path.clone())),
        ])?;
        recent.append(&sub)?;
    }
    menu.append(&recent)?;
    menu.append(&item("Open screenshots folder", true, Action::OpenFolder))?;
    menu.append(&item(&m.prune_label, m.prune_enabled, Action::PruneNow))?;
    menu.append(&PredefinedMenuItem::separator())?;

    let id = MenuId::new("autostart");
    actions.push((id.clone(), Action::ToggleAutostart));
    menu.append(&CheckMenuItem::with_id(
        id,
        "Launch at login",
        true,
        m.autostart,
        None,
    ))?;
    let mut item = |label: &str, action: Action| {
        let id = MenuId::new(format!("a{}", actions.len()));
        actions.push((id.clone(), action));
        MenuItem::with_id(id, label, true, None)
    };
    menu.append(&item("Edit settings…", Action::EditConfig))?;
    menu.append(&item("Reload settings", Action::ReloadConfig))?;
    if let Some(n) = &m.notice {
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&MenuItem::new(escape(n), false, None))?;
    }
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&item("Quit FluxShot", Action::Quit))?;
    Ok((menu, actions))
}

#[cfg(target_os = "macos")]
const REVEAL: &str = "Show in Finder";
#[cfg(windows)]
const REVEAL: &str = "Show in Explorer";
#[cfg(target_os = "macos")]
const TRASH: &str = "Move to Trash";
#[cfg(windows)]
const TRASH: &str = "Move to Recycle Bin";

/// `&` marks a mnemonic in muda labels.
fn escape(s: &str) -> String {
    s.replace('&', "&&")
}
