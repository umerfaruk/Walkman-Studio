# Windows app assets

Assets consumed by the Windows (Rust) build. Images are shared with the macOS app
and live in the repo-root `assets/` folder (device images, SensMe channel covers,
icons); the packaging step (spec task 19.1) copies the needed ones here.

## Bundled binaries (`assets/bin/`)

The app bundles FFmpeg for transcoding, M4A tagging, waveform fallback, and probing.
Place the Windows binaries here before packaging (they are git-ignored):

- `assets/bin/ffmpeg.exe`
- `assets/bin/ffprobe.exe`

Binary discovery prefers these bundled binaries, then falls back to `PATH`
(`services/transcode.rs`, task 10.1).

## Icons (`assets/icons/`)

SF Symbols are macOS-only. The Windows build ships an equivalent glyph set — the
[Lucide](https://lucide.dev) icon set (ISC-licensed) — with a fixed mapping from each
SF Symbol used in the sidebar/toolbar to a Lucide glyph (`ui/theme.rs`, module `icon`,
spec task 3.4).

The glyph assets (an icon font or per-glyph SVGs) are loaded at build time, mirroring the
bundled fonts: `ui::theme::bundled_icon_font` embeds them via `include_bytes!` **only** when
the `walkman_bundled_icons` cfg is set and the file is present. The files are git-ignored
and vendored by the Windows packaging step (task 19.1):

- `assets/icons/lucide.ttf`  (the Lucide glyph font, or equivalent)

When the cfg is not set (the default, incl. non-Windows dev hosts), the slot resolves to
`None` and icon-bearing views fall back to `Icon::fallback_text` (a short mono-faced label)
so every row still renders legibly.
## Bundled fonts (`assets/fonts/`)

The theme typography (`ui/theme.rs`, spec task 3.1) uses a Windows-system-like sans and a
monospace face. The loader (`ui::theme::install_fonts`) embeds these at build time via
`include_bytes!` **only** when the `walkman_bundled_fonts` cfg is set and the files are
present. The files are git-ignored (Segoe UI ships with Windows but is not redistributable).

Expected files when bundling (set `RUSTFLAGS="--cfg walkman_bundled_fonts"` in the Windows
packaging step, task 19.1, after vendoring these from the target machine / a licensed source):

- `assets/fonts/segoeui.ttf`          (Segoe UI Regular)
- `assets/fonts/segoeui-medium.ttf`   (Segoe UI Medium)
- `assets/fonts/segoeui-semibold.ttf` (Segoe UI Semibold)
- `assets/fonts/segoeui-bold.ttf`     (Segoe UI Bold)
- `assets/fonts/CascadiaMono.ttf`     (monospace face for metadata/timestamps/badges)

When the cfg is not set (the default, incl. non-Windows dev hosts), the slots resolve to
`None` and `install_fonts` falls back to egui's built-in proportional/monospace faces, so
every typography constant still renders.
