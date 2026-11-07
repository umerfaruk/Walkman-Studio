# Design — Walkman Studio for Windows (Rust Port)

## Overview

This document describes how to port Walkman Studio (macOS/Swift/SwiftUI) to Windows in Rust while
keeping the UI and every option identical. The strategy is a **layered port**: pure, portable
domain logic is translated 1:1 from Swift to Rust, while every macOS platform integration is
replaced by a Windows equivalent behind a trait-based platform abstraction. The UI is rebuilt in a
Rust GUI toolkit using a shared design-token module so the "Optical Liquid Glass & Obsidian" look
is reproduced faithfully.

The source app's structure (from the macOS codebase) maps cleanly onto Rust modules:

| macOS (Swift) | Windows (Rust) |
| --- | --- |
| SwiftUI views (`Views/*.swift`) | `ui/` view modules in the chosen toolkit |
| `Theme.swift` | `ui/theme.rs` design tokens + reusable "glass" widgets |
| `Models.swift` | `domain/models.rs` |
| `WalkmanEngine.swift`, `*Service.swift`, `*Detector.swift` | `services/*.rs` |
| AppKit / AVFoundation / IOKit / ImageIO / CryptoKit / SQLite3 | `platform/windows/*.rs` + Rust crates |
| FFmpeg/ffprobe (external binaries) | bundled `ffmpeg.exe` / `ffprobe.exe` |

### Research note on the GUI toolkit

The macOS app relies on SwiftUI features — `NavigationSplitView`, a trailing inspector, custom
toolbar, `.ultraThinMaterial` vibrancy, SF Symbols, and custom `NSWindow`/`NSSplitView`
manipulation. To reproduce this on Windows the port needs a toolkit with: custom-drawn theming
(for the obsidian/glass look), retained layout with split panes, tables/grids, hover/press states,
and native window control.

**Recommended:** build the UI with an immediate-or-retained Rust GUI framework that supports
fully custom theming. Two viable tracks:

- **Primary recommendation — `egui`/`eframe`:** mature, pure-Rust, fully custom-styled, easy to
  reproduce flat obsidian surfaces, pill/badge widgets, grids, and tables; good cross-arch Windows
  support; straightforward bundling. Blur/acrylic is approximated with translucent fills (the
  source design forbids glow and uses flat surfaces, so this is a close match). This is the
  pragmatic choice for pixel-level control and a single-binary deliverable.
- **Alternative — `slint` or `dioxus`:** declarative markup closer to SwiftUI's style; `slint` has
  strong theming and native Windows backends. Viable if the team prefers declarative UI; costs more
  up-front styling work to match tokens exactly.

The rest of this design is toolkit-agnostic where possible and calls out `egui` specifics as the
reference implementation. The toolkit decision is a task (see tasks.md T2) and can be revisited,
but all design tokens, layouts, and behaviors below are specified independently of it.

SF Symbols are not available on Windows; the port will ship an icon set (e.g. a bundled SVG/
icon-font such as Lucide/Fluent/Tabler) with a fixed mapping from each SF Symbol used in the
sidebar/toolbar to a visually-equivalent glyph.

## Architecture

