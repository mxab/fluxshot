<div align="center">

# ⚡ FluxShot

**Snap it. Mark it. Ship it.**

A tiny, blazing-fast screenshot and annotation tool for macOS, Linux and Windows.

[![Latest release](https://img.shields.io/github/v/release/mxab/fluxshot?style=flat-square)](https://github.com/mxab/fluxshot/releases/latest)
![Platforms](https://img.shields.io/badge/platforms-macOS%20%7C%20Linux%20%7C%20Windows-blue?style=flat-square)
![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange?style=flat-square&logo=rust)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-green?style=flat-square)](#-license)

<!-- Drop a demo here once you have one:
![FluxShot in action](docs/demo.gif)
-->

</div>

---

You want to show someone *that one thing* on your screen. With FluxShot you press
a key, drag over it, scribble an arrow, and press Enter. The image is on your
clipboard before you've let go of the mouse. That's the whole workflow.

## ✨ Why FluxShot?

- **Instant.** The overlay pops up right away, freezing your screen exactly as it was.
- **Annotate in place.** Arrows, boxes, ellipses, a highlighter, freehand pen, and pixelate for hiding secrets.
- **One key to finish.** `Enter` copies to the clipboard, `Cmd/Ctrl+S` saves a PNG.
- **Native everywhere.** macOS, Windows, and Linux on both X11 *and* Wayland (GNOME, KDE, sway, Hyprland…).
- **Just a binary.** No runtime and nothing running in the background. It starts, does its job, and gets out of your way.
- **Optional tray.** Want a hotkey, a history of recent shots and automatic cleanup? Run `fluxshot tray`. It's opt-in.

## 🚀 Quick start

1. Grab the archive for your platform from the [**latest release**](https://github.com/mxab/fluxshot/releases/latest) and unpack it.
2. Run `fluxshot`.
3. Drag a box around something, draw on it if you like, and press **`Enter`**.
4. Paste it anywhere. 🎉

**Pro tip:** FluxShot really shines when it's on a hotkey like `Print`. See the
[platform notes](#-platform-notes) for how to set that up on your system.

## 📦 Install

### Prebuilt binaries

| Platform | Download |
|---|---|
| macOS (Apple Silicon) | `fluxshot_<version>_darwin_arm64.tar.gz` |
| macOS (Intel) | `fluxshot_<version>_darwin_amd64.tar.gz` |
| Linux x86_64 | `fluxshot_<version>_linux_amd64.tar.gz` |
| Linux ARM64 | `fluxshot_<version>_linux_arm64.tar.gz` |
| Windows x64 | `fluxshot_<version>_windows_amd64.zip` |

All of them are on the [releases page](https://github.com/mxab/fluxshot/releases).

> **macOS:** the binaries aren't signed yet. After downloading, run
> `xattr -d com.apple.quarantine fluxshot` once before the first launch.

#### Verifying a download

Every archive comes with a `.sha256` checksum file. Each checksum file is signed
with [Sigstore cosign](https://docs.sigstore.dev/) by the release workflow
(keyless), so you can check that it came from this repository's CI:

```sh
ARCHIVE=fluxshot_<version>_<os>_<arch>.tar.gz   # or the .zip on Windows

cosign verify-blob \
  --bundle "$ARCHIVE.sha256.sigstore.json" \
  --certificate-identity-regexp '^https://github\.com/mxab/fluxshot/\.github/workflows/release\.yml@refs/tags/v' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  "$ARCHIVE.sha256"

# The .sha256 file holds just the hash, so pair it with the file name:
echo "$(cat "$ARCHIVE.sha256")  $ARCHIVE" | shasum -a 256 -c
```

### Build from source

You'll need a recent stable [Rust toolchain](https://rustup.rs) (edition 2024).

```sh
git clone https://github.com/mxab/fluxshot
cd fluxshot
cargo build --release
./target/release/fluxshot
```

Linux and Windows need a few extra build dependencies. They're listed in the
[platform notes](#-platform-notes).

## 🖱️ Usage

| Command | What it does |
|---|---|
| `fluxshot` | Select a region, annotate it, then copy or save |
| `fluxshot full` | Save every monitor straight to PNG, no UI |
| `fluxshot full --clipboard` | Copy the primary monitor to the clipboard, no UI |
| `fluxshot tray` | Stay in the system tray (see [Tray mode](#-tray-mode)) |
| `--path <dir>` | Where to save PNGs (default: `save_dir` setting, else `~/Pictures/FluxShot`) |
| `--delay <ms>` | Wait before capturing |
| `FLUXSHOT_TRACE=1 fluxshot` | Print latency timings, so you can see how fast it really is |

A few handy recipes:

```sh
# Capture a menu or tooltip that closes when it loses focus
fluxshot --delay 2000

# Archive all screens into a project folder
fluxshot full --path ~/work/bug-1234

# How quick is it on your machine?
FLUXSHOT_TRACE=1 fluxshot
```

If you finish without selecting anything, FluxShot takes the whole monitor under
your pointer.

## 🗂️ Tray mode

`fluxshot tray` puts a ⚡ icon in the menu bar or system tray. From there you can:

- **Capture.** Click *Capture* or press the global hotkey (`Cmd/Ctrl+Shift+2` by
  default). On Linux, left-clicking the icon also captures.
- **Recent.** See your last screenshots, then copy one to the clipboard again,
  open it, show it in the file manager, or move it to the trash.
- **Prune.** Move screenshots that are older or larger than your limits to the
  trash. This also runs automatically when the tray starts and once a day.
- **Launch at login** with a checkbox.
- **Edit and reload settings.**

Plain `fluxshot` still works the same, with or without the tray running.

## ⚙️ Settings

Settings live in `config.toml`. *Edit settings…* in the tray creates it with
every option commented out. Its location:

| Platform | Path |
|---|---|
| macOS | `~/Library/Application Support/fluxshot/config.toml` |
| Linux | `~/.config/fluxshot/config.toml` |
| Windows | `%APPDATA%\fluxshot\config\config.toml` |

Every key is optional. Command-line flags take precedence.

```toml
save_dir = "~/Pictures/FluxShot"
filename = "fluxshot_%Y-%m-%d_%H-%M-%S"  # strftime; ".png" is appended
save_on_copy = false                     # Enter also saves a PNG (full history)
png_compression = "fast"                 # fast | default | best
notify = true                            # notification after copy/save
# autostart = true                       # if set, enforced on tray start; else use the menu toggle

[hotkey]
capture = "CmdOrCtrl+Shift+2"            # "" disables; tray mode only

[retention]                              # 0 = unlimited
max_age_days = 0
max_files = 0
max_size_mb = 0

[tray]
recent_count = 10
```

**Pruning is careful.** It only touches `.png` files directly inside `save_dir`
whose names start with the fixed part of `filename` (`fluxshot_` by default).
Files go to the trash, not straight to deletion. If `filename` starts with a
`%` specifier, pruning is turned off.

## ⌨️ Keyboard shortcuts

**Tools**

| Key | Tool |
|---|---|
| `v` | Select (move and resize the region with its handles) |
| `p` | Pen |
| `l` | Line |
| `a` | Arrow |
| `r` | Rectangle |
| `e` | Ellipse |
| `m` | Marker (highlighter) |
| `x` | Pixelate |

**Style**

| Key | Action |
|---|---|
| `1`–`6` | Pick a color |
| `+` / `-` / scroll | Stroke width |

**Edit and finish**

| Key | Action |
|---|---|
| `Cmd/Ctrl+Z` | Undo |
| `Cmd/Ctrl+Shift+Z`, `Cmd/Ctrl+Y` | Redo |
| `Enter`, `Cmd/Ctrl+C` | Copy to clipboard and exit |
| `Cmd/Ctrl+S` | Save PNG and exit |
| `Esc` | Cancel the current drag, or quit |

## 🧭 Platform notes

### 🍎 macOS

Grant **Screen Recording** permission to FluxShot (or to the terminal you launch it from)
in *System Settings → Privacy & Security*. Without it, macOS quietly hands back
a picture of just your wallpaper.

Notifications from the bare binary may show up under a generic sender, because
FluxShot isn't packaged as an `.app` bundle yet. Set `notify = false` if they bother you.

### 🐧 Linux

FluxShot works on X11 and Wayland, including GNOME, KDE and wlroots compositors
such as sway and Hyprland.

The prebuilt Linux binaries need glibc 2.39 or newer (Ubuntu 24.04, Debian 13,
Fedora 40 or later). Building from source needs PipeWire 1.0 or newer development headers.

**Build dependencies**

```sh
# Debian/Ubuntu
sudo apt install pkg-config clang libclang-dev libpipewire-0.3-dev libxcb1-dev \
  libxcb-randr0-dev libxkbcommon-dev libwayland-dev libdbus-1-dev libegl-dev libgbm-dev

# Fedora
sudo dnf install pkgconf clang clang-devel pipewire-devel libxcb-devel \
  libxkbcommon-devel wayland-devel dbus-devel mesa-libEGL-devel mesa-libgbm-devel
```

**Good to know**

- **Hotkey:** bind `fluxshot` to a key such as `Print` in your desktop's keyboard
  settings, or in your sway/Hyprland config. The tray's own hotkey only works on
  X11, because Wayland doesn't allow apps to grab global keys.
- **Tray:** FluxShot uses the StatusNotifierItem protocol. KDE, waybar and most panels
  support it out of the box. GNOME needs the
  [AppIndicator extension](https://extensions.gnome.org/extension/615/appindicator-support/).
- **Wayland:** monitors are discovered through XWayland, so XWayland must be enabled.
  It is by default on GNOME, KDE and sway. GNOME and KDE capture through the
  desktop portal and may ask for screenshot permission the first time.
- **Clipboard:** after copying, a small background `fluxshot __serve-clipboard`
  process keeps the image available. X11 and Wayland clipboards are owned by a
  live process, the same way `xclip` and `wl-copy` work. It exits as soon as
  something else is copied.

### 🪟 Windows

Requires Windows 10 1703 or newer.

**Building:** use the MSVC toolchain (`rustup default stable-msvc`) plus the
Visual Studio C++ Build Tools.

**Good to know**

- **Hotkey:** run `fluxshot tray` and use its hotkey. You can also set a *Shortcut key*
  on a desktop or Start menu shortcut to `fluxshot.exe`, or use PowerToys Keyboard Manager.
  To use `PrtScn`, first turn off *Use the Print screen key to open screen
  capture* in *Settings → Accessibility → Keyboard*.
- **Console:** release builds don't open a console window. When started from a
  terminal they still print to it, and the output appears after the prompt returns.

## 🤝 Contributing

Ideas, bug reports and pull requests are all very welcome! If something feels
slow, clunky or broken, [open an issue](https://github.com/mxab/fluxshot/issues).
Run it with `FLUXSHOT_TRACE=1` and include the output if you can.

### Releasing

Releases are built by [GoReleaser](https://goreleaser.com) in
`.github/workflows/release.yml`. Each platform builds natively on its own runner
from its own config in `.goreleaser/`. The jobs upload into a shared draft
release, which is published once every build succeeds.

```sh
# Bump `version` in Cargo.toml, commit, then:
git tag v0.2.0 && git push origin v0.2.0

# Local dry run (no publishing), e.g. on a Mac:
goreleaser release --snapshot --clean --skip=sign --config .goreleaser/macos.yaml
```

## 📄 License

FluxShot is dual-licensed under either of

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.

Unless you explicitly state otherwise, any contribution you intentionally submit
for inclusion in FluxShot, as defined in the Apache-2.0 license, shall be
dual-licensed as above, without any additional terms or conditions.

---

<div align="center">

Made with 🦀 and ☕. Happy snapping!

</div>
