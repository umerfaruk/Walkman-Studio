# Requirements — Walkman Studio for Windows (Rust Port)

## Introduction

Walkman Studio is a native macOS (Swift/SwiftUI) desktop application for managing music
libraries on Sony Walkman digital audio players (both classic Sony-OS / Linux DAPs and modern
Android Walkmans), plus a standalone local music library and an interactive SensMe™ mood
channels player.

This spec defines a **feature-complete port to Windows written in Rust**. The explicit goals:

- **Same OS target reversed:** macOS → Windows 10/11 (x86_64 and arm64).
- **Same language reversed:** Swift → Rust.
- **Identical UI:** the layout, navigation, views, controls, options, colors, fonts, sizing,
  and the "Optical Liquid Glass & Obsidian" design system must be reproduced **same to same**.
  Every option, sort mode, filter chip, menu item, toggle, and sheet present in the macOS app
  must exist in the Windows app with the same labels and behavior.
- **Same behavior:** all hardware-compliance rules (Baseline JPEG, ID3v2.3 UTF-16, FLAC
  `SMFM` block, CRLF `.m3u8`, filesystem sanitization, SenseMe 12-Tone) are preserved exactly.
- **Platform integrations are re-implemented** with Windows equivalents (USB detection, volume
  mount events, drive eject, file dialogs, media transport controls, crypto, image codecs).

The port replaces macOS-only steps that have no Windows analog (AppleDouble `._*` dotfiles and
`dot_clean`, TCC/sandbox permission prompts, security-scoped bookmarks) with the correct Windows
behavior. macOS ghost-file cleanup becomes cleanup of Windows-equivalent junk
(`Thumbs.db`, `desktop.ini`, `$RECYCLE.BIN`, `System Volume Information`) plus any residual
`._*`/`.DS_Store` artifacts that may already exist on the device from prior macOS use.

### Scope boundaries

- In scope: a standalone Windows desktop app replicating all 13 navigation sections and all
  tools/sheets of the macOS app.
- Out of scope: iOS/Android companion apps, cloud sync, any feature not present in the source app.

### Terminology

- **DAP** — Digital Audio Player (a Walkman).
- **Sony-OS / classic** — non-Android Walkman running Sony's RTOS, mounts as USB Mass Storage.
- **Android Walkman** — modern Walkman exposing storage via MTP (not a mass-storage drive letter).
- **Target toolkit** — the chosen Rust GUI toolkit (decided in design.md).

---

## Requirements

### Requirement 1 — Application shell, window, and navigation

**User Story:** As a Walkman owner on Windows, I want the same single-window app with the same
sidebar and sections, so the experience is identical to the macOS version.

#### Acceptance Criteria

1. WHEN the app launches THEN the system SHALL present a single main window with a fixed
   left sidebar, a central detail pane, and an optional trailing inspector pane.
2. The window SHALL enforce a minimum size of 1320×720 and a default size of 1440×880, matching
   the macOS constraints.
3. The sidebar SHALL be a fixed-width 260pt source list that is non-collapsible.
4. The sidebar SHALL contain the following sections and rows exactly:
   - **Device** (or **Connected Devices (n)** when more than one device is attached): connected
     device card(s) plus one row per drive (Internal, MicroSD); a "disconnected" row when no
     Walkman is present.
   - **Local Music Library:** `Local Library` (track-count badge), `SenseMe™ Channels`.
   - **Walkman Library** (only shown when a Walkman is connected): `All Songs`, `Recently Added`,
     `Albums`, `Artists`, `Hi-Res Audio`, `Language Study`, `Playlists` — each with count badges.
   - **Tools:** `Duplicates` (count badge), `Transfer & Convert` (queue-count badge),
     `Backup & Restore` (disabled/greyed with tooltip when no Walkman is connected).
   - A bottom `About Walkman Studio` button.
5. Each sidebar row SHALL display the same title and an equivalent icon to its macOS SF Symbol.
6. The default selected section on launch SHALL be `Local Music Library`.
7. The app SHALL provide menu/command equivalents: `About Walkman Studio`, and
   `Manage Local Folders…` (keyboard shortcuts Ctrl+Shift+L and Ctrl+Shift+O).