```
walkman-studio/                      (Rust workspace)
├─ Cargo.toml                        (workspace)
├─ crates/
│  ├─ app/            # binary: window bootstrap, wiring, app state
│  │   └─ main.rs
│  ├─ ui/             # all views + theme + reusable glass widgets
│  │   ├─ theme.rs            (colors, fonts, spacing, glass widgets, icons)
│  │   ├─ shell.rs            (window, sidebar, toolbar, inspector, now-playing bar)
│  │   ├─ sidebar.rs
│  │   ├─ views/              (one module per navigation section + sheets)
│  │   └─ widgets/            (pills, badges, gauges, search bar, waveform, carousel)
│  ├─ domain/         # portable models + pure logic (no platform deps)
│  │   ├─ models.rs           (Track, Album, Artist, Device, Drive, Playlist, ...)
│  │   ├─ artist_parse.rs     (primary/featured artist, known-exceptions)
│  │   ├─ sanitize.rs         (Walkman text/filename sanitization)
│  │   └─ senseme.rs          (12-Tone analysis math + channel classification)
│  ├─ services/      # app services (orchestrate platform + domain)
│  │   ├─ library_scanner.rs  (SQLite cache, incremental scan)
│  │   ├─ local_library.rs
│  │   ├─ device_detector.rs  (Sony catalog, volume signatures)
│  │   ├─ usb_detector.rs     (VID/PID enumeration)
│  │   ├─ walkman_engine.rs   (paths, playlists, drive detect, eject, junk clean)
│  │   ├─ audio_tagger.rs     (FLAC/MP3/M4A read+write)
│  │   ├─ artwork.rs          (baseline JPEG encode/validate, export)
│  │   ├─ transcode.rs        (ffmpeg presets, transfer pipeline)
│  │   ├─ transfer_queue.rs
│  │   ├─ duplicate_finder.rs
│  │   ├─ duplicate_guard.rs
│  │   ├─ backup_restore.rs   (PBKDF2 + AES-256-GCM, copy, free-space)
│  │   ├─ trim_cut.rs         (waveform + ffmpeg filtergraph)
│  │   ├─ lyrics.rs           (LRCLIB + Lyrics.ovh)
│  │   ├─ online_metadata.rs  (iTunes + Deezer)
│  │   ├─ artist_art_cache.rs
│  │   ├─ cover_art_cache.rs
│  │   └─ player.rs           (playback + SMTC)
│  └─ platform/      # OS abstraction (traits) + Windows impls
│      ├─ mod.rs             (traits: Volumes, DeviceEvents, FileDialogs, Eject, MediaControls)
│      └─ windows/           (Win32 implementations)
└─ assets/            # icons, device images, SensMe channel covers, ffmpeg binaries
```

### Layering rules

- `domain` has **no** platform or I/O dependencies; it is unit-testable and ported from Swift 1:1.
- `services` depend on `domain` and on `platform` traits, not on concrete OS APIs directly (except
  cross-platform crates like `rusqlite`, `reqwest`, `image`).
- `platform::windows` is the only place that touches Win32 / WinRT.
- `ui` depends on `services` and `domain`; it holds no business logic beyond view state.

### Threading model

A single UI thread owns view state. Long operations (scan, transcode, backup, network) run on a
worker runtime (`tokio` for async I/O/HTTP; `rayon` or scoped threads for CPU-bound analysis) and
report progress to the UI via channels (`std::sync::mpsc` or `tokio` channels polled each frame).
This replaces Swift's `@MainActor` + `Task` + `@Published` model. Shared service state uses
`Arc<Mutex<…>>` or message passing; the UI never blocks on I/O.

## Platform abstraction (macOS → Windows mapping)

Each macOS integration becomes a trait method with a Windows implementation.

