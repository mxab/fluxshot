//! `fluxshot tray`: a long-running tray icon to capture, browse recent
//! screenshots and prune old ones.
//!
//! Captures run as a child `fluxshot` process, so the overlay keeps its own
//! event loop and the tray never holds frozen screen contents in memory.
//! [`Controller`] holds all state and logic; the platform backends only
//! render its [`MenuModel`] and feed [`Action`]s back.

mod icon;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "macos", windows))]
mod native;

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result};
use global_hotkey::GlobalHotKeyManager;
use global_hotkey::hotkey::HotKey;

use crate::config::Config;
use crate::{library, notify, output};

/// How often the backends call [`Controller::tick`].
pub const TICK: Duration = Duration::from_secs(2);
const PRUNE_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone, Debug)]
pub enum Action {
    Capture,
    Copy(PathBuf),
    Open(PathBuf),
    Reveal(PathBuf),
    Trash(PathBuf),
    OpenFolder,
    PruneNow,
    ToggleAutostart,
    EditConfig,
    ReloadConfig,
    Quit,
}

pub struct RecentItem {
    pub label: String,
    pub path: PathBuf,
}

/// Everything a backend needs to draw the menu.
pub struct MenuModel {
    pub capture_label: String,
    pub recent: Vec<RecentItem>,
    pub prune_label: String,
    pub prune_enabled: bool,
    pub autostart: bool,
    /// One-line notice shown as a disabled item (e.g. hotkey problems).
    pub notice: Option<String>,
}

pub fn run(path_override: Option<PathBuf>) -> Result<()> {
    let _lock = single_instance()?;
    let ctl = Controller::new(path_override);
    #[cfg(any(target_os = "macos", windows))]
    return native::run(ctl);
    #[cfg(target_os = "linux")]
    return linux::run(ctl);
}

/// Held for the life of the process; a second tray exits with an error.
fn single_instance() -> Result<File> {
    let dir = directories::ProjectDirs::from("", "", "fluxshot")
        .map(|d| d.cache_dir().to_path_buf())
        .unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join("tray.lock");
    let file = File::create(&path).with_context(|| format!("opening {}", path.display()))?;
    file.try_lock()
        .map_err(|_| anyhow::anyhow!("the FluxShot tray is already running"))?;
    Ok(file)
}

pub struct Controller {
    cfg: Config,
    path_override: Option<PathBuf>,
    capture: Option<Child>,
    last_prune: Option<Instant>,
    dir_stamp: Option<SystemTime>,
    hotkeys: Option<GlobalHotKeyManager>,
    hotkey: Option<HotKey>,
    notice: Option<String>,
}

impl Controller {
    fn new(path_override: Option<PathBuf>) -> Self {
        let mut ctl = Self {
            cfg: Config::default(),
            path_override,
            capture: None,
            last_prune: None,
            dir_stamp: None,
            hotkeys: None,
            hotkey: None,
            notice: None,
        };
        ctl.cfg = ctl.load_config().unwrap_or_default();
        ctl
    }

    /// Backend calls this once its event loop is running (hotkey
    /// registration needs that on macOS and Windows).
    pub fn start(&mut self) {
        self.apply_config();
        self.dir_stamp = self.read_dir_stamp();
    }

    fn load_config(&self) -> Option<Config> {
        match Config::load() {
            Ok(cfg) => Some(cfg),
            Err(e) => {
                notify::error("Settings not loaded", &format!("{e:#}"));
                None
            }
        }
    }

    /// Hotkey, autostart override, and a prune pass (also on reload).
    fn apply_config(&mut self) {
        self.notice = None;
        if let Err(e) = self.register_hotkey() {
            eprintln!("fluxshot: hotkey: {e:#}");
            self.notice = Some(format!("Hotkey unavailable: {e}"));
        }
        if let Some(want) = self.cfg.autostart
            && let Err(e) = set_autostart(want)
        {
            eprintln!("fluxshot: autostart: {e:#}");
        }
        self.prune(false);
    }