8. A toolbar SHALL provide `Refresh`, `Eject` (clean + unmount), and an `Inspector` toggle
   (Ctrl+I). Toggling the inspector SHALL widen the window by the inspector width when space allows,
   matching macOS behavior.
9. A floating bottom now-playing bar (max width 1040) SHALL be present across all sections.

### Requirement 2 — Visual design system parity ("Optical Liquid Glass & Obsidian")

**User Story:** As a user, I want the Windows app to look pixel-identical to the macOS app, so it
feels like the same product.

#### Acceptance Criteria

1. The app SHALL use the obsidian surface palette: background/obsidian `#101114`, card/surface
   `#18191D`, cardHover `#212126`, input `#141519`, border = white at 8% opacity.
2. The app SHALL use the silver palette (`#DCE0E8`, `#F2F4F8`, `#9BA0AC`, `#5F6470`) and the
   Hi-Res gold palette (`#E8B944`, `#F8D470`, `#C49426`) including the gold top-leading→
   bottom-trailing gradient.
3. Typography SHALL match the WalkmanFont scale: title1 22 bold, title2 18 bold, title3 15
   semibold, headline 14 semibold, body 13, bodyMedium 13 medium, bodyBold 13 semibold, callout 12,
   calloutMedium/Bold 12, subheadline 11.5, caption 11, sectionHeader 11 bold, mono 11.5 monospace,
   monoSmall 10.5 monospace, badge 9.5 bold monospace, pillCounter 10 bold rounded.
4. The design system SHALL reproduce the three glass surfaces: liquid-glass control (radius 12),
   studio glass card (radius 14 with a top specular sheen), and modal glass chassis (radius 16 with
   a translucent/acrylic backing and a strong drop shadow).
5. Buttons SHALL offer the six variants: prominent (silver/platinum), gold, glass, capsule, subtle,
   and destructive (red), with the same hover (scale ~1.025) and press (~0.97) feedback and a
   pointer cursor on hover.
6. The design SHALL enforce **zero background gradients on surfaces** (flat obsidian) and **zero
   glow/bloom**; depth comes only from neutral black shadows and crisp 1px borders.
7. Interactive toolbar controls, search bars, sort menus, and primary buttons SHALL share the 38pt
   control-height baseline.
8. A Hi-Res "HR" gold serif badge, active/hover capsule pills, storage gauges, and liquid-glass
   icon badges SHALL be reproduced.
9. Table views SHALL reserve an 88pt bottom inset so rows are not hidden behind the floating player.

### Requirement 3 — Device detection, identification, and overview

**User Story:** As a user, I want the app to auto-detect my connected Walkman and show its model,
OS family, and storage, exactly as on macOS.

#### Acceptance Criteria

1. The system SHALL enumerate connected USB devices and identify Sony Walkmans by USB vendor id
   `0x054c` and the known product-id table (A105 `0x0d00/0x0cff/0x0d02`, ZX507 `0x0d01/0x0d03`,
   A306 `0x0e6e/0x0e72`, ZX707 `0x0e6f/0x0e73`, WM1AM2 `0x0e70/0x0e74`, WM1ZM2 `0x0e71/0x0e75`),
   with product-string fallback heuristics.
2. The system SHALL inspect mounted volumes for authentic Sony signatures (`default-capability.xml`
   / `DEFAULT-CAPABILITY.XML`, `DevIcon.fil`, `Sony_Card.IND`, `MUSIC`/`Music` directory, Android
   package path `Android/data/com.sony.walkman*`) and classify each drive as Internal or MicroSD.
3. The system SHALL identify the specific model, series, OS family (Sony OS vs Android Walkman OS),
   firmware, display resolution, internal capacity, and supported audio formats, using the same
   hardcoded Sony catalog as the macOS app (NW-A10/A20/A30/A40/A50, NW-ZX100/ZX300, NW-WM1A/WM1Z,
   NW-WS/E/S/B series, and Android NW-A100/A300, NW-ZX500/ZX700, NW-WM1AM2/WM1ZM2).
