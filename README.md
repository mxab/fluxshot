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