    fn register_hotkey(&mut self) -> Result<()> {
        if let (Some(m), Some(old)) = (&self.hotkeys, self.hotkey.take()) {
            let _ = m.unregister(old);
        }
        let spec = self.cfg.hotkey.capture.trim();
        if spec.is_empty() {
            return Ok(());
        }
        if cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some() {
            anyhow::bail!("not supported on Wayland, bind `fluxshot` in your desktop settings");
        }
        let hotkey: HotKey = spec
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid hotkey {spec:?}: {e}"))?;
        if self.hotkeys.is_none() {
            self.hotkeys = Some(GlobalHotKeyManager::new()?);
        }
        let m = self.hotkeys.as_ref().expect("just created");
        m.register(hotkey)
            .map_err(|e| anyhow::anyhow!("{spec} is taken ({e})"))?;
        self.hotkey = Some(hotkey);
        Ok(())
    }

    fn save_dir(&self) -> PathBuf {
        self.path_override
            .clone()
            .unwrap_or_else(|| self.cfg.save_dir())
    }

    fn shots(&self) -> Vec<library::Shot> {
        library::list(&self.save_dir(), self.cfg.filename_prefix())
    }

    /// Returns `true` when the app should quit.
    pub fn handle(&mut self, action: Action) -> bool {
        let result = match action {
            Action::Capture => self.start_capture(),
            Action::Copy(p) => output::load_png(&p).and_then(|pm| output::to_clipboard(&pm)),
            Action::Open(p) => open(&p),
            Action::Reveal(p) => reveal(&p),
            Action::Trash(p) => library::trash(&p),
            Action::OpenFolder => {
                let dir = self.save_dir();
                std::fs::create_dir_all(&dir)
                    .with_context(|| format!("creating {}", dir.display()))
                    .and_then(|()| open(&dir))
            }
            Action::PruneNow => {
                self.prune(true);
                Ok(())
            }
            Action::ToggleAutostart => autostart_enabled().and_then(|on| set_autostart(!on)),
            Action::EditConfig => Config::ensure_file().and_then(|p| edit(&p)),
            Action::ReloadConfig => {
                if let Some(cfg) = self.load_config() {
                    self.cfg = cfg;
                    self.apply_config();
                }
                Ok(())
            }
            Action::Quit => return true,
        };
        if let Err(e) = result {
            eprintln!("fluxshot: {e:#}");
            notify::error("FluxShot", &format!("{e:#}"));
        }
        false
    }

    fn start_capture(&mut self) -> Result<()> {
        if self.capture_running() {
            return Ok(()); // overlay already up; ignore repeated hotkey presses
        }
        let exe = std::env::current_exe().context("locating fluxshot executable")?;
        let mut cmd = Command::new(exe);
        if let Some(p) = &self.path_override {
            cmd.arg("--path").arg(p);
        }
        let child = cmd
            .stdin(Stdio::null())
            .spawn()
            .context("starting capture")?;
        self.capture = Some(child);
        Ok(())
    }

    fn capture_running(&mut self) -> bool {
        match &mut self.capture {
            Some(c) => match c.try_wait() {
                Ok(None) => true,
                _ => {
                    self.capture = None;
                    false
                }
            },
            None => false,
        }
    }

    /// Periodic housekeeping. Returns `true` if the menu should be rebuilt.
    pub fn tick(&mut self) -> bool {
        self.capture_running(); // reaps a finished capture
        if self.last_prune.is_none_or(|t| t.elapsed() >= PRUNE_EVERY) {
            self.prune(false);
        }
        let stamp = self.read_dir_stamp();
        if stamp != self.dir_stamp {
            self.dir_stamp = stamp;
            return true;
        }
        false
    }

    /// Directory mtime changes whenever a file is added or removed.
    fn read_dir_stamp(&self) -> Option<SystemTime> {
        std::fs::metadata(self.save_dir())
            .and_then(|m| m.modified())
            .ok()
    }

    fn prune(&mut self, manual: bool) {
        self.last_prune = Some(Instant::now());
        let policy = self.cfg.retention;
        if policy.is_unlimited() {
            if manual {
                notify::info(
                    "Nothing to prune",
                    "No retention limits are set. Use \"Edit settings…\" to add some.",
                );
            }
            return;
        }
        match library::prune(&self.save_dir(), self.cfg.filename_prefix(), &policy) {
            Ok(r) if r.files > 0 => notify::info(
                "Old screenshots moved to trash",
                &format!("{} file(s), {}", r.files, library::human_size(r.bytes)),
            ),
            Ok(_) if manual => {
                notify::info("Nothing to prune", "All screenshots are within the limits.")
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("fluxshot: prune: {e:#}");
                notify::error("Prune failed", &format!("{e:#}"));
            }
        }
    }

