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
    #[cfg(target_os = "linux")]
    return linux_clipboard::spawn_server(pm);
    #[cfg(not(target_os = "linux"))]
    {
        let mut cb = arboard::Clipboard::new().context("opening clipboard")?;
        cb.set_image(arboard::ImageData {
            width: pm.width() as usize,
            height: pm.height() as usize,
            bytes: rgba(pm).into(),
        })
        .context("setting clipboard image")?;
        Ok(())
    }
}

/// On X11/Wayland the clipboard owner must keep running to serve paste
/// requests. Like `xclip`/`wl-copy`, hand the image to a detached copy of
/// ourselves (`fluxshot __serve-clipboard`) so the caller can exit right away.
#[cfg(target_os = "linux")]
pub mod linux_clipboard {
    use std::io::{Read, Write};
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    use anyhow::{Context, Result, bail};
    use arboard::SetExtLinux;
    use tiny_skia::Pixmap;

    pub const SUBCOMMAND: &str = "__serve-clipboard";
    const READY: u8 = b'+';

    pub fn spawn_server(pm: &Pixmap) -> Result<()> {
        let exe = std::env::current_exe().context("locating fluxshot executable")?;
        let mut child = Command::new(exe)
            .arg(SUBCOMMAND)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            // Own process group: Ctrl+C in the launching terminal won't kill it.
            .process_group(0)
            .spawn()
            .context("spawning clipboard helper")?;
        {
            let mut stdin = child.stdin.take().context("helper stdin")?;
            let mut header = [0u8; 8];
            header[..4].copy_from_slice(&pm.width().to_le_bytes());
            header[4..].copy_from_slice(&pm.height().to_le_bytes());
            stdin.write_all(&header)?;
            stdin.write_all(&super::rgba(pm))?;
        } // closing stdin signals end of data
        let mut ready = [0u8; 1];
        let ok = child
            .stdout
            .take()
            .context("helper stdout")?
            .read_exact(&mut ready)
            .is_ok()
            && ready[0] == READY;
        if !ok {
            let _ = child.wait();
            bail!("clipboard helper failed to take the clipboard");
        }
        // Deliberately not waiting: the helper lives on until another app
        // takes the clipboard.
        Ok(())
    }

    /// Entry point of the helper process.
    pub fn serve() -> Result<()> {
        let mut input = Vec::new();
        std::io::stdin().lock().read_to_end(&mut input)?;
        anyhow::ensure!(input.len() >= 8, "short clipboard input");
        let (header, bytes) = input.split_at(8);
        let w = u32::from_le_bytes(header[..4].try_into()?) as usize;
        let h = u32::from_le_bytes(header[4..].try_into()?) as usize;
        anyhow::ensure!(bytes.len() == w * h * 4, "clipboard input size mismatch");
        let img = || arboard::ImageData {
            width: w,
            height: h,
            bytes: bytes.into(),
        };
        let mut cb = arboard::Clipboard::new().context("opening clipboard")?;
        // Take ownership first so the parent can report success and exit,
        // then block serving requests until someone else owns the clipboard.
        cb.set_image(img()).context("setting clipboard image")?;
        {
            let mut out = std::io::stdout().lock();
            out.write_all(&[READY])?;
            out.flush()?;
        }
        cb.set()
            .wait()
            .image(img())
            .context("serving clipboard image")?;
        Ok(())
    }
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