| Concern | macOS source | Windows implementation (crate / API) |
| --- | --- | --- |
| Volume enumeration & capacity | `/Volumes` scan, `statvfs`, `statfs` | `GetLogicalDrives`, `GetDriveTypeW`, `GetVolumeInformationW`, `GetDiskFreeSpaceExW` (`windows` crate) |
| Volume attach/detach events | `NSWorkspace` mount/unmount notifications | `WM_DEVICECHANGE` + `RegisterDeviceNotification`, or WMI `Win32_VolumeChangeEvent`; plus periodic poll |
| USB Walkman VID/PID | IOKit `IOServiceMatching(kIOUSBDeviceClassName)` | SetupAPI (`SetupDiGetClassDevs` / `SP_DEVICE_INTERFACE`) or WMI `Win32_PnPEntity`, filter VID `054C` + PID table |
| Safe eject / unmount | `diskutil unmount` / `NSWorkspace.unmountAndEject` | `DeviceIoControl(IOCTL_STORAGE_EJECT_MEDIA)` + `FSCTL_LOCK_VOLUME`/`DISMOUNT_VOLUME`, or `mountvol /p` fallback |
| Junk-file cleanup | `dot_clean -m`, remove `._*`/`.DS_Store` | delete `._*`,`.DS_Store`,`Thumbs.db`,`desktop.ini`,`$RECYCLE.BIN`; no external tool |
| Open folder picker | `NSOpenPanel` | `IFileOpenDialog` (COM) via `windows` crate, or `rfd` crate |
| Save file dialog | `NSSavePanel` | `IFileSaveDialog` / `rfd` |
| File permissions (TCC) | security-scoped bookmarks, TCC prompts | **dropped** — Windows uses standard file ACLs; store plain paths |
| Reveal in file manager | Finder reveal | `explorer.exe /select,<path>` |
| Baseline JPEG encode/validate | ImageIO / CoreGraphics | `image` crate (`jpeg` baseline encoder, resize) + header inspection; or WIC |
| Audio decode for analysis/waveform | AVFoundation (`AVAudioFile`, PCM) | `symphonia` (pure Rust) or `ffmpeg` decode to raw PCM |
| Audio playback | `AVPlayer` | `rodio` (+ `symphonia`) or Windows Media Foundation |
| OS now-playing / media keys | `MPNowPlayingInfoCenter`, remote commands | `SystemMediaTransportControls` (WinRT via `windows` crate) |
| Crypto (backup) | CryptoKit + CommonCrypto PBKDF2 | RustCrypto: `pbkdf2` + `hmac` + `sha2`, `aes-gcm` |
| Fast file copy + flush | Darwin `copyfile`, `sync()` | `CopyFileExW` + `FlushFileBuffers`, or buffered copy |
| SQLite cache | `libsqlite3` | `rusqlite` (bundled SQLite) — near-direct port |
| HTTP | `URLSession` | `reqwest` (async, rustls) |
| Transcode / M4A tag / probe | FFmpeg/ffprobe at `/opt/homebrew` or `/usr/local` | bundled `ffmpeg.exe` / `ffprobe.exe` next to the app; discovery prefers bundled, then PATH |
| Window + split panes + inspector resize | `NSWindow`/`NSSplitView` tuning | toolkit window + layout; replicate min size, fixed sidebar, inspector widen-on-open |
| Vibrancy `.ultraThinMaterial` | AppKit vibrancy | translucent fill / acrylic approximation (design forbids glow, so flat is faithful) |

Trait sketch (reference):

```rust
pub trait VolumeProvider {
    fn list_volumes(&self) -> Vec<VolumeInfo>;             // path, label, fs, removable, read-only
    fn free_space(&self, path: &Path) -> io::Result<FreeSpace>; // total/used/free
}
pub trait DeviceEvents {
    fn subscribe(&self) -> Receiver<DeviceEvent>;          // Attached / Detached / Renamed
}
pub trait UsbEnumerator { fn scan(&self) -> Vec<UsbDevice>; } // vid, pid, serial, product
pub trait Ejector { fn eject(&self, mount: &Path) -> Result<(), EjectError>; }
pub trait FileDialogs {
    fn pick_folder(&self, title: &str) -> Option<PathBuf>;
    fn save_file(&self, suggested: &str, filter: &[(&str,&str)]) -> Option<PathBuf>;
}
pub trait MediaControls { /* set now-playing, play/pause/next/prev callbacks */ }
```

## Design system (`ui/theme.rs`)

The theme is a direct transcription of `Theme.swift` tokens. Colors and fonts are defined as
constants; glass surfaces are reusable widgets.

**Colors** (sRGB):
- Surfaces: `obsidian/background #101114`, `card/surface #18191D`, `cardHover #212126`,
  `input #141519`, `border` = white @ 8%.
- Silver: `silver #DCE0E8`, `silverLight #F2F4F8`, `silverMuted #9BA0AC`, `silverDark #5F6470`.
- Gold: `hiResGold #E8B944`, `hiResGoldLight #F8D470`, `hiResGoldDark #C49426`; gold gradient
  top-leading→bottom-trailing.
