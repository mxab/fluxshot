//! The folder of saved screenshots: listing, pruning, trashing.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result, bail};

use crate::config::Retention;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shot {
    pub path: PathBuf,
    pub modified: SystemTime,
    pub size: u64,
}

/// PNGs directly in `dir` whose names start with `prefix`, newest first.
/// A missing directory is just an empty library.
pub fn list(dir: &Path, prefix: &str) -> Vec<Shot> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut shots: Vec<Shot> = entries
        .filter_map(|e| {
            let e = e.ok()?;
            let name = e.file_name();
            let name = name.to_str()?;
            if !name.starts_with(prefix) || !name.to_ascii_lowercase().ends_with(".png") {
                return None;
            }
            let meta = e.metadata().ok()?;
            meta.is_file().then(|| Shot {
                path: e.path(),
                modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                size: meta.len(),
            })
        })
        .collect();
    sort_newest_first(&mut shots);
    shots
}

fn sort_newest_first(shots: &mut [Shot]) {
    shots.sort_by(|a, b| b.modified.cmp(&a.modified).then(b.path.cmp(&a.path)));
}

/// Which shots the policy removes. `shots` must be newest first (as from
/// [`list`]). Rules: too old, then beyond `max_files`, then oldest until
/// the total fits `max_size_mb`.
pub fn select_for_prune<'a>(
    shots: &'a [Shot],
    policy: &Retention,
    now: SystemTime,
) -> Vec<&'a Shot> {
    let max_age =
        (policy.max_age_days > 0).then(|| Duration::from_secs(policy.max_age_days * 24 * 60 * 60));
    let max_bytes = policy.max_size_mb * 1024 * 1024;
    let mut kept = 0usize;
    let mut kept_bytes = 0u64;
    let mut out = Vec::new();
    for shot in shots {
        let too_old =
            max_age.is_some_and(|max| now.duration_since(shot.modified).is_ok_and(|age| age > max));
        let too_many = policy.max_files > 0 && kept >= policy.max_files;
        let too_big = policy.max_size_mb > 0 && kept_bytes + shot.size > max_bytes;
        if too_old || too_many || too_big {
            out.push(shot);
        } else {
            kept += 1;
            kept_bytes += shot.size;
        }
    }
    out
}

pub struct PruneReport {
    pub files: usize,
    pub bytes: u64,
}

/// Move everything the policy selects to the trash.
pub fn prune(dir: &Path, prefix: &str, policy: &Retention) -> Result<PruneReport> {
    if prefix.is_empty() {
        bail!(
            "refusing to prune: the `filename` template has no fixed prefix, \
             so FluxShot can't tell its files apart from others in {}",
            dir.display()
        );
    }
    let shots = list(dir, prefix);
    let doomed = select_for_prune(&shots, policy, SystemTime::now());
    let report = PruneReport {
        files: doomed.len(),
        bytes: doomed.iter().map(|s| s.size).sum(),
    };
    if !doomed.is_empty() {
        trash_ctx()
            .delete_all(doomed.iter().map(|s| &s.path))
            .context("moving files to trash")?;
    }
    Ok(report)
}

/// Like [`prune`] but only reports what would go.
pub fn prune_preview(dir: &Path, prefix: &str, policy: &Retention) -> PruneReport {
    if prefix.is_empty() {
        return PruneReport { files: 0, bytes: 0 };
    }
    let shots = list(dir, prefix);
    let doomed = select_for_prune(&shots, policy, SystemTime::now());
    PruneReport {
        files: doomed.len(),
        bytes: doomed.iter().map(|s| s.size).sum(),
    }
}

pub fn trash(path: &Path) -> Result<()> {
    trash_ctx()
        .delete(path)
        .with_context(|| format!("moving {} to trash", path.display()))
}

fn trash_ctx() -> trash::TrashContext {
    #[allow(unused_mut)]
    let mut ctx = trash::TrashContext::default();
    // The default asks Finder via AppleScript, which blocks on an Automation
    // permission prompt. NSFileManager needs no permission (but has no
    // "Put Back").
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        ctx.set_delete_method(DeleteMethod::NsFileManager);
    }
    ctx
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: u64 = 24 * 60 * 60;
    const MB: u64 = 1024 * 1024;

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1000 * DAY)
    }

    /// Newest first: shot i is i days old.
    fn shots(sizes_mb: &[u64]) -> Vec<Shot> {
        sizes_mb
            .iter()
            .enumerate()
            .map(|(i, &mb)| Shot {
                path: PathBuf::from(format!("fluxshot_{i}.png")),
                modified: now() - Duration::from_secs(i as u64 * DAY),
                size: mb * MB,
            })
            .collect()
    }

    fn names(sel: &[&Shot]) -> Vec<String> {
        sel.iter().map(|s| s.path.display().to_string()).collect()
    }

    #[test]
    fn unlimited_prunes_nothing() {
        let s = shots(&[1, 1, 1]);
        assert!(select_for_prune(&s, &Retention::default(), now()).is_empty());
    }

    #[test]
    fn age_limit() {
        let s = shots(&[1, 1, 1, 1]);
        let p = Retention {
            max_age_days: 2,
            ..Default::default()
        };
        assert_eq!(names(&select_for_prune(&s, &p, now())), ["fluxshot_3.png"]);
    }

    #[test]
    fn count_limit_keeps_newest() {
        let s = shots(&[1, 1, 1, 1]);
        let p = Retention {
            max_files: 2,
            ..Default::default()
        };
        assert_eq!(
            names(&select_for_prune(&s, &p, now())),
            ["fluxshot_2.png", "fluxshot_3.png"]
        );
    }

    #[test]
    fn size_limit_keeps_newest_that_fit() {
        let s = shots(&[2, 2, 2]);
        let p = Retention {
            max_size_mb: 5,
            ..Default::default()
        };
        assert_eq!(names(&select_for_prune(&s, &p, now())), ["fluxshot_2.png"]);
    }

    #[test]
    fn combined_limits() {
        let s = shots(&[1, 1, 3, 1, 1]);
        let p = Retention {
            max_age_days: 3,
            max_files: 3,
            max_size_mb: 4,
        };
        // 0,1 kept (2 MB); 2 would make 5 MB; 3 is kept (3 files, 3 MB); 4 is too old.
        assert_eq!(
            names(&select_for_prune(&s, &p, now())),
            ["fluxshot_2.png", "fluxshot_4.png"]
        );
    }

    #[test]
    fn future_mtime_is_not_old() {
        let mut s = shots(&[1]);
        s[0].modified = now() + Duration::from_secs(DAY);
        let p = Retention {
            max_age_days: 1,
            ..Default::default()
        };
        assert!(select_for_prune(&s, &p, now()).is_empty());
    }

    #[test]
    fn list_filters_by_prefix_and_extension() {
        let dir = std::env::temp_dir().join(format!("fluxshot-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("fluxshot_sub.png")).unwrap();
        for f in [
            "fluxshot_a.png",
            "fluxshot_b.PNG",
            "other.png",
            "fluxshot_c.jpg",
        ] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        let mut got: Vec<_> = list(&dir, "fluxshot_")
            .into_iter()
            .map(|s| s.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        got.sort();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(got, ["fluxshot_a.png", "fluxshot_b.PNG"]);
    }

    #[test]
    fn prune_refuses_empty_prefix() {
        let p = Retention {
            max_files: 1,
            ..Default::default()
        };
        assert!(prune(Path::new("/nonexistent"), "", &p).is_err());
    }

    #[test]
    fn human_sizes() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(3 * MB), "3.0 MB");
    }
}
