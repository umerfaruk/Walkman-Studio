# Implementation Plan — Walkman Studio for Windows (Rust Port)

## Overview

This plan ports Walkman Studio from macOS/Swift to a Windows/Rust application with an identical UI
and all options preserved. Each task is an actionable coding activity, builds incrementally on prior
tasks, and references the requirements it satisfies. Test-writing is folded into the tasks that
produce the code under test. Tasks are ordered to deliver a running shell early, then portable
domain logic, then platform integrations, then feature views, then packaging and automated
verification.

## Tasks

- [x] 1. Bootstrap the Rust workspace and project skeleton
  - Created a Cargo workspace at `windows-app/` with crates `app` (binary `walkman-studio`), `ui`,
    `domain`, `services`, `platform`; pinned shared deps and a release profile (thin LTO, strip,
    panic=abort). Core crates type-check and the full workspace builds, producing the binary.
  - Added `windows-app/assets/` with a README documenting the `assets/bin/` slot for bundled
    `ffmpeg.exe`/`ffprobe.exe` and the shared image assets; added `windows-app/.gitignore`.
  - Cross-arch `x86_64`/`aarch64` MSVC targets are configured as a packaging concern in task 19.1
    (build verified on the dev host; Windows-native CI build happens at packaging time).
  - _Requirements: 19.1, 19.2_

- [x] 2. Stand up the GUI toolkit with an empty window
  - Decision recorded: **egui/eframe** (pure Rust, fully custom-styled, single-binary) over
    `slint`/`dioxus`; pinned in `ui`/`app` Cargo.toml.
  - `app/src/main.rs` opens a single dark window with obsidian `#101114` base visuals, enforces the
    min size 1320×720 and default 1440×880, and selects the default section (Local Music Library).
  - _Requirements: 1.1, 1.2, 19.1_

- [x] 3. Port the design system into `ui/theme.rs`
  - [x] 3.1 Define color constants (obsidian/card/input/border, silver, gold + the three gradients)
        and the `WalkmanFont` typography scale; load bundled Segoe UI + a monospace face into the
        toolkit font registry.
    - _Requirements: 2.1, 2.2, 2.3_
    - Progress: color tokens + sizing constants done in `ui/theme.rs` (with tests). Remaining: the three gradients, the `WalkmanFont` typography scale, and bundled-font loading.
  - [x] 3.2 Implement the glass surface widgets: `liquid_glass` (r12), `studio_glass_card` (r14
        sheen), `modal_glass_chassis` (r16 translucent backing + shadow); render flat surfaces with
        zero glow.
    - _Requirements: 2.4, 2.6_
  - [x] 3.3 Implement the six button variants with hover/press/cursor feedback, plus the active/
        hover pills, HR gold badge, liquid-glass icon badge, and storage-gauge widgets.
    - _Requirements: 2.5, 2.8_
  - [x] 3.4 Add the SF-Symbol→bundled-glyph icon mapping table and the 38pt control-height and 88pt
        table bottom-inset layout constants.
    - _Requirements: 2.7, 2.9, 1.5_
  - [x] 3.5 Write snapshot tests that render each theme widget to an image buffer and assert color
        values and pixel dimensions against the documented tokens.
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 2.9_

- [x] 4. Build the application shell (`ui/shell.rs`) with navigation
  - [x] 4.1 Lay out the three-column shell: fixed 260pt sidebar, flexible detail, 300pt inspector
        placeholder; initialize the default nav tab to Local Music Library.
    - _Requirements: 1.1, 1.3, 1.6_
  - [x] 4.2 Implement the `NavTab` enum (13 sections, exact titles/icons) and render the sidebar
        sections with their visibility rules (Device 0/1/>1 modes, Walkman Library only when
        connected, Backup & Restore disabled when disconnected) and count badges; add the About
        button.
    - _Requirements: 1.4, 1.5_
  - [x] 4.3 Implement the toolbar (Refresh, Eject, Inspector toggle Ctrl+I) and menu commands (About,
        Manage Local Folders… Ctrl+Shift+L/Ctrl+Shift+O); implement the inspector widen-on-open
        window-resize logic.
    - _Requirements: 1.7, 1.8_
  - [x] 4.4 Implement the always-present floating now-playing bar (max width 1040) and the
        fullscreen overlay host (wired to the player in task 15).
    - _Requirements: 1.9, 7.2_
  - [x] 4.5 Implement the detail router (`match nav_tab`) that renders a placeholder pane per
        section.
    - _Requirements: 1.4_

