//! Clipboard and file output.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tiny_skia::Pixmap;

/// Straight-alpha RGBA bytes (our images are opaque, but demultiply anyway
/// so translucent edge cases stay correct).
fn rgba(pm: &Pixmap) -> Vec<u8> {
    pm.pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect()
}

pub fn to_clipboard(pm: &Pixmap) -> Result<()> {
    let mut cb = arboard::Clipboard::new().context("opening clipboard")?;
    let img = arboard::ImageData {
        width: pm.width() as usize,
        height: pm.height() as usize,
        bytes: rgba(pm).into(),
    };
    #[cfg(target_os = "linux")]
    {
        // On X11/Wayland the owning process must serve the data; block until
        // another app takes over the clipboard.
        use arboard::SetExtLinux;
        cb.set()
            .wait()
            .image(img)
            .context("setting clipboard image")?;
    }
    #[cfg(not(target_os = "linux"))]
    cb.set_image(img).context("setting clipboard image")?;
    Ok(())
}

pub fn default_dir() -> PathBuf {
    directories::UserDirs::new()
        .and_then(|u| u.picture_dir().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("FluxShot")
}

pub fn timestamped_path(dir: &Path) -> PathBuf {
    let ts = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    let mut path = dir.join(format!("fluxshot_{ts}.png"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("fluxshot_{ts}_{n}.png"));
        n += 1;
    }
    path
}

pub fn save_png(pm: &Pixmap, path: &Path) -> Result<()> {
    use image::ImageEncoder;
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let file =
        std::fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    // Fast compression: screenshots compress well anyway and latency matters more.
    PngEncoder::new_with_quality(
        std::io::BufWriter::new(file),
        CompressionType::Fast,
        FilterType::Adaptive,
    )
    .write_image(
        &rgba(pm),
        pm.width(),
        pm.height(),
        image::ExtendedColorType::Rgba8,
    )
    .context("encoding PNG")?;
    Ok(())
}