- Gradients: `silverGradient` (white .90→.78), `storageGradient` (white .60→.85, L→R),
  `cardHeaderGradient` (white .24→.16). Rule: surfaces are flat (no background gradients); gradients
  are only used on specific controls (buttons, storage bars, sheens) as in the source.

**Typography:** a `Font` scale mirroring `WalkmanFont` (sizes/weights listed in requirements R2.3).
Bundle a system-like sans (Segoe UI is the Windows default and a close visual match to the macOS
system font used at these sizes) and a monospace face for metadata/timestamps and badges.

**Glass widgets** (reusable):
- `liquid_glass(cornerRadius=12)`: card @ 78% fill, 1px white border (opacity varies by
  hover/selected), neutral black shadow (radius 6 → 10 on hover). No glow.
- `studio_glass_card(cornerRadius=14)`: card @ 88% + 8px top specular sheen gradient.
- `modal_glass_chassis(cornerRadius=16)`: card @ 92% over a translucent/acrylic backing + 10px top
  sheen + strong shadow (radius 28). Used by all modal sheets.
- `button(variant)`: `Prominent | Gold | Glass | Capsule | Subtle | Destructive`, hover scale
  1.025, press 0.97, pointer cursor; destructive is red-tinted.
- `active_pill`, `hover_pill`, `hi_res_badge` ("HR" gold serif), `liquid_glass_icon_badge`,
  `storage_gauge`.

**Sizing baselines:** window min 1320×720 / default 1440×880; sidebar fixed 260pt; inspector 300pt
(+1px divider); floating player max width 1040; table bottom inset 88; control-height baseline 38pt
for toolbar controls/search/sort/primary buttons.

**Icon mapping:** a `const SIDEBAR_ICON: &[(NavTab, Icon)]` table maps each SF Symbol to a bundled
glyph (e.g. `hifispeaker.fill`→speaker, `music.note.list`→list-music, `doc.on.doc.fill`→copy, etc.).

## Application shell (`ui/shell.rs`)

Reproduces `ContentView` + `SidebarView` + `WalkmanStudioApp` window.

- **Window:** custom dark title bar, opaque `#101114` background, enforced min size, default size.
- **Layout:** three columns — fixed 260pt sidebar | flexible detail | optional 300pt inspector.
  Toggling the inspector widens the window by 300pt when screen space allows (replicating the
  macOS `handleInspectorVisibilityChanged` logic), otherwise overlays.
- **Sidebar:** the `NavTab` enum (13 variants, same titles/icons as R1.4) and the exact section
  grouping and visibility rules (Device section modes for 0 / 1 / >1 devices; Walkman Library only
  when connected; Backup & Restore disabled when disconnected). Count badges bound to app state.
- **Toolbar:** Refresh, Eject (spinner while ejecting), Inspector toggle (Ctrl+I). Menu commands:
  About (also bottom sidebar button), Manage Local Folders… (Ctrl+Shift+L / Ctrl+Shift+O).
- **Detail router:** a `match nav_tab { … }` that renders each section view, mirroring the
  `switch selectedTab` in `ContentView`. Each view receives callbacks for inspect/edit/trim/
  delete/transfer that mutate shared app state — same wiring as the Swift closures.
- **Now-playing bar:** always-present floating bar (max width 1040) bound to `player`.
- **Fullscreen overlay:** hosts the fullscreen Now Playing view.

### App state

```rust
struct AppState {
    drives: Vec<WalkmanDrive>,
    devices: Vec<WalkmanDevice>,
    active_device: Option<WalkmanDevice>,
    active_drive: Option<WalkmanDrive>,
    tracks: Vec<Track>,
    nav_tab: NavTab,                    // default: LocalLibrary
    selected_track_id: Option<String>,
    show_inspector: bool,
    queue_count: usize,
    device_info: Option<WalkmanDeviceInfo>,
    sheets: SheetState,                 // which modal sheet is open + its payload
    drive_cache: HashMap<String, DriveLibraryCache>,
    cached: PrecomputedCaches,          // albums/artists/recent/language/stats
    is_fullscreen_player: bool,
}
```