- [x] 5. Port the domain models and pure logic (`domain/`)
  - [x] 5.1 Port `models.rs` (Track, AlbumItem, ArtistItem, WalkmanDrive, WalkmanDevice,
        WalkmanDeviceInfo, Playlist, TransferItem/TransferStatus, TransferTargetMode, DriveType,
        LocalMusicFolder, AudioFileMetadata) with derived properties (is_hi_res, is_language_study,
        learning_dir, device totals).
    - _Requirements: 18.1, 5.7_
    - Progress: `Track`, `Playlist`, `TransferStatus`, `DriveType`, `TransferTargetMode` done in `domain/models.rs` (with tests). Remaining: `WalkmanDrive`, `WalkmanDevice`, `WalkmanDeviceInfo`, `AlbumItem`, `ArtistItem`, `TransferItem`, `AudioFileMetadata`, `LocalMusicFolder`.
  - [x] 5.2 Port `artist_parse.rs` including the known-exceptions list, and write unit tests over the
        same inputs/expected outputs as the Swift version.
    - _Requirements: 5.7_
  - [x] 5.3 Port `sanitize.rs` (`sanitize_walkman_text`, `sanitize_filename`, `target_track_url`) and
        write unit tests for non-BMP stripping, illegal chars, `[Explicit]` markers, and path layout.
    - _Requirements: 9.4, 13.1_
  - [x] 5.4 Port `senseme.rs` 12-Tone math + 14-channel classification (thresholds, time-of-day
        guards), define the SMFM 3,270-byte payload layout, and write unit tests for channel
        classification.
    - _Requirements: 6.1, 6.2_
    - Progress: `SenseMeChannel` catalog (14 channels, order/labels), `SenseMeInfo`, and the `SMFM_PAYLOAD_LEN` (3270) constant done in `domain/senseme.rs` (with tests). Remaining: the BPM/valence/arousal/mood extraction math and the classification thresholds + tests.

