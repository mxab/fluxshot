mod annotation;
mod app;
mod capture;
mod geom;
mod output;
#[cfg(target_os = "macos")]
mod platform_macos;
mod render;
mod state;
mod toolbar;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about = "Ultra fast screen snippet and annotation tool")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
    /// Directory for saved screenshots [default: ~/Pictures/FluxShot]
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
    /// Internal: hold the clipboard contents piped on stdin (Linux).
    #[cfg(target_os = "linux")]
    #[command(name = output::linux_clipboard::SUBCOMMAND, hide = true)]
    ServeClipboard,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    #[cfg(target_os = "linux")]
    if let Some(Cmd::ServeClipboard) = cli.cmd {
        return output::linux_clipboard::serve();
    }
    let t0 = Instant::now();
    let trace = std::env::var_os("FLUXSHOT_TRACE").is_some();
    if cli.delay > 0 {
        std::thread::sleep(Duration::from_millis(cli.delay));
    }
    let save_dir = cli.path.unwrap_or_else(output::default_dir);

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
                    println!("Copied {}×{} to clipboard", img.width(), img.height());
                }
                Some(app::Outcome::Save(img)) => {
                    let path = output::timestamped_path(&save_dir);
                    output::save_png(&img, &path)?;
                    println!(
                        "Saved {}×{} to {}",
                        img.width(),
                        img.height(),
                        path.display()
                    );
                }
                None => {}
            }
        }
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
                let path = output::timestamped_path(&save_dir);
                output::save_png(&shot.image, &path)?;
                println!("Saved {}", path.display());
            }
        }
    }
    if trace {
        eprintln!("[trace] total {:?}", t0.elapsed());
    }
    Ok(())
}