Services are shared via `Arc` (player, local_library, duplicate_finder, scanner, etc.), mirroring
the Swift singletons.

## Domain logic ports (`domain/`)

These are translated from Swift to Rust with identical behavior and are covered by unit tests.

- **models.rs** — `Track` (id = full_path, all fields incl. SenseMe), `AlbumItem`, `ArtistItem`,
  `WalkmanDrive`, `WalkmanDevice`, `WalkmanDeviceInfo`, `Playlist`, `TransferItem`, `TransferStatus`,
  `TransferTargetMode`, `DriveType`, `LocalMusicFolder`, `AudioFileMetadata`, plus derived
  properties (`is_hi_res` = >16bit or >48kHz, `is_language_study`, `learning_dir`, device totals).
- **artist_parse.rs** — `parse_primary_and_featured`, `is_balanced`, `clean_trailing_punctuation`,
  `strip_leading_feat_tokens`, `featured_display_string`, and the **known-exceptions list**
  (e.g. "Earth, Wind & Fire", "Tyler, the Creator", "AC/DC"). This is behavior-critical and must be
  ported verbatim with tests over the same inputs.
- **sanitize.rs** — `sanitize_walkman_text` (strip non-BMP scalars > U+FFFF, C0/C1 controls,
  variation selectors U+FE00..FE0F, `[Explicit]`/`(E)` markers) and `sanitize_filename` (strip
  `<>:"/\|?*`, NUL, trailing dots/spaces; max length; fallback "Unknown"); `target_track_url`
  (`Artist/Album/NN - Title.ext`, zero-padded track number).
- **senseme.rs** — the 12-Tone math: BPM/valence/arousal/mood-score extraction from mono PCM and
  classification into the 14 channels with the same thresholds and time-of-day guards. The SMFM
  3,270-byte payload layout is defined here; injection into FLAC/MP3 lives in `audio_tagger`.

## Service ports (`services/`)

- **walkman_engine.rs** — Baseline JPEG encode/validate (via `artwork.rs`), text/filename
  sanitization (delegates to `domain::sanitize`), `target_track_url`, playlist read/write/rename
  (`.m3u8`, `#EXTM3U`, `#EXTINF`, relative POSIX paths, CRLF), drive detection (via
  `platform::VolumeProvider` + `device_detector` signatures + free-space), custom-storage folder
  (plain stored path, no bookmark), junk-file cleanup (Windows + residual macOS artifacts), eject
  (via `platform::Ejector` with watchdog), transcode (delegates to `transcode.rs`), byte/duration
  formatters, companion `.lrc` writer (CRLF, UTF-8 no BOM, `[mm:ss.xx]`, offset handling).
- **device_detector.rs** — the hardcoded Sony model catalog, `is_supported_model`,
  `is_android_model`, `icon_key`, `inspect_volume` (capability XML regex parse, `DevIcon.fil`,
  MUSIC dir, Android package check, capacity-based WM1AM2 vs WM1ZM2), `detect_devices` (group
  internal+microsd into devices).
- **usb_detector.rs** — enumerate USB devices (SetupAPI/WMI), filter Sony VID/PID table, publish a
  list of `ConnectedUsbWalkman`; refreshed on device-change events.
- **audio_tagger.rs** — native FLAC writer (Vorbis comments + Picture block type 3 baseline JPEG,
  byte-level), native ID3v2.3 UTF-16+BOM writer (synchsafe sizes), M4A tagging via ffmpeg;
  multi-tier reader: native FLAC/MP3 → `symphonia`/`lofty` → ffprobe; cover extraction; lyrics
  extraction (FLAC Vorbis `LYRICS`/`UNSYNCEDLYRICS`/`SYNCEDLYRICS`, MP4 `©lyr`, MP3 `USLT`/`SYLT`/
  `ULT`); genre normalization; timestamp restore (preserve created/modified). Recommended crates:
  `lofty` or `symphonia` for reading; keep hand-written writers for the strict Sony-compliant
  FLAC/MP3 paths to match byte output exactly.