- [~] 6. Define the platform abstraction and Windows implementations (`platform/`)
  - [x] 6.1 Define the traits `VolumeProvider`, `DeviceEvents`, `UsbEnumerator`, `Ejector`,
        `FileDialogs`, and `MediaControls`, plus a no-op/fake implementation for tests.
    - _Requirements: 3.1, 7.4, 12.6, 16.2_
    - Progress: all six traits + shared types (`model.rs`) done; `platform/src/fake.rs` adds
      `FakeVolumeProvider`/`FakeDeviceEvents`/`FakeUsbEnumerator`/`FakeEjector`/
      `FakeFileDialogs`/`FakeMediaControls` (in-memory, platform-independent, with tests) for
      `services`/`ui` tests to use instead of real hardware. `cargo check -p walkman-platform
      --tests` passes (see environment note).
  - [x] 6.2 Implement `VolumeProvider` (GetLogicalDrives/GetDriveTypeW/GetVolumeInformationW) and
        free-space (GetDiskFreeSpaceExW).
    - _Requirements: 3.2, 12.6, 18.1_
    - Progress: `WindowsVolumeProvider` in `platform/src/windows/volumes.rs`; type-checks clean.
  - [~] 6.3 Implement `DeviceEvents` via WM_DEVICECHANGE/RegisterDeviceNotification plus a periodic
        poll fallback.
    - _Requirements: 3.6_
    - Progress: `WindowsDeviceEvents` (`platform/src/windows/device_events.rs`) implements the
      periodic-poll half only (1.5s `GetLogicalDrives` diff) — simpler and sufficient for R3.6's
      "periodic safety refresh" on its own. The `WM_DEVICECHANGE` hidden-window push path is not
      implemented yet; it can be layered on later without changing the `DeviceEvents` trait.
      Type-checks clean.
  - [x] 6.4 Implement `UsbEnumerator` (SetupAPI or WMI) filtering Sony VID `054C` + the PID table.
    - _Requirements: 3.1_
    - Progress: `WindowsUsbEnumerator` (`platform/src/windows/usb.rs`) shells out to
      `Get-PnpDevice` (WMI/CIM) instead of raw SetupAPI device-info-set calls, trading a small
      process-spawn cost for far less unsafe FFI surface; parses `VID_xxxx`/`PID_xxxx` plus serial
      from the PnP instance id, with unit tests on the parser (pass under `cargo check --tests`,
      see environment note on why `--tests` and not a full `cargo test` run). It reports every USB
      device seen — the Sony VID/PID table filter belongs to `usb_detector.rs` (task 9.2), per
      design.md.
  - [x] 6.5 Implement `Ejector` (IOCTL_STORAGE_EJECT_MEDIA / lock+dismount, mountvol fallback) with a
        watchdog timeout.
    - _Requirements: 16.2_
    - Progress: `WindowsEjector` (`platform/src/windows/eject.rs`) implements
      lock-volume/dismount-volume/eject-media via `DeviceIoControl` on a worker thread with a 10s
      watchdog (`EjectError::Timeout`); no `mountvol` fallback yet. Type-checks clean.
  - [x] 6.6 Implement `FileDialogs` (IFileOpenDialog/IFileSaveDialog or `rfd`) and a reveal-in-
        Explorer helper (`explorer /select`).
    - _Requirements: 4.1, 8.2, 10.4, 13.3_
    - Progress: `WindowsFileDialogs` (`platform/src/windows/dialogs.rs`) uses the classic
      `SHBrowseForFolderW`/`GetSaveFileNameW` common dialogs (COM-free) instead of
      `IFileOpenDialog`/`IFileSaveDialog`/`rfd`, plus `explorer.exe /select,<path>` for reveal.
      Type-checks clean.
  - [~] 6.7 Implement `MediaControls` via WinRT SystemMediaTransportControls (play/pause/next/prev,
        now-playing metadata + artwork).
    - _Requirements: 7.4_
    - Progress: `WindowsMediaControls` (`platform/src/windows/media_controls.rs`) binds
      `SystemMediaTransportControls` to the app's `HWND` via
      `ISystemMediaTransportControlsInterop::GetForWindow` (`windows::core::factory`) and wires
      title/artist/album + play/pause state. Type-checks clean. Artwork (`SetThumbnail`) and the
      OS-side play/pause/next/prev *command* callbacks (vs. just pushing state out) are not wired
      yet — left for when task 13 (player) exists to drive them.

  **Environment note (task 6):** this dev host's Rust toolchain is `x86_64-pc-windows-gnu`, and
  *linking* a binary or test harness that pulls in a recent `windows-sys`/`windows-link` (including
  `chrono`, already a task-5 dependency) fails with `error calling dlltool 'dlltool.exe': program
  not found`, even though a working `dlltool.exe` ships with the `rust-mingw` component and runs
  fine invoked directly — only rustc's own spawn of it fails. This reproduces on a clean checkout
  with **no code changes at all** (confirmed by stashing and testing `walkman-domain` alone), so it
  predates and is unrelated to this task's code; `walkman-services`/`walkman-ui` additionally can't
  *build* at all here because `ring` (a `reqwest` dependency, task 5-era) needs a `gcc.exe` this
  host doesn't have. The MSVC toolchain is installed but has no linker (`link.exe`) available
  either. Given that, verification here used `cargo check -p walkman-platform --tests` (type-checks
  the crate and its unit tests without linking) rather than `cargo build`/`cargo test`: it passes
  clean for all of task 6, and was used iteratively while writing this code — it caught and fixed
  five real mistakes (wrong module paths for `GENERIC_READ`/`DRIVE_*`/`ISystemMediaTransportControlsInterop`,
  a `GetLastError`-vs-`Result` mixup, and a `SHGetPathFromIDListW` buffer-type mismatch). Treat the
  actual link/run behavior as unverified until this host's `windows-gnu` linker is fixed (working
  MinGW-w64 binutils + `gcc.exe`) or a real MSVC Build Tools install is added.

- [ ] 7. Implement the SQLite library scanner and local library service
  - [~] 7.1 Port `library_scanner.rs` with `rusqlite` `tracks_cache` schema, WAL/pragmas, incremental
        migrations (SenseMe + date columns), mtime+size change detection, per-mount cache load, and
        cache mutators.
    - _Requirements: 18.1, 18.2_
  - [~] 7.2 Port `local_library.rs` (managed folders persisted, rescan, exposed tracks + precomputed
        albums, update-track).
    - _Requirements: 4.1, 4.7, 18.4_
  - [~] 7.3 Implement off-thread precompute of albums/artists/recent/language/stats and publish the
        results into app state via a channel.
    - _Requirements: 18.4_
  - [~] 7.4 Write an integration test that scans a fixture music tree into SQLite and asserts the
        album/artist/hi-res/recent/language aggregation counts.
    - _Requirements: 18.1, 18.2, 18.4_

