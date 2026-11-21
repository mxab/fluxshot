# FluxShot

Ultra fast Screen Snippet and annotation Tool

- Build with rust
- Native for Linux, Mac and Windows

## Usage

```sh
cargo build --release
./target/release/fluxshot            # select region + annotate
./target/release/fluxshot full       # capture all monitors to PNG, no UI
FLUXSHOT_TRACE=1 ./target/release/fluxshot   # print latency timings
```

macOS: grant **Screen Recording** to your terminal (System Settings → Privacy & Security),
otherwise captures come back as just the wallpaper.

### Linux

Works on X11 and Wayland (GNOME, KDE, wlroots compositors like sway/Hyprland).
Build dependencies:

```sh
# Debian/Ubuntu
sudo apt install pkg-config clang libclang-dev libpipewire-0.3-dev libxcb1-dev \
  libxcb-randr0-dev libxkbcommon-dev libwayland-dev libdbus-1-dev libegl-dev libgbm-dev
# Fedora
sudo dnf install pkgconf clang clang-devel pipewire-devel libxcb-devel \
  libxkbcommon-devel wayland-devel dbus-devel mesa-libEGL-devel mesa-libgbm-devel
```

- There is no built-in global hotkey: bind `fluxshot` to a key (e.g. `Print`)
  in your desktop's keyboard settings, or in sway/Hyprland config.
- Wayland: monitors are discovered through XWayland, so it must be enabled
  (it is by default on GNOME, KDE and sway). GNOME/KDE capture through the
  desktop portal and may ask for screenshot permission the first time.
- Copying leaves a small background `fluxshot __serve-clipboard` process
  that serves the image (X11/Wayland clipboards are owned by a live process,
  same as `xclip`/`wl-copy`). It exits as soon as something else is copied.

### Windows

Build with the MSVC toolchain (`rustup default stable-msvc`, plus the Visual
Studio C++ Build Tools). Windows 10 1703 or newer.

- There is no built-in global hotkey: set a "Shortcut key" on a desktop/Start
  menu shortcut to `fluxshot.exe`, or use PowerToys Keyboard Manager. To use
  `PrtScn`, first turn off *Use the Print screen key to open screen capture*
  in Settings → Accessibility → Keyboard.
- Release builds don't open a console window; when started from a terminal
  they still print to it (after the prompt returns).

| Key | Action |
|---|---|
| drag | select region (Select tool: move/resize with handles) |
| `v` `p` `l` `a` `r` `e` `m` `x` | select, pen, line, arrow, rect, ellipse, marker, pixelate |
| `1`–`6` | color |
| `+` / `-` / scroll | stroke width |
| `Cmd/Ctrl+Z`, `Cmd/Ctrl+Shift+Z` | undo / redo |
| `Enter`, `Cmd/Ctrl+C` | copy to clipboard and exit |
| `Cmd/Ctrl+S` | save PNG to `~/Pictures/FluxShot` (`--path` to override) |
| `Esc` | cancel the current drag, or quit |
