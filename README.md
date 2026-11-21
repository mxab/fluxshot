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

**1. Install it** (more options and details under [Install](#-install)):

| Platform | Install |
|---|---|
| 🍎 macOS | `brew install --cask mxab/tap/fluxshot` |
| 🪟 Windows | Download and run [`fluxshot_<version>_windows_amd64_setup.exe`](https://github.com/mxab/fluxshot/releases/latest) |
| 🐧 Debian/Ubuntu | Download the `.deb`, then `sudo apt install ./fluxshot_<version>_linux_amd64.deb` |
| 🐧 Fedora | Download the `.rpm`, then `sudo dnf install ./fluxshot_<version>_linux_amd64.rpm` |

2. Run `fluxshot`.
3. Drag a box around something, draw on it if you like, and press **`Enter`**.
4. Paste it anywhere. 🎉

**Pro tip:** FluxShot really shines when it's on a hotkey like `Print`. See the
[platform notes](#-platform-notes) for how to set that up on your system.

## 📦 Install

Every [release](https://github.com/mxab/fluxshot/releases/latest) has an
installer for each platform, plus a [plain binary](#-plain-binary-any-platform)
if you'd rather manage it yourself.

### 🍎 macOS

**Homebrew** (recommended):

```sh
brew install --cask mxab/tap/fluxshot
```

**Or download** `fluxshot_<version>_darwin_universal.dmg`, open it and drag
FluxShot to *Applications*.

**What you get:** `FluxShot.app` in *Applications* (opening it starts the
[tray](#-tray-mode)) and, with Homebrew, the `fluxshot` command on your `PATH`.
Both are the same binary.

> **First launch:** FluxShot isn't notarized yet. If you installed from the DMG,
> macOS blocks the first launch: open *System Settings → Privacy & Security* and
> click **Open Anyway** (Homebrew handles this for you). FluxShot also needs
> **Screen Recording** permission, which macOS asks for again after each update.
> See [platform notes](#-macos-1).

### 🪟 Windows

**Installer** (recommended): download and run
`fluxshot_<version>_windows_amd64_setup.exe`. It installs just for you, so no
admin rights are needed.

**Or use Scoop:**

```powershell
scoop bucket add mxab https://github.com/mxab/scoop-bucket
scoop install fluxshot
```

**What you get:** *FluxShot* (tray) and *FluxShot Capture* in the Start menu.
The installer can also put `fluxshot` on your `PATH` and start the tray when you
sign in. Just tick the boxes.

> **First launch:** FluxShot isn't code-signed yet, so SmartScreen may warn
> you. Click *More info → Run anyway*.

### 🐧 Linux

Download the `.deb` or `.rpm` for your machine (`amd64` or `arm64`) and install it:

```sh
sudo apt install ./fluxshot_<version>_linux_amd64.deb     # Debian/Ubuntu
sudo dnf install ./fluxshot_<version>_linux_amd64.rpm     # Fedora
```

**What you get:** the `fluxshot` command and an app menu entry with a *Start
Tray* action.

> **Requirements:** glibc 2.39 or newer (Ubuntu 24.04, Debian 13, Fedora 40 or
> later). On GNOME, the tray icon needs the
> [AppIndicator extension](https://extensions.gnome.org/extension/615/appindicator-support/).
> See [platform notes](#-linux-1) for Wayland and hotkey tips.

### 📁 Plain binary (any platform)

Prefer no installer? Grab the archive for your platform, unpack it and put
`fluxshot` somewhere on your `PATH`.

| Archive | Platform |
|---|---|
| `fluxshot_<version>_darwin_arm64.tar.gz` / `_darwin_amd64.tar.gz` | macOS (Apple Silicon / Intel) |
| `fluxshot_<version>_linux_amd64.tar.gz` / `_linux_arm64.tar.gz` | Linux |
| `fluxshot_<version>_windows_amd64.zip` | Windows |

> **macOS:** run `xattr -d com.apple.quarantine fluxshot` once before the first
> launch, and grant Screen Recording to the terminal you start it from.

### ✅ Verifying a download

Every download (archives, DMG, installers, packages) comes with a `.sha256`
checksum file. Each checksum file is signed with
[Sigstore cosign](https://docs.sigstore.dev/) by the release workflow
(keyless), so you can check that it came from this repository's CI:

```sh
ARCHIVE=fluxshot_<version>_<os>_<arch>.tar.gz   # or any other download

cosign verify-blob \
  --bundle "$ARCHIVE.sha256.sigstore.json" \
  --certificate-identity-regexp '^https://github\.com/mxab/fluxshot/\.github/workflows/release\.yml@refs/tags/v' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  "$ARCHIVE.sha256"

# The .sha256 file holds just the hash, so pair it with the file name:
echo "$(cat "$ARCHIVE.sha256")  $ARCHIVE" | shasum -a 256 -c
```

### 🛠️ Build from source

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

Grant **Screen Recording** permission to FluxShot (or, for the bare binary, to the
terminal you launch it from) in *System Settings → Privacy & Security*. Without it,
macOS quietly hands back a picture of just your wallpaper.

Notifications from the bare binary may show up under a generic sender. Install
the app (Homebrew or DMG) to avoid that, or set `notify = false`.

### 🐧 Linux

FluxShot works on X11 and Wayland, including GNOME, KDE and wlroots compositors
such as sway and Hyprland.

The prebuilt packages and binaries need glibc 2.39 or newer (see [Install](#-linux)).
Building from source needs PipeWire 1.0 or newer development headers.

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

Besides the archives, the macOS job builds `FluxShot.app` (DMG + zip,
`packaging/macos/build-app.sh`), the Windows job builds the Inno Setup installer
(`packaging/windows/`), and the Linux jobs build `.deb`/`.rpm` with nfpm. Once
the release is published, follow-up jobs update the Homebrew cask, the Scoop
manifest and winget (stable tags only). These optional repository secrets turn
the extras on:

| Secret | Enables |
|---|---|
| `PACKAGES_TOKEN` | Pushing to `mxab/homebrew-tap` and `mxab/scoop-bucket` (fine-grained PAT, contents: write) |
| `WINGET_TOKEN` | winget PRs (classic PAT, `public_repo`; needs a fork of `microsoft/winget-pkgs` and a first manual submission) |
| `MACOS_CERT_P12`, `MACOS_CERT_PASSWORD`, `MACOS_SIGN_IDENTITY` | Developer ID signing instead of ad-hoc (base64 `.p12`) |
| `MACOS_NOTARY_KEY_P8`, `MACOS_NOTARY_KEY_ID`, `MACOS_NOTARY_ISSUER` | Notarization (App Store Connect API key) |

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
