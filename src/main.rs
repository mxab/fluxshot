// No console window when launched from a hotkey (see platform_windows::init).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod annotation;
mod app;
mod capture;
mod config;
mod geom;
mod library;
mod notify;
mod output;
#[cfg(target_os = "macos")]
mod platform_macos;
#[cfg(windows)]
mod platform_windows;
mod render;
mod state;
mod toolbar;
mod tray;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about = "Ultra fast screen snippet and annotation tool")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
    /// Directory for saved screenshots [default: `save_dir` setting, else ~/Pictures/FluxShot]
    #[arg(long, global = true)]
    path: Option<PathBuf>,
    /// Wait this many milliseconds before capturing
    #[arg(long, global = true, default_value_t = 0)]
    delay: u64,
}

#[derive(Subcommand)]
enum Cmd {
    /// Select a region and annotate it (default)
    Gui,
    /// Capture every monitor straight to PNG files, no UI
    Full {
        /// Copy the primary monitor to the clipboard instead of saving
        #[arg(long)]
        clipboard: bool,
    },
    /// Stay in the system tray: capture, browse and prune screenshots
    Tray,
    /// Internal: hold the clipboard contents piped on stdin (Linux).
    #[cfg(target_os = "linux")]
    #[command(name = output::linux_clipboard::SUBCOMMAND, hide = true)]
    ServeClipboard,
}

fn main() -> Result<()> {
    #[cfg(windows)]
    platform_windows::init();
    let cli = Cli::parse();
    #[cfg(target_os = "linux")]
    if let Some(Cmd::ServeClipboard) = cli.cmd {
        return output::linux_clipboard::serve();
    }
    if let Some(Cmd::Tray) = cli.cmd {
        return tray::run(cli.path);
    }
    let cfg = config::Config::load_or_default();
    let t0 = Instant::now();
    let trace = std::env::var_os("FLUXSHOT_TRACE").is_some();
    if cli.delay > 0 {
        std::thread::sleep(Duration::from_millis(cli.delay));
    }
    let save_dir = cli.path.unwrap_or_else(|| cfg.save_dir());
    let save = |img: &tiny_skia::Pixmap| -> Result<PathBuf> {
        let path = output::timestamped_path(&save_dir, &cfg.filename);
        output::save_png(img, &path, cfg.png_compression)?;
        Ok(path)
    };
    let notify = |summary: &str, body: String| {
        if cfg.notify {
            notify::info(summary, &body);
        }
    };

    let shots = capture::capture_all()?;
    if trace {
        eprintln!(
            "[trace] captured {} monitor(s) in {:?}",
            shots.len(),
            t0.elapsed()
        );
    }

    match cli.cmd.unwrap_or(Cmd::Gui) {
        Cmd::Gui => {
            let opts = app::Options { t0, trace };
            // Output happens after the overlay is gone, so slow encoding or
            // clipboard hand-off never keeps a frozen window on screen.
            match app::run(shots, opts)? {
                Some(app::Outcome::Copy(img)) => {
                    output::to_clipboard(&img)?;
                    let msg = format!("Copied {}×{} to clipboard", img.width(), img.height());
                    println!("{msg}");
                    if cfg.save_on_copy {
                        let path = save(&img)?;
                        println!("Saved {}", path.display());
                    }
                    notify("Screenshot copied", msg);
                }
                Some(app::Outcome::Save(img)) => {
                    let path = save(&img)?;
                    println!(
                        "Saved {}×{} to {}",
                        img.width(),
                        img.height(),
                        path.display()
                    );
                    notify("Screenshot saved", path.display().to_string());
                }
                None => {}
            }
        }
        Cmd::Tray => unreachable!("handled above"),
        Cmd::Full { clipboard: true } => {
            let shot = shots.iter().find(|s| s.primary).unwrap_or(&shots[0]);
            output::to_clipboard(&shot.image)?;
            println!(
                "Copied {}×{} to clipboard",
                shot.image.width(),
                shot.image.height()
            );
        }
        #[cfg(target_os = "linux")]
        Cmd::ServeClipboard => unreachable!("handled above"),
        Cmd::Full { clipboard: false } => {
            for shot in &shots {
                let path = save(&shot.image)?;
                println!("Saved {}", path.display());
            }
        }
    }
    if trace {
        eprintln!("[trace] total {:?}", t0.elapsed());
    }
    Ok(())
}