- **artwork.rs** — baseline RGB JPEG encode (`image` crate, resize ≤800px, quality ~0.92, force
  baseline, strip alpha/CMYK), validate (dimensions/color-model/progressive flag → compliance
  badges), export to PC (save dialog).
- **transcode.rs** — the preset→ffmpeg-args table ported verbatim (copy / copy_optimize / copy_raw,
  mp3_128/192/256/320/v0, aac_128/256/320, flac_16, flac_24_48, flac_24/flac_24_96, flac_24_192,
  alac_16, wav_16), optional `loudnorm` filter, `-vn` output; run bundled ffmpeg, capture stderr.
- **transfer_queue.rs** — `TransferItem` queue, format-support rules (distinct Sony-OS vs Android
  extension sets), enqueue from tracks/paths, direct transfer (copy + convert + artwork optimize +
  lyrics embed + sanitize path), single-pass concurrent prep, duplicate-guard integration, dual
  target (MUSIC / LEARNING with chapter playlist), progress reporting.
- **duplicate_finder.rs** / **duplicate_guard.rs** — same-storage grouping, exact vs cross-album
  scopes, ±4s duration clustering with normalized title/artist (preserving version tags), quality-
  diff detection, recoverable-space totals, auto-resolve; the guard checks the per-mount SQLite
  cache before transfer.
- **backup_restore.rs** — scopes (Full/Playlists/Learning/Albums/Tracks), destinations (local PC,
  external, custom), optional encryption (PBKDF2-HMAC-SHA256 100k iters, AES-256-GCM, 16-byte salt,
  streamed), manifest for unencrypted bundles, free-space verification, copy + flush, junk cleanup
  on restore, cancelation.
- **trim_cut.rs** — waveform generation (decode PCM via `symphonia`, downsample; ffmpeg fallback),
  processing via an ffmpeg filtergraph (trim/cut via `atrim`/concat, channel ops via
  `pan`/`channelmap`/phase invert, `afade`, `volume` gain), preserving tags/artwork afterward.
- **lyrics.rs** — LRCLIB (`/api/get`, `/api/search`, duration-matched, same user-agent) +
  Lyrics.ovh fallback, concurrent workers, synced/plain/skipped/not-found accounting.
- **online_metadata.rs** — iTunes Search API + Deezer search; returns candidates + cover art.
- **artist_art_cache.rs** / **cover_art_cache.rs** — Deezer artist search + embedded/device art,
  disk + memory cache keyed by path and `artist|album`.
- **library_scanner.rs** — `rusqlite` `tracks_cache` schema + WAL/pragmas + incremental migrations
  (SenseMe + date columns), recursive scan with mtime+size change detection, SenseMe backfill
  scheduler, per-mount cache load, cache mutators/invalidation.
- **local_library.rs** — managed local folders (persisted), rescan, published tracks + precomputed
  albums, update-track.
- **player.rs** — playback (`rodio`/MF), queue + advance hook, progress, now-playing via SMTC,
  media-key handling.

## UI view ports (`ui/views/`)

One module per section/sheet, each reproducing the exact controls/options listed in requirements
R4–R16. Enums are ported verbatim so every sort mode, filter, and display mode matches:

- `LocalDisplayMode` {Songs, Albums, Artists, Recently Added, Folders}; `LocalSortOption` {8 modes}.
- `AlbumSortOption` {7}; `FilterOption` {ALL + per-format, hi-res flag}.
- `RecentlyAddedTimeScope` {24h, 7d, 30d, All Time}.
- `HiResViewMode` {Tracks, Albums, Artists}; `HiResTrackSortOption` {6}.
- `LanguageStudyDisplayMode` {5}; `LanguageStudySortOption` {6}.
- `DuplicateSortOption` {4}; `DuplicateFilter`/`DuplicateModalAction`.
- Backup `Mode` {Backup, Restore}; `AlbumFilter`/`TrackFilter`.
- `EditorTab` {Tags & Specs, Album Artwork} (song + queue-item editors).
- Batch lyrics `TabContext` + `LyricsScope` {All Tracks in View, Selected, Active Item, Choose…}.

