//! User settings, read from `config.toml` in the platform config directory.
//! Every field is optional; a missing file means all defaults.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

pub const DEFAULT_FILENAME: &str = "fluxshot_%Y-%m-%d_%H-%M-%S";
pub const DEFAULT_HOTKEY: &str = "CmdOrCtrl+Shift+2";

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Where PNGs go. `~` is expanded.
    pub save_dir: Option<PathBuf>,
    /// chrono strftime template, `.png` is appended.
    pub filename: String,
    /// Copying (Enter) also saves a PNG, so the history has every shot.
    pub save_on_copy: bool,
    pub png_compression: Compression,
    /// Desktop notification after copy/save.
    pub notify: bool,
    /// Start `fluxshot tray` at login. When set, it is enforced every time
    /// the tray starts or reloads; when unset, the tray menu toggle decides.
    pub autostart: Option<bool>,
    pub hotkey: Hotkey,
    pub retention: Retention,
    pub tray: Tray,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Compression {
    #[default]
    Fast,
    Default,
    Best,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Hotkey {
    /// Global capture hotkey in tray mode; empty disables it.
    pub capture: String,
}

/// Auto-prune policy; 0 means unlimited.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Retention {
    pub max_age_days: u64,
    pub max_files: usize,
    pub max_size_mb: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct Tray {
    pub recent_count: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            save_dir: None,
            filename: DEFAULT_FILENAME.into(),
            save_on_copy: false,
            png_compression: Compression::Fast,
            notify: true,
            autostart: None,
            hotkey: Hotkey::default(),
            retention: Retention::default(),
            tray: Tray::default(),
        }
    }
}

impl Default for Hotkey {
    fn default() -> Self {
        Self {
            capture: DEFAULT_HOTKEY.into(),
        }
    }
}

impl Default for Tray {
    fn default() -> Self {
        Self { recent_count: 10 }
    }
}

impl Retention {
    pub fn is_unlimited(&self) -> bool {
        self.max_age_days == 0 && self.max_files == 0 && self.max_size_mb == 0
    }
}

impl Config {
    pub fn path() -> Option<PathBuf> {
        directories::ProjectDirs::from("", "", "fluxshot")
            .map(|d| d.config_dir().join("config.toml"))
    }