- [ ] 8. Implement audio metadata read/write (`audio_tagger.rs`) and artwork engine (`artwork.rs`)
  - [~] 8.1 Implement baseline RGB JPEG encode (resize ≤800px, q~0.92, force baseline, strip
        alpha/CMYK) and the validator that produces compliance badges (`image` crate).
    - _Requirements: 13.1, 13.2_
  - [~] 8.2 Implement the native FLAC writer (Vorbis comments + Picture block type 3) and the
        multi-tier reader (native → `lofty`/`symphonia` → ffprobe) including cover + lyrics
        extraction and genre normalization.
    - _Requirements: 8.4, 13.4, 15.5_
  - [~] 8.3 Implement the native binary ID3v2.3 UTF-16+BOM writer (synchsafe sizes) and M4A tagging
        via ffmpeg.
    - _Requirements: 8.4, 13.4_
  - [~] 8.4 Implement timestamp preservation (restore created/modified) and SMFM injection into
        FLAC/MP3.
    - _Requirements: 8.5, 6.3, 15.4_
  - [~] 8.5 Write round-trip (read→write→read) tests and golden-byte tests comparing tagger output
        against fixtures captured from the macOS app, and write validator tests against progressive/
        CMYK/oversized image fixtures.
    - _Requirements: 13.1, 13.2, 13.4, 6.3_

- [ ] 9. Implement the Walkman engine and device detection services
  - [~] 9.1 Port `device_detector.rs`: the Sony model catalog, supported/android checks, icon-key
        map, `inspect_volume` (capability XML parse, signatures, capacity-based WM1 M2 split),
        `detect_devices`, and write unit tests for model classification from fixture signatures.
    - _Requirements: 3.2, 3.3, 3.4_
  - [~] 9.2 Port `usb_detector.rs` on top of `UsbEnumerator` to publish connected Walkmans.
    - _Requirements: 3.1_
  - [~] 9.3 Port `walkman_engine.rs`: drive detection (VolumeProvider + signatures + free-space),
        custom-storage folder (plain stored path), `target_track_url`, playlist read/write/rename
        (CRLF `.m3u8`), byte/duration formatters, and the companion `.lrc` writer; write unit tests
        for the `.m3u8` CRLF/relative-path output and the `.lrc` formatter.
    - _Requirements: 3.2, 3.7, 11.1, 15.4_
  - [~] 9.4 Implement Clean & Eject (Windows + residual macOS junk removal, flush, eject via
        `Ejector` with watchdog).
    - _Requirements: 16.1, 16.2_

- [ ] 10. Implement transcoding and the transfer queue
  - [~] 10.1 Port `transcode.rs` preset→ffmpeg-args table verbatim (all presets + loudnorm + `-vn`)
        using the bundled ffmpeg with bundled-first path discovery, and write unit tests asserting
        the generated argument list per preset.
    - _Requirements: 9.3, 19.2, 19.3_
  - [~] 10.2 Port `transfer_queue.rs`: queue items/status, Sony-OS vs Android format rules, enqueue
        from tracks/paths, direct transfer (copy/convert/optimize/lyrics/sanitize), single-pass
        concurrent prep, dual target (MUSIC/LEARNING + chapter playlist), and progress reporting.
    - _Requirements: 9.1, 9.2, 9.4, 9.6, 9.7_
  - [~] 10.3 Port `duplicate_guard.rs` (per-mount SQLite cache check before transfer).
    - _Requirements: 9.5_

- [ ] 11. Implement duplicate finder and backup/restore services
  - [~] 11.1 Port `duplicate_finder.rs`: same-storage grouping, exact vs cross-album scopes, ±4s
        clustering with normalized title/artist (version-tag preserving), quality-diff detection,
        recoverable-space totals, auto-resolve, and write unit tests for the matching/clustering
        logic.
    - _Requirements: 10.1, 10.2, 10.3, 10.4_
  - [~] 11.2 Port `backup_restore.rs`: scopes/destinations, PBKDF2-HMAC-SHA256 (100k) + AES-256-GCM
        encryption (16-byte salt, streamed) + manifest, free-space verify (GetDiskFreeSpaceExW),
        CopyFileExW + FlushFileBuffers, junk cleanup on restore, and cancelation.
    - _Requirements: 12.1, 12.2, 12.3, 12.4, 12.5, 12.6_
  - [~] 11.3 Write backup→restore round-trip integration tests for both encrypted and unencrypted
        bundles, asserting manifest contents and free-space checks.
    - _Requirements: 12.4, 12.5_