4. WHEN an unsupported model is detected THEN the system SHALL record it and surface it in the UI
   the same way the macOS app does.
5. The Device Overview dashboard SHALL show the device model image, specs, per-drive storage gauges
   (Internal + MicroSD), quick stats (albums/artists/hi-res), and provide `Refresh`, `Eject`, and a
   multi-device switcher.
6. The system SHALL detect volume attach/detach events in real time (Windows device-change
   notifications) and refresh the device list automatically, with a periodic safety refresh.
7. WHEN an Android Walkman that exposes storage only via MTP is connected THEN the app SHALL show an
   MTP help/limitation sheet and SHALL allow the user to select a custom storage folder as the
   device root (the MTP/custom-folder bridge).

### Requirement 4 — Local Music Library (Mac-side → PC-side)

**User Story:** As a user, I want to manage my local Windows music collection independent of any
connected device, with the same views and options.

#### Acceptance Criteria

1. The user SHALL be able to import one or more local folders from any Windows drive, with
   background indexing, via a `Manage Local Folders…` sheet (add/remove/rescan).
2. The Local Library SHALL offer the display modes: `Songs`, `Albums`, `Artists`,
   `Recently Added`, `Folders`.
3. The Local Library SHALL offer the sort options: Title A–Z, Title Z–A, Artist A–Z, Album A–Z,
   Date Added (Newest), Date Added (Oldest), Duration (Longest), Audio Quality/Bitrate.
4. The Local Library SHALL provide a search bar, per-track context menu, and a folder context menu
   including `Relocate Folder…`.
5. The system SHALL provide a direct-transfer flow (`DirectTransferSheet`) to send selected tracks,
   albums, or genres to a connected Walkman drive, including the format-warning alert offering
   "Transfer & Convert (All)".
6. The system SHALL show confirmation dialogs for "Remove Folder from Local Library?" and
   "Remove Missing Folders and Tracks?", and an "Audio File Not Found" alert, matching macOS.
7. Folder contents SHALL be indexed into a persistent local cache (SQLite) using modified-time +
   size change detection, with incremental re-scans.

### Requirement 5 — Walkman library browsing views

**User Story:** As a user, I want to browse a connected Walkman's library through the same set of
views and options.

#### Acceptance Criteria

1. **All Songs** SHALL present a multi-column sortable track table with search, a batch-lyrics
   button, and a per-row context menu (play, edit metadata, trim/cut, transfer, delete).
2. **Recently Added** SHALL present ranked recently-added tracks with a time-scope picker:
   `Past 24 Hours`, `Past 7 Days`, `Past 30 Days`, `All Time`, plus a batch-lyrics button.
3. **Albums** SHALL present a cover-art grid with: sort options (Recently Added, Album Name A–Z,
   Album Name Z–A, Artist A–Z, Release Year Newest, Release Year Oldest, Track Count Most); format
   filter chips (ALL + per-format with a Hi-Res flag); an expandable search bar; album detail
   drill-down; album and track context menus; and a batch-lyrics button.
4. **Artists** SHALL present an artist grid with circular avatars, search, album/track drill-down,
   and context menus. Artist avatars SHALL be sourced from device art, embedded art, or a Deezer
   artist-image lookup, with disk + memory caching.
5. **Hi-Res Audio** SHALL filter to tracks >16-bit or >48kHz and provide view modes `Tracks`,
   `Albums`, `Artists`; track sort options (Default, Title, Artist, Album, Highest Quality, Recently
   Added); album sort options; format/quality filter chips; search; context menus; batch lyrics.
6. **Language Study** SHALL present tracks from `/LEARNING/` or language/audiobook/podcast/learning
   genres with display modes `Tracks`, `Albums`, `Artists`, `Recently Added`, `Folders`; sort
   options (Title A–Z, Title Z–A, Speaker/Artist A–Z, Course/Album A–Z, Duration Longest First,
   Recently Added First); context menus; batch lyrics.