    pub fn model(&self) -> MenuModel {
        let recent = self
            .shots()
            .into_iter()
            .take(self.cfg.tray.recent_count)
            .map(|s| RecentItem {
                label: recent_label(&s),
                path: s.path,
            })
            .collect();
        let capture_label = match &self.hotkey {
            Some(_) => format!("Capture  ({})", self.cfg.hotkey.capture.trim()),
            None => "Capture".into(),
        };
        let policy = self.cfg.retention;
        let (prune_label, prune_enabled) = if policy.is_unlimited() {
            ("Prune now (no limits set)".into(), false)
        } else if self.cfg.filename_prefix().is_empty() {
            ("Prune now (filename has no fixed prefix)".into(), false)
        } else {
            let r = library::prune_preview(&self.save_dir(), self.cfg.filename_prefix(), &policy);
            if r.files == 0 {
                ("Prune now (nothing to prune)".into(), false)
            } else {
                (
                    format!(
                        "Prune now ({} file(s), {})",
                        r.files,
                        library::human_size(r.bytes)
                    ),
                    true,
                )
            }
        };
        MenuModel {
            capture_label,
            recent,
            prune_label,
            prune_enabled,
            autostart: autostart_enabled().unwrap_or(false),
            notice: self.notice.clone(),
        }
    }
}

fn recent_label(s: &library::Shot) -> String {
    let when: chrono::DateTime<chrono::Local> = s.modified.into();
    let today = chrono::Local::now().date_naive();
    let time = if when.date_naive() == today {
        when.format("%H:%M:%S").to_string()
    } else {
        when.format("%b %d, %H:%M").to_string()
    };
    format!("{time}  ·  {}", library::human_size(s.size))
}

// ---- autostart ----

fn auto_launch() -> Result<auto_launch::AutoLaunch> {
    let exe = std::env::current_exe().context("locating fluxshot executable")?;
    let mut b = auto_launch::AutoLaunchBuilder::new();
    b.set_app_name("FluxShot")
        .set_app_path(&exe.to_string_lossy())
        .set_args(&["tray"])
        .set_macos_launch_mode(auto_launch::MacOSLaunchMode::LaunchAgent)
        .set_windows_enable_mode(auto_launch::WindowsEnableMode::CurrentUser);
    b.build().context("configuring launch at login")
}

fn autostart_enabled() -> Result<bool> {
    Ok(auto_launch()?.is_enabled()?)
}

fn set_autostart(on: bool) -> Result<()> {
    let al = auto_launch()?;
    if al.is_enabled()? == on {
        return Ok(());
    }
    if on { al.enable() } else { al.disable() }.context("updating launch at login")
}

// ---- opening files ----

fn spawn_detached(cmd: &mut Command) -> Result<()> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .with_context(|| format!("running {:?}", cmd.get_program()))?;
    // Reap it so it doesn't linger as a zombie.
    std::thread::spawn(move || child.wait());
    Ok(())
}

fn open(path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    return spawn_detached(Command::new("open").arg(path));
    #[cfg(target_os = "linux")]
    return spawn_detached(Command::new("xdg-open").arg(path));
    #[cfg(windows)]
    return spawn_detached(Command::new("explorer").arg(path));
}

fn reveal(path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    return spawn_detached(Command::new("open").arg("-R").arg(path));
    #[cfg(target_os = "linux")]
    return open(path.parent().unwrap_or(path));
    #[cfg(windows)]
    {
        let mut arg = std::ffi::OsString::from("/select,");
        arg.push(path);
        return spawn_detached(Command::new("explorer").arg(arg));
    }
}

/// Open a text file in an editor. `.toml` often has no default app.
fn edit(path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    return spawn_detached(Command::new("open").arg("-t").arg(path));
    #[cfg(target_os = "linux")]
    return spawn_detached(Command::new("xdg-open").arg(path));
    #[cfg(windows)]
    return spawn_detached(Command::new("notepad").arg(path));
}