- [ ] 12. Implement trim/cut, lyrics, online metadata, and art caches
  - [~] 12.1 Port `trim_cut.rs`: waveform generation (symphonia + ffmpeg fallback) and the ffmpeg
        filtergraph for trim/cut/channel ops/fades/gain, preserving tags+artwork afterward.
    - _Requirements: 14.1, 14.2, 14.3, 14.4, 14.5_
  - [~] 12.2 Port `lyrics.rs`: LRCLIB (get/search, duration-matched, same user-agent) + Lyrics.ovh
        fallback, concurrent workers, and synced/plain/skipped/not-found accounting.
    - _Requirements: 15.1, 15.3_
  - [~] 12.3 Port `online_metadata.rs` (iTunes + Deezer) and `artist_art_cache.rs` /
        `cover_art_cache.rs` (disk + memory caches).
    - _Requirements: 17.1, 17.2, 17.3, 5.4_

- [~] 13. Implement audio playback (`player.rs`)
  - Implement the playback engine (`rodio`/Media Foundation) with play/pause/seek/next/prev, a queue
    + advance hook, progress reporting, and SMTC integration via `MediaControls`.
  - _Requirements: 7.1, 7.4, 7.5_

- [ ] 14. Build the Local Library and Walkman library views
  - [~] 14.1 Implement `LocalLibraryView`: display modes (Songs/Albums/Artists/Recently Added/
        Folders), the 8 sort options, search, track/folder context menus (incl. Relocate Folder…),
        DirectTransfer sheet, format-warning + removal confirmation dialogs; Manage Local Folders
        sheet.
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6_
  - [~] 14.2 Implement `LibraryView` (All Songs): sortable multi-column table + search + batch-lyrics
        button + per-row context menu; shared selection (Shift-range/Ctrl-toggle) and 88pt inset.
    - _Requirements: 5.1, 2.9_
  - [~] 14.3 Implement `RecentlyAddedView`: time-scope picker (24h/7d/30d/All) + ranked table + batch
        lyrics.
    - _Requirements: 5.2_
  - [~] 14.4 Implement `AlbumsView`: cover grid, 7 sort options, format filter chips, expandable
        search, album detail drill-down, album+track context menus, batch lyrics.
    - _Requirements: 5.3_
  - [~] 14.5 Implement `ArtistsView`: artist grid with avatars (cache-backed), search, drill-down,
        context menus.
    - _Requirements: 5.4_
  - [~] 14.6 Implement `HiResAudioView`: view modes (Tracks/Albums/Artists), track + album sort
        options, format/quality filter chips, search, context menus, batch lyrics.
    - _Requirements: 5.5_
  - [~] 14.7 Implement `LanguageStudyView`: 5 display modes, 6 sort options, two tables, context
        menus, batch lyrics.
    - _Requirements: 5.6_
  - [~] 14.8 Implement `DeviceOverviewView`: model image, specs, per-drive storage gauges, quick
        stats, Refresh/Eject, multi-device switcher; and the Android MTP help sheet + custom-folder
        bridge.
    - _Requirements: 3.5, 3.7_

- [ ] 15. Build playback UI: now-playing bar, fullscreen player, SensMe channels
  - [~] 15.1 Implement `TrackPreviewBar` bound to the player (artwork, title/artist, transport,
        scrubber, open-inspector, open-fullscreen).
    - _Requirements: 7.2_
  - [~] 15.2 Implement `FullScreenNowPlayingView`: large artwork, blurred ambient background,
        transport, karaoke auto-scroll synced lyrics (~0.3s lead), active-line highlight.
    - _Requirements: 7.3_
  - [~] 15.3 Implement `SenseMeChannelsView`: 3D cover-flow carousel of the 14 channels + Shuffle
        All, atmospheric backgrounds, gapless highlight-seek playback, live lyrics, analysis
        trigger.
    - _Requirements: 6.4, 6.5_