7. All browsing views SHALL respect the same artist-name parsing (primary vs featured artist,
   including the known-exceptions list such as "Earth, Wind & Fire", "Tyler, the Creator", "AC/DC").
8. Right-click context menus across all views SHALL offer the same actions, including launching the
   Song Metadata Editor, Album Metadata Editor, and Trim & Cut editor.

### Requirement 6 — SensMe™ 12-Tone analysis and Mood Channels player

**User Story:** As a user, I want the SensMe mood-channel player and 12-Tone analysis to work
identically on Windows, including on-device tag injection.

#### Acceptance Criteria

1. The system SHALL perform on-board 12-Tone acoustic analysis (BPM, valence, arousal, mood scores)
   by decoding audio to mono PCM, using the same algorithm and classification thresholds as macOS.
2. The system SHALL classify tracks into the 14 official SensMe channels: the mood channels
   (Energetic, Relax, Upbeat, Mellow, Emotional, Lounge, Dance, Extreme) and the time-of-day
   channels (Morning, Daytime, Evening, Night, Midnight), plus a global "Shuffle All Songs".
3. The system SHALL inject 12-Tone metadata into files: FLAC `APPLICATION` block code `2` with
   application id `SMFM` (exact 3,270-byte payload) and MP3 ID3v2.3 proprietary frames
   (`GEOB` / `TXXX:SONY_12TONE_ANALYSIS`), preserving audio frames.
4. The SensMe Channels view SHALL present an interactive 3D carousel/cover-flow of the channels with
   atmospheric background artwork, gapless channel playback, and a highlight-seek playback mode.
5. The player SHALL show synchronized live lyrics during channel playback.

### Requirement 7 — Audio playback, now-playing bar, and fullscreen player

**User Story:** As a user, I want in-app playback with the same transport controls and fullscreen
experience.

#### Acceptance Criteria

1. The system SHALL play local and device audio files (FLAC, MP3, M4A/ALAC, WAV, AAC) with play/
   pause, next/previous, seek, and a progress scrubber.
2. The floating bottom now-playing bar SHALL show artwork, title/artist, transport controls, a
   scrubber, an open-inspector button, and an open-fullscreen button.
3. The fullscreen Now Playing view SHALL present large artwork, a dynamic blurred ambient
   background, transport controls, and karaoke-style auto-scrolling synchronized lyrics with a
   ~0.3s vocal lead-time, highlighting the active line.
4. The system SHALL integrate with Windows System Media Transport Controls (media keys, OS now-
   playing surface), replacing the macOS MPNowPlayingInfoCenter integration.
5. Playback SHALL support a queue with an advance hook used by the SensMe highlight mode.

### Requirement 8 — Metadata editing (song, album, inspector, artwork)

**User Story:** As a user, I want the same metadata editors and the trailing inspector.

#### Acceptance Criteria

1. The trailing inspector (`MetadataStudioView`, 300pt) SHALL show the selected track's artwork,
   metadata, and audio specs, with an `Edit` button that opens the Song Metadata Editor, and it
   SHALL display any lyrics stored on the device.
2. The Song Metadata Editor SHALL provide tabs `Tags & Specs` and `Album Artwork`; editable fields
   Title, Artist, Album, Track Number, Disc Number, Year, Genre, and Lyrics; a drag-and-drop
   artwork drop zone with Baseline-JPEG validation badges; an artwork export-to-PC action; an
   integrated online metadata lookup; and integrated LRCLIB/Lyrics.ovh lyrics search. Modal size
   SHALL match (≈ min width 960, max height 860).
3. The Album Metadata Editor SHALL batch-edit album title, artist, year, and genre across all
   tracks, embed unified Baseline-JPEG artwork to every track plus a companion `folder.jpg`, and
   support interactive tracklist reordering with automatic renumbering (`1..N` or `01..N`).