    /// Defaults if the file doesn't exist; an error if it exists but is invalid.
    pub fn load() -> Result<Self> {
        let Some(path) = Self::path() else {
            return Ok(Self::default());
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text).with_context(|| format!("in {}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }

    /// Like [`Config::load`], but falls back to defaults with a warning, so
    /// a typo in the config never stops a capture.
    pub fn load_or_default() -> Self {
        Self::load().unwrap_or_else(|e| {
            eprintln!("fluxshot: ignoring config: {e:#}");
            Self::default()
        })
    }

    pub fn parse(text: &str) -> Result<Self> {
        let cfg: Self = toml::from_str(text)?;
        validate_template(&cfg.filename)?;
        Ok(cfg)
    }

    pub fn save_dir(&self) -> PathBuf {
        match &self.save_dir {
            Some(p) => expand_tilde(p),
            None => crate::output::default_dir(),
        }
    }

    /// Literal text before the first strftime specifier. Only files starting
    /// with it count as ours when listing and pruning.
    pub fn filename_prefix(&self) -> &str {
        template_prefix(&self.filename)
    }

    /// Write a commented default config if none exists yet; returns its path.
    pub fn ensure_file() -> Result<PathBuf> {
        let path = Self::path().context("no config directory on this system")?;
        if !path.exists() {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)
                    .with_context(|| format!("creating {}", dir.display()))?;
            }
            std::fs::write(&path, TEMPLATE)
                .with_context(|| format!("writing {}", path.display()))?;
        }
        Ok(path)
    }
}

pub fn template_prefix(template: &str) -> &str {
    template.split('%').next().unwrap_or("")
}

fn validate_template(t: &str) -> Result<()> {
    use chrono::format::{Item, StrftimeItems};
    if t.is_empty() {
        bail!("`filename` must not be empty");
    }
    if t.contains(['/', '\\']) {
        bail!("`filename` must not contain path separators");
    }
    if StrftimeItems::new(t).any(|i| matches!(i, Item::Error)) {
        bail!("`filename` has an invalid strftime specifier: {t:?}");
    }
    Ok(())
}

fn expand_tilde(p: &Path) -> PathBuf {
    if let Ok(rest) = p.strip_prefix("~")
        && let Some(home) = directories::BaseDirs::new().map(|b| b.home_dir().to_path_buf())
    {
        return home.join(rest);
    }
    p.to_path_buf()
}

/// Written by "Edit settings…" when no config exists yet.
const TEMPLATE: &str = r#"# FluxShot settings. Every key is optional; the values below are the defaults.
# After editing, choose "Reload settings" in the tray menu.

# Where screenshots are saved (default: <Pictures>/FluxShot).
# save_dir = "~/Pictures/FluxShot"

# File name, chrono strftime syntax. ".png" is appended.
# Only files starting with the text before the first "%" are listed and pruned.
# filename = "fluxshot_%Y-%m-%d_%H-%M-%S"

# Also save a PNG when copying with Enter, so the tray history has every shot.
# save_on_copy = false

# PNG compression: "fast", "default" or "best".
# png_compression = "fast"

# Show a desktop notification after copying or saving.
# notify = true

# Start the tray at login. If set here, it is applied every time the tray
# starts or reloads settings; leave it unset to use the tray menu toggle.
# autostart = true

[hotkey]
# Global capture hotkey while the tray runs. "" disables it.
# Not available on Wayland; bind `fluxshot` in your desktop settings instead.
# capture = "CmdOrCtrl+Shift+2"

[retention]
# Old screenshots are moved to the trash. 0 means unlimited.
# max_age_days = 0
# max_files = 0
# max_size_mb = 0

[tray]
# How many screenshots the "Recent" menu shows.
# recent_count = 10
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_is_default() {
        assert_eq!(Config::parse("").unwrap(), Config::default());
    }

    #[test]
    fn template_file_is_default() {
        assert_eq!(Config::parse(TEMPLATE).unwrap(), Config::default());
    }

    #[test]
    fn partial_tables_keep_other_defaults() {
        let cfg = Config::parse(
            "save_on_copy = true\npng_compression = \"best\"\n[retention]\nmax_files = 50\n",
        )
        .unwrap();
        assert!(cfg.save_on_copy);
        assert_eq!(cfg.png_compression, Compression::Best);
        assert_eq!(cfg.retention.max_files, 50);
        assert_eq!(cfg.retention.max_age_days, 0);
        assert_eq!(cfg.hotkey.capture, DEFAULT_HOTKEY);
        assert_eq!(cfg.tray.recent_count, 10);
    }

    #[test]
    fn rejects_bad_values() {
        assert!(Config::parse("filename = \"a/%Y\"").is_err());
        assert!(Config::parse("filename = \"shot_%Q\"").is_err());
        assert!(Config::parse("filename = \"\"").is_err());
        assert!(Config::parse("bogus = 1").is_err());
        assert!(Config::parse("png_compression = \"ultra\"").is_err());
    }

    #[test]
    fn prefix_is_text_before_first_specifier() {
        assert_eq!(template_prefix(DEFAULT_FILENAME), "fluxshot_");
        assert_eq!(template_prefix("%Y-%m-%d"), "");
        assert_eq!(template_prefix("shot"), "shot");
    }

    #[test]
    fn tilde_expands_to_home() {
        let p = expand_tilde(Path::new("~/x"));
        assert!(p.is_absolute() && p.ends_with("x"));
        assert_eq!(expand_tilde(Path::new("/abs")), Path::new("/abs"));
    }
}