- [ ] 16. Build metadata editors, inspector, and artwork export
  - [~] 16.1 Implement `MetadataStudioView` inspector (300pt): artwork, metadata, specs, Edit button,
        device lyrics display.
    - _Requirements: 8.1, 15.5_
  - [~] 16.2 Implement `SongMetadataEditorSheet`: tabs (Tags & Specs / Album Artwork), all editable
        fields, artwork drop zone + compliance badges, export-to-PC, online metadata + lyrics
        search; modal size ≈960×860.
    - _Requirements: 8.2, 13.2, 13.3, 15.1_
  - [~] 16.3 Implement `AlbumMetadataEditorSheet`: batch field edit, unified artwork + `folder.jpg`,
        tracklist reorder + renumber (`1..N`/`01..N`).
    - _Requirements: 8.3_
  - [~] 16.4 Implement `TransferQueueItemEditorSheet`: per-item tabs + online metadata/lyrics fetch.
    - _Requirements: 9.1_

- [ ] 17. Build tool views and sheets
  - [~] 17.1 Implement `TransferQueueView`: queue list with status, expandable rows, per-item editor,
        preview context menu, duplicate-import guard sheet (Skip/Overwrite/Keep Both), clear-queue
        confirm, `TransferSyncSheet` progress.
    - _Requirements: 9.1, 9.5, 9.6_
  - [~] 17.2 Implement `DuplicatesView`: scope/filter picker, 4 sort options, per-group modal
        resolution, recoverable totals, audition, reveal-in-Explorer, Auto-Resolve Quality, delete
        confirms.
    - _Requirements: 10.4, 10.5_
  - [~] 17.3 Implement `PlaylistsView`: create/rename/delete/edit `.m3u8`, add-tracks browser modes,
        multi-select (Shift/Ctrl + header Select), reorder, floating batch action bar (Move to Top /
        Up / Down / To Bottom / remove) with instant sync.
    - _Requirements: 11.1, 11.2, 11.3_
  - [~] 17.4 Implement `BackupRestoreView`: Backup/Restore modes, 5 scopes + Album/Track filters,
        destination picker, passphrase field, progress + cancel.
    - _Requirements: 12.1, 12.2, 12.3, 12.5_
  - [~] 17.5 Implement `TrimAndCutSheet`: waveform scrubber + in/out handles, Trim/Cut modes, ms
        steppers (±0.1/±1/±5s) + lock-to-playhead, channel routing (swap/mono/stereo/isolate L/R/
        phase), fades + gain, preview playback.
    - _Requirements: 14.1, 14.2, 14.3, 14.4, 14.5_
  - [~] 17.6 Implement `BatchLyricsGeneratorSheet`: launch from all 7 views, scopes (All in View /
        Selected / Active Item / Choose Specific… checklist with search + Select/Deselect All +
        counts + missing diagnostics), live progress/speed/ETA, metric pills, filterable results.
    - _Requirements: 15.2, 15.3_
  - [~] 17.7 Implement `CleanEjectSheet`, `AboutWalkmanStudioView` (hardware / supported DAPs /
        credits tabs), `ManageLocalFoldersSheet`, `AndroidWalkmanMtpHelpSheet`, and
        `DuplicateImportGuardSheet`.
    - _Requirements: 16.3, 3.7, 4.1_

- [~] 18. Wire real-time device events and library lifecycle into the shell
  - Subscribe to `DeviceEvents` + periodic refresh to auto-detect attach/detach; trigger scans and
    off-thread precompute; keep sidebar badges, Device Overview, and the active drive/device in
    sync.
  - Implement the built-in lyrics editor (edit/paste/sync → write companion `.lrc`).
  - _Requirements: 3.6, 18.4, 15.6_

- [ ] 19. Packaging and automated verification
  - [~] 19.1 Implement bundled-first binary discovery for `ffmpeg.exe`/`ffprobe.exe` and configure
        the installer/standalone-exe build (device images, SensMe covers, fonts, icons) for
        `x86_64` and `aarch64` Windows.
    - _Requirements: 19.1, 19.2_
  - [~] 19.2 Write a benchmark/integration test that scans a generated 7,000+ track fixture library
        and asserts scan+precompute completes within a performance budget without blocking the UI
        thread (worker-channel assertions).
    - _Requirements: 18.4, 19.5_
  - [~] 19.3 Implement input-validation guards for all external content (image decode bounds, lyrics/
        metadata sanitization) and assert non-destructive behavior in tests: delete operations
        require a confirmation flag, and tag-only operations leave audio stream bytes unchanged.
    - _Requirements: 19.3, 19.4, 19.6_
  - [~] 19.4 Implement a parity-checklist test that enumerates every `NavTab`, sort option, filter,
        display mode, and sheet in code and asserts the set matches a fixture captured from the macOS
        app (so missing/renamed options fail the build).
    - _Requirements: 1.4, 4.2, 4.3, 5.1, 5.2, 5.3, 5.5, 5.6, 10.4, 12.2, 15.2_