4. Metadata read SHALL use a multi-tier reader (native FLAC/MP3 parsing, then a decoder-based
   reader, then ffprobe) and metadata write SHALL use native FLAC (Vorbis + Picture block type 3)
   and native binary ID3v2.3 UTF-16 writers, with ffmpeg used for M4A tagging.
5. Saving metadata SHALL preserve original file creation/modification timestamps so "Recently
   Added" smart views do not re-order unexpectedly.

### Requirement 9 — Transfer & Convert queue (FFmpeg)

**User Story:** As a user, I want the same batch transcoding/transfer queue with the same presets,
targets, and duplicate guard.

#### Acceptance Criteria

1. The system SHALL provide a drag-and-drop batch queue of `TransferItem`s with per-item status
   (pending, optimizing, converting, success, skipped, failed), expandable rows, a per-item editor
   sheet (with online metadata + lyrics fetch), and a per-row preview context menu.
2. The queue SHALL support dual-target routing: `Music Library` (`/MUSIC/` or `/Music/`) and
   `Language Study & Audiobooks` (`/LEARNING/`), with automatic chapter playlist generation for the
   learning target.
3. The system SHALL provide the transcoding presets, at minimum: Bit-Perfect Copy (tag sanitize,
   no re-encode), FLAC 16-bit/44.1kHz, FLAC 24-bit/96kHz, and MP3 320kbps (UTF-16 tags), plus the
   other presets the engine recognizes (FLAC 24/48, FLAC 24/192, MP3 128/192/256/V0, AAC
   128/256/320, ALAC 16, WAV 16), including an optional loudness-normalization flag.
4. The system SHALL sanitize filesystem paths (strip `< > : " / \ | ? *`, trailing dots/spaces,
   non-BMP characters/emoji, and `[Explicit]` markers) and lay files out as
   `/MUSIC/Artist/Album/NN - Title.ext`.
5. Before transfer the system SHALL detect tracks already present on the target volume and present
   a duplicate-import guard sheet with `Skip Existing`, `Overwrite`, and `Keep Both`.
6. The system SHALL show real-time transfer progress (`TransferSyncSheet`) and enforce format
   support rules distinctly for Sony-OS vs Android Walkman devices.
7. The system SHALL support 1-click direct transfers from the Local Library with a single-pass
   concurrent metadata-preparation stage.

### Requirement 10 — Duplicate detection and same-storage scoping

**User Story:** As a user, I want the same-storage duplicate cleaner with the same scopes and
resolution options.

#### Acceptance Criteria

1. Duplicate comparison SHALL be scoped strictly within the same storage partition (Internal,
   MicroSD, or Local Library) to prevent cross-drive deletion.
2. The system SHALL provide the matching scopes `Exact Same Album (Safe)` and
   `Include Cross-Album (Loose)`, using normalized title/artist matching (preserving distinct
   versions: Acoustic/Live/Remix/Instrumental) with ±4.0s duration clustering.
3. The system SHALL detect quality discrepancies ("Quality Diff") between duplicate copies.
4. The results view SHALL provide sort options (Space Recoverable, Song Title A–Z, Artist A–Z, Most
   Copies), a scope/filter picker, per-group resolution actions, recoverable-space totals, in-app
   audition, reveal-in-Explorer, and an `Auto-Resolve Quality` one-click action that keeps the
   highest-fidelity copy and removes lower-quality copies plus companion `.lrc` and junk files.
5. Deletions SHALL require native confirmation dialogs.

### Requirement 11 — Hardware-compliant playlists (CRLF `.m3u8`)

**User Story:** As a user, I want the same playlist editor that writes Sony-compliant playlists.

#### Acceptance Criteria

1. The system SHALL create, rename, delete, and edit playlists stored in `/MUSIC/` or `/Music/` as
   `.m3u8` files using relative POSIX paths (`Artist/Album/Track.ext`) and strict CRLF (`\r\n`)
   line endings, with an `#EXTM3U` header and `#EXTINF` entries.
2. The playlist editor SHALL support adding tracks via a browser with modes (Songs, Recently Added,
   and the other add-browsing modes), multi-track selection with Shift-range and Ctrl-toggle plus a
   header "Select" button, and per-row context menus.
