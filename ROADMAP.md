# 🧭 Roadmap

FluxShot is aimed at **developers**: the people who paste screenshots into bug
reports, PRs, docs and chat. The guiding rules:

- **The core stays tiny.** `fluxshot` starts, captures and exits. Heavier
  features are opt-in, and nothing adds a background daemon.
- **No built-in uploads.** A post-capture hook lets you send shots wherever you
  like (`gh`, S3, your own script).
- **Trust before features.** Installing and first launch should be painless
  before the feature list grows.

Each item has a tracking issue. Plans can change, so the
[issue tracker](https://github.com/mxab/fluxshot/issues) has the latest.

## v0.3.x: Trust & reach

- **Verify the Windows installer** end-to-end. It's untested in a real install.
- **winget**: fork `winget-pkgs`, set `WINGET_TOKEN`, first manual submission. The release job already supports it.
- **Windows code signing** via the SignPath Foundation OSS program, so SmartScreen stops warning.
- **macOS Developer ID + notarization**. The workflow already supports it once the secrets are set. This removes *Open Anyway* and having to grant Screen Recording again after every update.
- **Demo GIF** in the README, the quickest way to show what FluxShot does.
- **More Linux channels**: Flatpak/Flathub, AUR, Nix. These reach distros older than glibc 2.39.

## v0.4: Annotation essentials + scripting

- **Text tool**: `ab_glyph` and one small bundled font, rendered with tiny-skia. Output looks the same on every OS.
- **Numbered step markers**: auto-incrementing circles for "click here, then here".
- **Select, move and resize existing shapes** instead of undoing and redrawing.
- **Blur tool** next to Pixelate.
- **Scripting CLI**: `--stdout` (PNG to stdout), `--region x,y,w,h`, `--last-region`, and exit codes that tell a cancel apart from an error.
- **Post-capture hook**: a setting that runs a command with the saved file's path. This is how uploads work.

## v0.5: Speed

- **Window snapping**: hover highlights a window and a click selects it. Planned for macOS, Windows, X11, and Hyprland/sway (via IPC). GNOME and KDE on Wayland don't expose window geometry, so snapping won't be available there.
- **Pin to screen**: keep a shot floating on top while you work. A one-shot run stays alive until its last pin is closed, and the tray owns pins when it's running.

## v0.6: Text intelligence & polish

- **Copy text (OCR)**: macOS Vision and Windows.Media.Ocr. Nothing extra to ship. On Linux, `tesseract` is used if it's installed.
- **Redaction suggestions**: highlight likely secrets (emails, tokens, keys, IPs) for you to confirm, then pixelate/blur. Nothing is ever redacted silently.
- **Export polish**: optional padding, drop shadow and rounded corners for good-looking PR and docs screenshots.
