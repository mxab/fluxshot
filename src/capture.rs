//! Screen capture via `xcap`. Runs *before* any UI exists, so the frozen
//! screen is exactly what the user saw when they pressed the hotkey.

use anyhow::{Context, Result};
use tiny_skia::{IntSize, Pixmap};

pub struct MonitorShot {
    /// Physical-pixel capture of the monitor (opaque, so premultiplied == straight).
    pub image: Pixmap,
    /// Monitor origin as reported by xcap (logical points on macOS, physical elsewhere).
    pub x: i32,
    pub y: i32,
    /// Monitor size in the same units as `x`/`y`.
    pub width: u32,
    pub height: u32,
    /// Image pixels per logical point.
    pub scale: f32,
    pub primary: bool,
}

pub fn capture_all() -> Result<Vec<MonitorShot>> {
    let monitors = xcap::Monitor::all().context("listing monitors")?;
    // Capture every monitor concurrently; on multi-monitor setups this
    // roughly divides capture latency by the monitor count.
    let shots: Vec<Result<MonitorShot>> = std::thread::scope(|s| {
        let handles: Vec<_> = monitors
            .iter()
            .map(|m| s.spawn(move || capture_one(m)))
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("capture thread panicked"))
            .collect()
    });
    let shots: Vec<MonitorShot> = shots.into_iter().collect::<Result<_>>()?;
    anyhow::ensure!(!shots.is_empty(), "no monitors found");
    Ok(shots)
}

fn capture_one(m: &xcap::Monitor) -> Result<MonitorShot> {
    let img = m.capture_image().context(
        "capturing screen (on macOS grant Screen Recording permission in \
         System Settings → Privacy & Security)",
    )?;
    let (w, h) = img.dimensions();
    let mut data = img.into_raw();
    // Force opaque: some backends leave alpha at 0, which would make the
    // premultiplied pixmap invisible.
    for px in data.as_chunks_mut::<4>().0 {
        px[3] = 255;
    }
    let size = IntSize::from_wh(w, h).context("empty capture")?;
    let image = Pixmap::from_vec(data, size).context("building pixmap")?;
    let width = m.width()?;
    Ok(MonitorShot {
        scale: w as f32 / width.max(1) as f32,
        image,
        x: m.x()?,
        y: m.y()?,
        width,
        height: m.height()?,
        primary: m.is_primary().unwrap_or(false),
    })
}