Shared widgets: expandable search bar, multi-column sortable table (with 88pt bottom inset),
cover-art grid, artist avatar, storage gauge, 3D cover-flow carousel (SensMe), waveform editor,
metric pills, duplicate-guard sheet, now-playing bar, fullscreen player.

Tables: reproduce `Table` with multi-selection, Shift-range / Ctrl-toggle, header select, and
per-selection context menus. In `egui`, use a virtualized table (e.g. `egui_extras::TableBuilder`)
for large libraries.

## Data flow examples

**Connect a Walkman:** `DeviceEvents` fires `Attached` → `usb_detector.scan()` + `walkman_engine.
detect_drives()` (via `VolumeProvider` + `device_detector.inspect_volume`) → build
`WalkmanDevice`/`WalkmanDrive` → update `AppState` → `library_scanner.scan(music_dir)` on a worker →
precompute albums/artists/recent/language/stats off-thread → push to `AppState` → UI shows sidebar
badges and Device Overview.

**Transfer with convert:** user enqueues tracks → `transfer_queue` prepares items (concurrent
metadata read, artwork optimize to baseline JPEG, optional lyrics fetch) → duplicate guard checks
per-mount cache → if collisions, show `DuplicateImportGuardSheet` (Skip/Overwrite/Keep Both) →
`transcode` runs bundled ffmpeg per preset → `audio_tagger` writes Sony-compliant tags → files laid
out under `/MUSIC/Artist/Album/NN - Title.ext` → progress via channel → refresh library.

## Error handling

- Services return `Result<T, AppError>` (a `thiserror` enum per domain: `TagError`, `TranscodeError`,
  `BackupError`, `DeviceError`, `IoError`, `NetworkError`). The UI surfaces failures with the same
  dialogs/alerts as macOS (e.g. "Audio File Not Found", transfer-format warning, delete confirms).
- External binaries (ffmpeg/ffprobe) missing → a clear, actionable error; bundled binaries make
  this rare.
- Network calls (`reqwest`) time out and degrade gracefully; lyrics/metadata failures never block.
- Eject/clean use a watchdog timeout so a hung drive never blocks the UI (ports the macOS 10s
  watchdog).
- All external content is validated (image decode guarded, lyrics/metadata sanitized) before use.

## Testing strategy

- **Unit tests (domain):** artist parsing (incl. known-exceptions), text/filename sanitization,
  `target_track_url` layout, `.lrc` formatting (CRLF/offset/synthesized timestamps), SenseMe channel
  classification thresholds, playlist CRLF/relative-path generation, preset→ffmpeg-args mapping.
- **Golden-byte tests (tagger):** write ID3v2.3 UTF-16+BOM and FLAC Vorbis/Picture blocks and the
  SMFM payload, then compare against fixtures captured from the macOS app's output to guarantee
  byte-for-byte hardware compliance.
- **Integration tests (services):** scan a fixture music tree into SQLite and assert albums/artists/
  hi-res/recent/language aggregation; round-trip read→write→read metadata; baseline-JPEG validator
  against progressive/CMYK/oversized fixtures; backup→restore round-trip with and without
  encryption (free-space + manifest checks).
- **Platform tests (Windows):** volume enumeration/free-space, USB VID/PID detection against a known
  device (manual/CI-gated), eject stub; file-dialog smoke tests.
- **UI/visual checks:** snapshot the theme widgets (pills, badges, cards, buttons, gauges) and
  compare against the macOS screenshots to confirm color/size parity; manual pass over each of the
  13 sections verifying every option/menu/sheet is present and labeled identically.
- **Manual device matrix:** validate against at least one Sony-OS DAP (e.g. NW-A55) and one Android
  Walkman (e.g. NW-A306 via the MTP/custom-folder bridge).
