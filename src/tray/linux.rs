//! Linux tray: a StatusNotifierItem over D-Bus (ksni), no GTK.
//! ksni serves the menu from its own thread; menu clicks and hotkey presses
//! are sent here over a channel and handled on the main thread.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};

use anyhow::{Context, Result};
use global_hotkey::{GlobalHotKeyEvent, HotKeyState};
use ksni::blocking::TrayMethods;
use ksni::menu::{CheckmarkItem, StandardItem, SubMenu};
use ksni::{MenuItem, ToolTip};

use super::{Action, Controller, MenuModel, TICK, icon};

struct KsTray {
    model: MenuModel,
    tx: Sender<Action>,
    icon: ksni::Icon,
}

pub fn run(mut ctl: Controller) -> Result<()> {
    let (tx, rx) = mpsc::channel::<Action>();
    let hotkey_tx = tx.clone();
    GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
        if e.state() == HotKeyState::Pressed {
            let _ = hotkey_tx.send(Action::Capture);
        }
    }));

    ctl.start();
    let tray = KsTray {
        model: ctl.model(),
        tx,
        icon: argb_icon(),
    };
    let handle = tray
        .spawn()
        .context("creating tray icon (is a StatusNotifierItem host running? On GNOME, install the AppIndicator extension)")?;

    loop {
        let refresh = match rx.recv_timeout(TICK) {
            Ok(action) => {
                let refresh = !matches!(
                    action,
                    Action::Capture | Action::Open(_) | Action::Reveal(_)
                );
                if ctl.handle(action) {
                    break;
                }
                refresh
            }
            Err(RecvTimeoutError::Timeout) => ctl.tick(),
            Err(RecvTimeoutError::Disconnected) => break,
        };
        if refresh {
            let model = ctl.model();
            if handle.update(move |t| t.model = model).is_none() {
                break; // tray service went away
            }
        }
    }
    handle.shutdown().wait();
    Ok(())
}

fn argb_icon() -> ksni::Icon {
    // ksni wants ARGB32 in network byte order.
    let data = icon::rgba(false)
        .chunks_exact(4)
        .flat_map(|p| [p[3], p[0], p[1], p[2]])
        .collect();
    ksni::Icon {
        width: icon::SIZE as i32,
        height: icon::SIZE as i32,
        data,
    }
}

fn send(action: Action) -> Box<dyn Fn(&mut KsTray) + Send> {
    Box::new(move |t: &mut KsTray| {
        let _ = t.tx.send(action.clone());
    })
}

fn item(label: &str, enabled: bool, action: Action) -> MenuItem<KsTray> {
    StandardItem {
        label: escape(label),
        enabled,
        activate: send(action),
        ..Default::default()
    }
    .into()
}

/// `_` marks an access key in D-Bus menu labels.
fn escape(s: &str) -> String {
    s.replace('_', "__")
}

impl ksni::Tray for KsTray {
    fn id(&self) -> String {
        "fluxshot".into()
    }

    fn title(&self) -> String {
        "FluxShot".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![self.icon.clone()]
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "FluxShot".into(),
            description: "Left-click to capture".into(),
            ..Default::default()
        }
    }

    /// Left click captures right away; the menu is on right click.
    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.send(Action::Capture);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let m = &self.model;
        let recent = m
            .recent
            .iter()
            .map(|r| {
                SubMenu {
                    label: escape(&r.label),
                    submenu: vec![
                        item("Copy", true, Action::Copy(r.path.clone())),
                        item("Open", true, Action::Open(r.path.clone())),
                        item("Open folder", true, Action::Reveal(r.path.clone())),
                        MenuItem::Separator,
                        item("Move to Trash", true, Action::Trash(r.path.clone())),
                    ],
                    ..Default::default()
                }
                .into()
            })
            .collect();
        let mut menu = vec![
            item(&m.capture_label, true, Action::Capture),
            SubMenu {
                label: "Recent".into(),
                enabled: !m.recent.is_empty(),
                submenu: recent,
                ..Default::default()
            }
            .into(),
            item("Open screenshots folder", true, Action::OpenFolder),
            item(&m.prune_label, m.prune_enabled, Action::PruneNow),
            MenuItem::Separator,
            CheckmarkItem {
                label: "Launch at login".into(),
                checked: m.autostart,
                activate: send(Action::ToggleAutostart),
                ..Default::default()
            }
            .into(),
            item("Edit settings…", true, Action::EditConfig),
            item("Reload settings", true, Action::ReloadConfig),
        ];
        if let Some(n) = &m.notice {
            menu.push(MenuItem::Separator);
            menu.push(item(n, false, Action::Capture));
        }
        menu.push(MenuItem::Separator);
        menu.push(item("Quit FluxShot", true, Action::Quit));
        menu
    }
}