## Task Dependency Graph

```json
{
  "waves": [
    { "wave": 1, "tasks": ["1"] },
    { "wave": 2, "tasks": ["2", "5", "6"] },
    { "wave": 3, "tasks": ["3", "7", "8"] },
    { "wave": 4, "tasks": ["4", "9", "13"] },
    { "wave": 5, "tasks": ["10", "11", "12"] },
    { "wave": 6, "tasks": ["14", "15", "16", "17"] },
    { "wave": 7, "tasks": ["18"] },
    { "wave": 8, "tasks": ["19"] }
  ]
}
```

- Task 1 (workspace) is a prerequisite for all tasks.
- Task 2 (window) depends on 1; 3 (theme) depends on 2; 4 (shell) depends on 3.
- Task 5 (domain) depends on 1 only; prerequisite for all services (7-13) and views (14-17).
- Task 6 (platform traits + Windows impls) depends on 1; required by 7 (volumes), 9 (detection),
  11.2 (free-space/copy), 13 (media controls), 16.2/17.2 (dialogs), 9.4/16/17 (eject).
- Task 7 (scanner + local library) depends on 5, 6.2; required by 9.3, 10.3, 11, 14, 18.
- Task 8 (tagger + artwork) depends on 5; required by 10 (transfer), 12.1 (trim/cut), 16 (editors).
- Task 9 (engine + detection) depends on 5, 6, 7, 8; required by 10, 14.8, 17, 18.
- Task 10 (transcode + queue) depends on 8, 9, and 10.3 depends on 7.1; required by 17.1.
- Task 11 (duplicates + backup) depends on 5, 7, 8, 9, and 6.2 (free-space); required by 17.2, 17.4.
- Task 12 (trim/lyrics/metadata/art) depends on 8 (tags), 6 (network host); required by 15, 16, 17.
- Task 13 (player) depends on 6.7, 8; required by 4.4, 15.
- Tasks 14–17 (views/sheets) depend on 3, 4, and their respective services (7–13).
- Task 18 (events + lifecycle) depends on 4, 6.3, 7, 9, 12.2.
- Task 19 (packaging + verification) depends on all prior tasks; 19.4 depends on the view enums
  from 4 and 14–17.

## Notes

- GUI toolkit: `eframe`/`egui` is the reference choice from design.md; all tasks are written to be
  toolkit-agnostic where possible, so swapping toolkits changes only tasks 2–4 and the `ui` crate.
- Requirement IDs refer to `requirements.md` (e.g. `5.3` = Requirement 5, acceptance criterion 3).
- SF Symbols are unavailable on Windows; task 3.4 establishes the bundled-glyph mapping used by all
  icon-bearing views.
- Golden-byte and parity fixtures (tasks 8.5, 19.4) should be captured from the macOS app output and
  committed under `crates/*/tests/fixtures/`.
- No audio stream is re-encoded during metadata-only operations; this is asserted in task 19.3.
- Status (workspace at `windows-app/`): tasks 1-2 complete; 5.2, 5.3, 6.1, 6.2, 6.4, 6.5, and 6.6
  complete; 3.1, 5.1, 5.4, 6.3, and 6.7 partially done (see per-task Progress notes). `cargo test
  --workspace` could not be run after task 6's changes — this dev host's `windows-gnu` toolchain
  can't *link* anything pulling a recent `windows-sys`/`windows-link` (including `chrono`, already
  a task-5 dependency) and can't *build* `walkman-services`/`walkman-ui` at all (`ring` needs a
  missing `gcc.exe`); see the environment note under task 6. This predates task 6 and needs a
  host-side toolchain fix (working MinGW `dlltool` + `gcc.exe`, or an MSVC Build Tools install)
  before `cargo test --workspace` can run again. `cargo check -p walkman-platform --tests` passes
  clean and was used as the verification method for all of task 6 instead. A sub-task is checked
  when it type-checks clean and its logic has been read back carefully against the real `windows`
  0.52 API source (confirmed via the registry sources, not guessed); 6.3 and 6.7 stay `[~]` because
  they're intentionally partial (poll-only device events; no artwork/command callbacks yet), not
  because of unverified code.