3. The system SHALL provide a floating batch action bar with `Move to Top`, group Up/Down shift,
   `To Bottom`, and batch removal, syncing the `.m3u8` instantly after each change.

### Requirement 12 — Backup & Restore (encrypted)

**User Story:** As a user, I want the same backup/restore studio with the same scopes and optional
encryption.

#### Acceptance Criteria

1. The system SHALL support modes `Backup Walkman` and `Restore to Walkman`.
2. Backups SHALL support 5 scopes: Full Library, Playlists Scope, Language Study (`/LEARNING`),
   Selected Albums, and Selected Tracks (with `All Albums`/`Selected` and `All Tracks`/`Selected`
   filters).
3. Backup destinations SHALL include Local PC storage (default under the user's Music folder, e.g.
   `%USERPROFILE%\Music\Walkman Studio Backups`), connected external USB/SD drives, and custom
   folders.
4. Optional encryption SHALL use PBKDF2-HMAC-SHA256 (100,000 iterations) key derivation and
   AES-256-GCM authenticated encryption, producing a `*.walkmanbackup.enc` package with a 16-byte
   random salt; unencrypted backups SHALL produce a bundle with a manifest.
5. Restore SHALL verify target free space (net bytes required), reconstruct the directory
   hierarchy, run junk-file cleanup, and show progress; backup/restore SHALL be cancelable.
6. Free-space checks SHALL use the Windows free-space API; fast copies SHALL use the Windows copy
   API with buffer flushing.

### Requirement 13 — Baseline JPEG artwork engine and ID3v2.3 / FLAC compliance

**User Story:** As a user, I want cover art and tags written exactly as Sony firmware requires.

#### Acceptance Criteria

1. The system SHALL convert cover art to Baseline (non-progressive) RGB JPEG at ≤ 800×800,
   stripping progressive scans, alpha, and CMYK, at ~0.92 quality.
2. The system SHALL validate artwork and show compliance badges (`800×800 • Walkman Ready`,
   `Exceeds 800px`, `Progressive JPEG (Advisory)`, color-model/encoding diagnostics).
3. The system SHALL extract embedded/companion artwork and export it to the PC via a Windows save
   dialog.
4. MP3 tagging SHALL write binary ID3v2.3 with UTF-16 LE + BOM (`encoding=1`); FLAC SHALL write
   standard Vorbis comments and a Front Cover Picture block (`type=3`, `image/jpeg`); audio frames
   SHALL never be re-encoded during tagging.

### Requirement 14 — Trim & Cut audio studio

**User Story:** As a user, I want the same waveform trim/cut editor with channel routing and fades.

#### Acceptance Criteria

1. The Trim & Cut sheet SHALL render a waveform with draggable in/out handles and gapless preview
   playback.
2. It SHALL offer `Trim (Keep Selection)` and `Cut (Remove Selection)` modes.
3. It SHALL provide millisecond steppers (±0.1s, ±1s, ±5s) and lock-to-playhead boundary controls.
4. It SHALL provide channel routing: invert L/R (swap), downmix to Mono, force 2-channel Stereo,
   isolate Left or Right to both ears, and 180° phase flip.
5. It SHALL provide fade-in/fade-out curves and −6dB to +6dB gain, processing via FFmpeg while
   preserving ID3v2.3/Vorbis tags and Baseline-JPEG artwork.

### Requirement 15 — Lyrics engine (synchronized `.lrc`)

**User Story:** As a user, I want the same lyrics fetching, viewing, and editing capabilities.

#### Acceptance Criteria

1. The system SHALL fetch synchronized `.lrc` and plain lyrics from LRCLIB (`/api/get`,
   `/api/search`, matched by title/artist/album/millisecond duration) with a fallback to
   Lyrics.ovh, using the same user-agent string and concurrent multi-worker behavior.
2. The batch lyrics generator SHALL be launchable from All Songs, Recently Added, Albums, Artists,
   Hi-Res Audio, Playlists, and Language Study, with scopes `All Tracks in View`, `Selected
   Track(s)`, `Active Item`, and `Choose Specific…` (checklist modal with live search, Select/
   Deselect All, live counts, and missing-lyrics diagnostics).
3. The batch generator SHALL show a live progress bar, speed, ETA, and interactive metric pills
   (`Synced .LRC`, `Plain Text`, `Skipped`, `Not Found`) with a filterable results breakdown.
4. Generated `.lrc` files SHALL use CRLF, UTF-8 without BOM, strict `[mm:ss.xx]` timestamps,
   chronological order, synthesized timestamps when only plain text is available, and `[offset:…]`
   handling; they SHALL be written adjacent to the audio file. Original file timestamps SHALL be
   preserved.
5. The device lyrics viewer SHALL read companion `.lrc`/`.txt` (auto-decoding UTF-8, Windows-1252,
   ISO-8859-1, UTF-16), embedded FLAC Vorbis comments (`LYRICS`/`UNSYNCEDLYRICS`/`SYNCEDLYRICS`),
   MP4/M4A `©lyr` atoms, and MP3 `USLT`/`SYLT`/`ULT` frames.
6. A built-in lyrics editor SHALL allow editing/pasting/syncing lyrics and saving as a companion
   `.lrc`.

### Requirement 16 — Clean & Eject (Windows junk purge + safe removal)

**User Story:** As a user, I want safe ejection with junk-file cleanup appropriate for Windows.

#### Acceptance Criteria

1. The Clean & Eject flow SHALL remove device junk before unmounting: residual macOS artifacts
   (`._*`, `.DS_Store`, `.Spotlight-V100`, `.Trashes`, `.fseventsd`) if present, and Windows
   artifacts (`Thumbs.db`, `desktop.ini`, `$RECYCLE.BIN`, `System Volume Information` contents where
   permitted).
2. The flow SHALL flush writes and then safely eject/unmount each drive using the Windows volume
   eject mechanism, with a watchdog so a hung operation never blocks the UI.
3. The flow SHALL present a progress/confirmation sheet equivalent to the macOS `CleanEjectSheet`.

### Requirement 17 — Online metadata and artist imagery

**User Story:** As a user, I want online metadata and artist images, same as macOS.

#### Acceptance Criteria

1. The system SHALL fetch track metadata and cover art from the iTunes Search API and Deezer.
2. The system SHALL fetch artist images from Deezer's artist search, cached to disk and memory.
3. All network calls SHALL be resilient to failure and SHALL never block the UI.

### Requirement 18 — Persistence, caching, and performance

**User Story:** As a user, I want fast library loading of large collections, same as macOS.

#### Acceptance Criteria

1. The system SHALL cache scanned track metadata in a SQLite database with a `tracks_cache` table
   (including SenseMe and date columns) and incremental schema migrations.
2. Scans SHALL detect changes via modified-time + size and only re-parse changed files; per-mount
   cached tracks SHALL be loadable for the duplicate guard and quick reloads.
3. Cover-art and artist-art caches SHALL use both in-memory and on-disk layers.
4. Library precompute (albums/artists/recent/language/stats) SHALL run off the UI thread so the
   interface stays responsive while indexing thousands of tracks.

### Requirement 19 — Non-functional requirements

#### Acceptance Criteria

1. The app SHALL target Windows 10 (1809+) and Windows 11 on x86_64 and arm64.
2. The app SHALL be written in Rust and packaged as a standalone Windows installer/executable,
   bundling `ffmpeg.exe` and `ffprobe.exe` (with path discovery that prefers the bundled binaries).
3. The app SHALL never re-encode audio during metadata-only operations.
4. The app SHALL perform all file operations non-destructively, requiring explicit confirmation for
   deletions, matching the source app's safety posture.
5. The app SHALL remain responsive (no UI freezes) during scanning, transcoding, backup, and
   network operations.
6. All externally fetched content (lyrics, metadata, images) SHALL be treated as untrusted input
   and validated before use.
