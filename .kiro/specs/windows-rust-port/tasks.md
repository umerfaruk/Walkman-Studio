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

- [x] 6. Define the platform abstraction and Windows implementations (`platform/`)
  - [x] 6.1 Define the traits `VolumeProvider`, `DeviceEvents`, `UsbEnumerator`, `Ejector`,
        `FileDialogs`, and `MediaControls`, plus a no-op/fake implementation for tests.
    - _Requirements: 3.1, 7.4, 12.6, 16.2_
    - Progress: all six traits + shared types (`model.rs`) done; `platform/src/fake.rs` adds
      `FakeVolumeProvider`/`FakeDeviceEvents`/`FakeUsbEnumerator`/`FakeEjector`/
      `FakeFileDialogs`/`FakeMediaControls` (in-memory, platform-independent, with tests) for
      `services`/`ui` tests to use instead of real hardware.
  - [x] 6.2 Implement `VolumeProvider` (GetLogicalDrives/GetDriveTypeW/GetVolumeInformationW) and
        free-space (GetDiskFreeSpaceExW).
    - _Requirements: 3.2, 12.6, 18.1_
    - Progress: `WindowsVolumeProvider` in `platform/src/windows/volumes.rs`.
  - [~] 6.3 Implement `DeviceEvents` via WM_DEVICECHANGE/RegisterDeviceNotification plus a periodic
        poll fallback.
    - _Requirements: 3.6_
    - Progress: `WindowsDeviceEvents` (`platform/src/windows/device_events.rs`) implements the
      periodic-poll half only (1.5s `GetLogicalDrives` diff) — simpler and sufficient for R3.6's
      "periodic safety refresh" on its own. The `WM_DEVICECHANGE` hidden-window push path is not
      implemented yet; it can be layered on later without changing the `DeviceEvents` trait.
      Intentionally partial, not unverified — kept `[~]` for that reason.
  - [x] 6.4 Implement `UsbEnumerator` (SetupAPI or WMI) filtering Sony VID `054C` + the PID table.
    - _Requirements: 3.1_
    - Progress: `WindowsUsbEnumerator` (`platform/src/windows/usb.rs`) shells out to
      `Get-PnpDevice` (WMI/CIM) instead of raw SetupAPI device-info-set calls, trading a small
      process-spawn cost for far less unsafe FFI surface; parses `VID_xxxx`/`PID_xxxx` plus serial
      from the PnP instance id, with unit tests on the parser. It reports every USB device seen —
      the Sony VID/PID table filter belongs to `usb_detector.rs` (task 9.2), per design.md.
  - [x] 6.5 Implement `Ejector` (IOCTL_STORAGE_EJECT_MEDIA / lock+dismount, mountvol fallback) with a
        watchdog timeout.
    - _Requirements: 16.2_
    - Progress: `WindowsEjector` (`platform/src/windows/eject.rs`) implements
      lock-volume/dismount-volume/eject-media via `DeviceIoControl` on a worker thread with a 10s
      watchdog (`EjectError::Timeout`); no `mountvol` fallback yet.
  - [x] 6.6 Implement `FileDialogs` (IFileOpenDialog/IFileSaveDialog or `rfd`) and a reveal-in-
        Explorer helper (`explorer /select`).
    - _Requirements: 4.1, 8.2, 10.4, 13.3_
    - Progress: `WindowsFileDialogs` (`platform/src/windows/dialogs.rs`) uses the classic
      `SHBrowseForFolderW`/`GetSaveFileNameW` common dialogs (COM-free) instead of
      `IFileOpenDialog`/`IFileSaveDialog`/`rfd`, plus `explorer.exe /select,<path>` for reveal.
  - [~] 6.7 Implement `MediaControls` via WinRT SystemMediaTransportControls (play/pause/next/prev,
        now-playing metadata + artwork).
    - _Requirements: 7.4_
    - Progress: `WindowsMediaControls` (`platform/src/windows/media_controls.rs`) binds
      `SystemMediaTransportControls` to the app's `HWND` via
      `ISystemMediaTransportControlsInterop::GetForWindow` (`windows::core::factory`) and wires
      title/artist/album + play/pause state. Artwork (`SetThumbnail`) and the OS-side
      play/pause/next/prev *command* callbacks (vs. just pushing state out) are not wired yet —
      left for when task 13 (player) exists to drive them. Intentionally partial.

  **Verified 2026-11-07:** the dev host now has Visual Studio Build Tools (MSVC) installed and
  `rustup default stable-x86_64-pc-windows-msvc`. `cargo test -p walkman-platform` builds, links,
  and passes all 9 tests (the earlier `windows-gnu` dlltool/raw-dylib issue and the missing-`gcc`
  issue below were both toolchain gaps, not code problems — switching to MSVC sidesteps both).
  `cargo test --workspace` passes in full (92+16 `ui` tests, `domain`, `platform`, all green);
  `walkman-services` also now builds clean (`rusqlite`'s bundled SQLite C source and `reqwest`'s
  `ring` both need a real C compiler, which MSVC provides).

- [x] 7. Implement the SQLite library scanner and local library service
  - [x] 7.1 Port `library_scanner.rs` with `rusqlite` `tracks_cache` schema, WAL/pragmas, incremental
        migrations (SenseMe + date columns), mtime+size change detection, per-mount cache load, and
        cache mutators.
    - _Requirements: 18.1, 18.2_
    - Progress: `services/src/library_scanner.rs`. `LibraryScanner` owns a `Mutex<Connection>`
      (bundled `rusqlite`, WAL + `synchronous=NORMAL`), a versioned `migrate()` that creates the
      base `tracks_cache` schema at v1, `ALTER TABLE`s in the SenseMe/date/mtime columns at v2
      (tested against a hand-built v1 DB to confirm existing rows survive), and a `local_folders`
      table at v3 (used by 7.2). `scan_tree` walks a root with `walkdir`, skips non-audio
      extensions, compares cached `(size_bytes, mtime_secs)` to skip unchanged files, and reads
      tags for new/changed files via `lofty` (not the native `audio_tagger` readers — task 8 is
      not a dependency of task 7, per design.md's wave graph). `upsert_track`/`remove_mount`/
      `tracks_for_mount`/`all_tracks` cover the mutator/per-mount-load surface. 4 unit tests.
  - [x] 7.2 Port `local_library.rs` (managed folders persisted, rescan, exposed tracks + precomputed
        albums, update-track).
    - _Requirements: 4.1, 4.7, 18.4_
    - Progress: `services/src/local_library.rs`. `LocalLibrary` wraps an `Arc<LibraryScanner>`;
      `add_folder`/`remove_folder`/`relocate_folder`/`rescan_folder` manage `LocalMusicFolder` rows
      (persisted via the scanner's `local_folders` table) and their per-folder `tracks_cache`
      rows; `rescan_folder` sets `is_missing` instead of erroring when the folder's path is gone,
      matching the macOS "Remove Missing Folders and Tracks?" flow (R4.6) rather than failing the
      scan. `tracks()` aggregates across every registered folder; `update_track` lets a metadata
      editor commit a change without a full rescan. 6 unit tests.
  - [x] 7.3 Implement off-thread precompute of albums/artists/recent/language/stats and publish the
        results into app state via a channel.
    - _Requirements: 18.4_
    - Progress: `precompute(Vec<Track>) -> PrecomputedCaches` (pure function) groups albums/artists
      (by lowercased name+artist / artist), sorts `recent` newest-first, filters
      `is_language_study()` tracks, and computes `LibraryStats` (track/album/artist/hi-res/
      language counts + total bytes). `LocalLibrary::rescan_all_async` spawns a thread that
      rescans every folder and sends the result over an `mpsc::Receiver<Result<PrecomputedCaches>>`
      — the off-thread + channel mechanism R18.4 asks for. It does not yet push into an `AppState`
      because no `AppState` exists yet; that wiring is task 18's job (which explicitly depends on
      task 7), same deferral pattern as task 6.7 deferring to task 13. 3 unit tests (2 for
      `precompute`, 1 confirming `rescan_all_async` delivers without blocking/panicking).
  - [x] 7.4 Write an integration test that scans a fixture music tree into SQLite and asserts the
        album/artist/hi-res/recent/language aggregation counts.
    - _Requirements: 18.1, 18.2, 18.4_
    - Progress: `services/tests/library_scan_integration.rs`. Hand-builds real, spec-valid
      (frameless) FLAC files — a `STREAMINFO` block with real `sample_rate`/`bits_per_sample` plus
      a `VORBIS_COMMENT` block with TITLE/ARTIST/ALBUM/DATE — across a small fixture tree (2
      artists/3 albums/1 hi-res track/1 `/LEARNING/` track/1 non-audio file), scans it end to end
      through `LocalLibrary::add_folder` → `tracks()` → `precompute`, and asserts the exact
      album/artist/hi-res/language counts. This exercises the real `lofty` tag-read path, not just
      `precompute` over hand-built `Track` values.

  **Verified:** all of task 7 builds, links, and passes under the MSVC toolchain —
  `cargo test -p walkman-services` (15 unit + 1 integration test, all green) and
  `cargo clippy -p walkman-services --tests` (clean). `cargo test --workspace` passes in full
  (180 tests across all five crates).

- [x] 8. Implement audio metadata read/write (`audio_tagger.rs`) and artwork engine (`artwork.rs`)
  - [x] 8.1 Implement baseline RGB JPEG encode (resize ≤800px, q~0.92, force baseline, strip
        alpha/CMYK) and the validator that produces compliance badges (`image` crate).
    - _Requirements: 13.1, 13.2_
    - Progress: `services/src/artwork.rs`. `encode_baseline_jpeg(_from_image)` decodes any
      `image`-supported input, downscales (never upscales) to fit 800px on the longest edge via
      Lanczos3, converts to RGB8 (stripping alpha/CMYK), and encodes through `image`'s
      `JpegEncoder` — which has no progressive-encoding support at all, so every output is
      baseline by construction. `validate_jpeg` hand-parses the raw JPEG marker stream up to the
      first `SOFn` (stopping before the entropy-coded scan data, so no byte-stuffing/restart-
      marker handling is needed) to read real width/height/component-count/baseline-vs-progressive
      independently of what `image`'s decoder would silently normalize away, and
      `ArtworkCompliance::badges()` produces the `"WxH • Walkman Ready"` /
      `"Exceeds 800px"` / `"Progressive JPEG (Advisory)"` / CMYK-advisory strings. 6 tests,
      including one against hand-built progressive+CMYK+oversized JPEG marker bytes.
  - [x] 8.2 Implement the native FLAC writer (Vorbis comments + Picture block type 3) and the
        multi-tier reader (native → `lofty`/`symphonia` → ffprobe) including cover + lyrics
        extraction and genre normalization.
    - _Requirements: 8.4, 13.4, 15.5_
    - Progress: `services/src/flac_tags.rs` hand-parses/rewrites the FLAC metadata-block chain
      directly (not through a tag library): `STREAMINFO` and every other block type is preserved
      byte-for-byte in its original position, only `VORBIS_COMMENT` (rewritten in place, keeping
      any fields this app doesn't touch) and `PICTURE` (replaced wholesale, type 3/front-cover)
      change, and the audio stream after the last metadata block is copied through untouched.
      Reads `SYNCEDLYRICS`/`LYRICS`/`UNSYNCEDLYRICS` Vorbis fields for lyrics (R15.5) and routes
      `GENRE` through `services/src/genre.rs`'s ID3v1-index normalizer (task 8.2's "genre
      normalization"). The reader side of task 8's "multi-tier" design lives in
      `audio_tagger.rs`'s `read_metadata` (see 8.3's note) rather than here. 5 tests, including a
      write→read round trip and a "write twice replaces, doesn't duplicate, the picture block"
      regression test.
  - [x] 8.3 Implement the native binary ID3v2.3 UTF-16+BOM writer (synchsafe sizes) and M4A tagging
        via ffmpeg.
    - _Requirements: 8.4, 13.4_
    - Progress: `services/src/id3v2.rs` hand-builds a binary ID3v2.3 tag: every text/`USLT` frame
      is `encoding=1` (UTF-16LE + BOM per R13.4), the *tag* header size is synchsafe (the only
      field ID3v2.3 synchsafes — each frame's own size is a plain big-endian `u32`, unlike
      ID3v2.4, which this writer gets right), and `APIC` embeds the front-cover JPEG
      (`picture type = 3`). Replaces any pre-existing ID3v2 tag wholesale and copies the rest of
      the file (audio frames, any trailing ID3v1 tag) through unchanged. A general reader
      (`read_tags`) decodes Latin-1/UTF-16(BOM)/UTF-16BE/UTF-8 text frames for robustness against
      arbitrary existing files, even though this writer only ever produces UTF-16+BOM. `audio_tagger.rs`
      dispatches FLAC → `flac_tags`, MP3 → `id3v2`, and M4A → `ffmpeg_util::find_ffmpeg` +
      `build_m4a_ffmpeg_args`/`write_m4a_tags_via_ffmpeg` (remux with `-c copy`, metadata flags,
      `-disposition:v attached_pic` for cover art — never re-encodes the audio stream). `ffmpeg`
      isn't bundled yet (task 19.1), so the M4A path is unit-tested only at the
      argument-construction level (matching task 10.1's own planned test style), not against a
      real `ffmpeg.exe` run. 5 tests for `id3v2`, 6 for `audio_tagger` (incl. the FLAC dispatch
      round trip through the real filesystem).
  - [x] 8.4 Implement timestamp preservation (restore created/modified) and SMFM injection into
        FLAC/MP3.
    - _Requirements: 8.5, 6.3, 15.4_
    - Progress: `platform/src/windows/fs_time.rs` adds `set_file_times` (`CreateFileW` +
      `SetFileTime`, a plain utility function rather than a fake-able trait — it has real-
      filesystem-only behavior with nothing to swap in tests); `audio_tagger::write_tags` reads
      the original creation/modified times before rewriting a file and restores them afterward
      (R8.5). `services/src/smfm.rs` implements the real, spec-correct container mechanics for
      injecting the SenseMe payload — a FLAC `APPLICATION` block (replacing one on re-injection,
      not duplicating it) and an ID3v2.3 `GEOB` frame (`id3v2::inject_geob_frame`, reused by
      nothing else) — and builds the payload's `GBPM`/`STSA`/`STMO` chunks correctly from real
      BPM/highlight/mood-score inputs. The `STBF`/`STHF`/`STMM` chunks are device-template bytes
      the design doc says come from a real macOS-app-captured fixture that isn't in this repo;
      `build_payload` zero-fills them rather than guessing proprietary byte content, documented
      in the module doc as the one piece still blocked on that fixture landing. 5 tests
      (`fs_time`'s + 4 in `smfm`).
  - [x] 8.5 Write round-trip (read→write→read) tests and golden-byte tests comparing tagger output
        against fixtures captured from the macOS app, and write validator tests against progressive/
        CMYK/oversized image fixtures.
    - _Requirements: 13.1, 13.2, 13.4, 6.3_
    - Progress: round-trip tests exist for all three writers (FLAC, ID3v2.3, SMFM-into-FLAC) and
      the artwork validator (hand-built progressive/CMYK/oversized marker bytes) — see each
      sub-task's notes above. **Byte-for-byte golden comparison against real macOS-app output is
      not done**: no such fixtures exist in this repo (same gap `tasks.md`'s Notes section already
      flagged for task 8.5 before this session). The round-trip tests confirm this writer's output
      is internally consistent and spec-valid, not that it's byte-identical to the Swift app's own
      writer; capturing real `.flac`/`.mp3` fixtures from the macOS app and diffing against them is
      follow-up work, not something fabricable from this repo alone.

  **Verified:** all of task 8 builds, links, and passes under the MSVC toolchain — `cargo test -p
  walkman-services` (46 unit tests + the task-7 integration test, all green), `cargo test -p
  walkman-platform` (10 tests, incl. `fs_time`), both `clippy`-clean. `cargo test --workspace`
  passes in full.

- [x] 9. Implement the Walkman engine and device detection services
  - [x] 9.1 Port `device_detector.rs`: the Sony model catalog, supported/android checks, icon-key
        map, `inspect_volume` (capability XML parse, signatures, capacity-based WM1 M2 split),
        `detect_devices`, and write unit tests for model classification from fixture signatures.
    - _Requirements: 3.2, 3.3, 3.4_
    - Progress: `services/src/device_detector.rs` has the hardcoded catalog (NW-A50/A100/A300,
      ZX100/ZX300/ZX500/ZX700, WM1A/WM1Z/WM1AM2/WM1ZM2 — split into separate entries where PIDs
      differ, so `resolve_model` can tell e.g. WM1AM2 from WM1ZM2 by PID alone — and the
      WS/E/S/B series), `resolve_model` (PID-first, then a model-token substring match, with
      capacity-nearest-match to resolve same-PID/capacity-differentiated members like the A50
      series' A55/A56/A57), `is_supported_model`/`is_android_model`/`icon_key`, and
      `inspect_volume` (signature files/folders from R3.2, Internal-vs-MicroSD via the
      `Sony_Card.IND` marker, and a best-effort `default-capability.xml` scan for an
      `NW-XXXX`-shaped token — this repo has no sample of that file's real schema, so it scans
      for the token textually rather than parsing specific tags, documented in the module doc).
      `detect_devices` (the "group internal+microsd into devices" grouping) ended up in
      `walkman_engine.rs` instead, since it needs `VolumeProvider` for free-space/capacity and
      building it there avoided a `device_detector` → `walkman_engine` → `device_detector`
      dependency cycle; see that module's notes for the documented grouping-heuristic
      limitations. 11 tests, including one real bug caught and fixed during development
      (WM1AM2/WM1ZM2 were initially one shared catalog entry, so PID alone couldn't tell them
      apart as the task asks — split into two entries).
  - [x] 9.2 Port `usb_detector.rs` on top of `UsbEnumerator` to publish connected Walkmans.
    - _Requirements: 3.1_
    - Progress: `services/src/usb_detector.rs`. `scan_connected_walkmans` filters
      `UsbEnumerator::scan()` to Sony's vendor id and resolves each device against the catalog;
      `scan_unsupported` separately surfaces Sony-vendor-id devices that didn't resolve (R3.4:
      "unsupported model" should be recorded and surfaced, not silently dropped). Tested against
      `walkman_platform::fake::FakeUsbEnumerator`, not real hardware. 3 tests.
  - [x] 9.3 Port `walkman_engine.rs`: drive detection (VolumeProvider + signatures + free-space),
        custom-storage folder (plain stored path), `target_track_url`, playlist read/write/rename
        (CRLF `.m3u8`), byte/duration formatters, and the companion `.lrc` writer; write unit tests
        for the `.m3u8` CRLF/relative-path output and the `.lrc` formatter.
    - _Requirements: 3.2, 3.7, 11.1, 15.4_
    - Progress: `services/src/walkman_engine.rs`. `detect_drives` builds `WalkmanDrive`s from
      every authentic volume; `detect_devices` additionally groups them into `WalkmanDevice`s
      (see 9.1's note on where this landed and its documented grouping limits).
      `CustomStorageFolder` is a plain `PathBuf` wrapper (no security-scoped bookmark needed on
      Windows, per design.md). `target_track_fs_path` joins `domain::sanitize`'s
      already-sanitized relative path onto a music root with native separators.
      `write_playlist`/`read_playlist`/`rename_playlist`/`delete_playlist` round-trip a
      `#EXTM3U`/`#EXTINF`/CRLF `.m3u8` exactly (R11.1). `write_lrc` writes a companion `.lrc`
      with strict `[mm:ss.xx]` timestamps sorted chronologically, an optional `[offset:±ms]`
      header, CRLF, no BOM, and never touches the audio file itself (so its timestamps are
      preserved by construction, not by restoring them after the fact); `synthesize_lrc_lines`
      spaces out plain-text lines evenly across a duration when no sync points exist. `has_cfw`
      detection and the custom-firmware marker names are a documented best-effort placeholder —
      no authoritative list of Sony Walkman CFW install artifacts was available to source this
      from. 13 tests.
  - [x] 9.4 Implement Clean & Eject (Windows + residual macOS junk removal, flush, eject via
        `Ejector` with watchdog).
    - _Requirements: 16.1, 16.2_
    - Progress: `services/src/clean_eject.rs`'s `clean_junk` recursively removes the per-
      directory junk files R16.1 lists (`.DS_Store`, `Thumbs.db`, `desktop.ini`, `._*` sidecars)
      and the top-level-only junk folders (`$RECYCLE.BIN`, `System Volume Information`,
      `.Spotlight-V100`, `.Trashes`, `.fseventsd`), with every removal best-effort (a
      permission-denied folder, e.g. `System Volume Information` without elevation, is skipped,
      not an error). `clean_and_eject` runs that pass then calls `Ejector::eject` (already
      watchdog-bounded, task 6.5). Added a `FlushFileBuffers` call to
      `platform::windows::WindowsEjector::do_eject` (right after opening the volume handle,
      before locking) to actually satisfy R16.2's "flush writes and then safely eject", which
      task 6.5 had not done. 5 tests for `clean_eject`; `walkman-platform`'s existing eject tests
      (`fake::tests::fake_ejector_*`) still pass unchanged.

- [x] 10. Implement transcoding and the transfer queue
  - [x] 10.1 Port `transcode.rs` preset→ffmpeg-args table verbatim (all presets + loudnorm + `-vn`)
        using the bundled ffmpeg with bundled-first path discovery, and write unit tests asserting
        the generated argument list per preset.
    - _Requirements: 9.3, 19.2, 19.3_
    - Progress: `services/src/transcode.rs`. `Preset` covers every preset R9.3 lists (Copy/
      CopyOptimize/CopyRaw, MP3 128/192/256/320/V0, AAC 128/256/320, FLAC 16/44.1 + 24/48 + 24/96
      + 24/192, ALAC 16, WAV 16); `build_ffmpeg_args` is a pure function (no `ffmpeg` process
      spawned) asserted per-preset by 10 tests, matching the task's own "assert the generated
      argument list" instruction. `-vn` is always added (so an attached-picture video stream
      never rides along into a converted audio file) and `loudnorm` is silently dropped for the
      three bit-perfect presets, since `-c copy` and an audio filter can't be combined in
      `ffmpeg`. `CopyRaw` is a plain filesystem copy that never invokes `ffmpeg` at all — handled
      by `transfer_queue.rs`'s dispatch, not by this module's `transcode()`, which asserts rather
      than silently no-op'ing if called with it. Discovery reuses task 8.3's
      `ffmpeg_util::find_ffmpeg`; since `ffmpeg` isn't bundled yet (task 19.1), `transcode()`
      itself isn't exercised against a real binary, same documented gap as task 8.3's M4A path.
  - [x] 10.2 Port `transfer_queue.rs`: queue items/status, Sony-OS vs Android format rules, enqueue
        from tracks/paths, direct transfer (copy/convert/optimize/lyrics/sanitize), single-pass
        concurrent prep, dual target (MUSIC/LEARNING + chapter playlist), and progress reporting.
    - _Requirements: 9.1, 9.2, 9.4, 9.6, 9.7_
    - Progress: `services/src/transfer_queue.rs`. `TransferQueue` (a `Mutex<Vec<TransferItem>>`)
      covers enqueue/remove/clear/update/snapshot; `item_from_track`/`item_from_path` build a
      pending item from an indexed `Track` (R9.7's 1-click local-library transfer) or directly
      from a dropped file's tags (R9.1). `is_format_supported` applies distinct Sony-OS/Android
      extension tables (R9.6). `prepare_all` is the "single-pass concurrent metadata-preparation
      stage" (R9.7) — via `rayon`, across every not-yet-optimized item in the queue — but only
      extracts + Baseline-JPEG-optimizes embedded artwork today; lyrics fetching is task 12.2's
      job and isn't wired in (an item's `lyrics` field is simply carried through from whatever
      `item_from_path`'s tag read already found). `transfer_item` is the actual per-item transfer:
      lays out `/MUSIC/Artist/Album/NN - Title.ext` via `walkman_engine::target_track_fs_path`
      from *sanitized-for-filesystem* fields, copies (`CopyRaw`) or transcodes (every other
      preset) the audio, then writes *tag* fields sanitized via `sanitize_walkman_text` (which
      preserves things like `/` in "AC/DC" — only the path needs `sanitize_filename`'s stricter
      pass) plus any optimized artwork, and applies a `DuplicateResolution`
      (Skip/Overwrite/KeepBoth, defaulting to KeepBoth — it never silently overwrites) when the
      destination collides. `transfer_batch_async` runs a whole batch on a background thread,
      publishing `TransferProgress` per item over a channel (R9.6's `TransferSyncSheet`, same
      off-thread pattern as task 7.3). `write_chapter_playlist` + `target_root_for` cover R9.2's
      dual MUSIC/LEARNING targets and automatic chapter-playlist generation. 7 tests, including a
      full round trip through the real filesystem (fixture FLAC → transfer → read tags back) and
      a caught-and-fixed bug in the test itself (asserting the sanitized *path* component against
      the unsanitized *tag* value — fixed to assert each against the sanitization function that
      actually applies to it).
  - [x] 10.3 Port `duplicate_guard.rs` (per-mount SQLite cache check before transfer).
    - _Requirements: 9.5_
    - Progress: `services/src/duplicate_guard.rs`. `check_for_duplicate`/
      `check_batch_for_duplicates` match case/whitespace-insensitively against
      `library_scanner`'s per-mount `tracks_cache` (title+artist only — a lighter, faster check
      than `duplicate_finder.rs`'s task-11.1 ±4s/quality-diff clustering, which is for the
      standalone Duplicates view, not an inline pre-transfer gate). `resolve_keep_both_path`
      implements the `Keep Both` resolution by incrementing `"Title (2).ext"`, `"(3)"`, etc.
      against the real filesystem. 7 tests.

- [x] 11. Implement duplicate finder and backup/restore services
  - [x] 11.1 Port `duplicate_finder.rs`: same-storage grouping, exact vs cross-album scopes, ±4s
        clustering with normalized title/artist (version-tag preserving), quality-diff detection,
        recoverable-space totals, auto-resolve, and write unit tests for the matching/clustering
        logic.
    - _Requirements: 10.1, 10.2, 10.3, 10.4_
    - Progress: `services/src/duplicate_finder.rs`. Operates on one partition's tracks at a time
      (R10.1 — the caller, not this module, is what guarantees same-storage scoping, by only
      ever passing one mount's `tracks_for_mount` result); `find_duplicates` groups by normalized
      title/artist(/album for `ExactSameAlbum`) then splits each group into ±4s duration
      clusters, so a title/artist match with too large a duration gap isn't treated as a
      duplicate. Normalization deliberately does *not* strip parenthetical version tags
      ("(Live)"/"(Acoustic)"/etc.) — leaving them in the key is exactly what keeps those versions
      from colliding with the original (R10.2). `has_quality_diff`/`recoverable_bytes` compare
      bitrate/sample-rate/bit-depth/format across a group's members; `auto_resolve_quality` keeps
      `best_quality_track()` and deletes every other member's audio file plus its companion
      `.lrc`/`.txt` (R10.4). This module only has the matching/clustering/resolution *logic* —
      native confirmation dialogs (R10.5) are a UI-layer concern (task 17.2). 8 tests.
  - [x] 11.2 Port `backup_restore.rs`: scopes/destinations, PBKDF2-HMAC-SHA256 (100k) + AES-256-GCM
        encryption (16-byte salt, streamed) + manifest, free-space verify (GetDiskFreeSpaceExW),
        CopyFileExW + FlushFileBuffers, junk cleanup on restore, and cancelation.
    - _Requirements: 12.1, 12.2, 12.3, 12.4, 12.5, 12.6_
    - Progress: `services/src/backup_restore.rs`, plus a new `platform::fast_copy`
      (`CopyFileExW` + `FlushFileBuffers`, `platform/src/windows/fast_copy.rs`) alongside task
      8.4's `set_file_times`. Unencrypted backups are a plain `manifest.json` + `files/` tree
      directory bundle (R12.4); encrypted backups are a single `*.walkmanbackup.enc` file: a
      16-byte random salt, PBKDF2-HMAC-SHA256 (100,000 rounds, `pbkdf2`/`sha2` crates) deriving
      an AES-256-GCM key (`aes-gcm` crate), then the JSON manifest and every source file each
      sealed as their own AES-GCM ciphertext with a fresh random nonce. "Streamed" is interpreted
      here as *per-file* (never more than one source file in memory at once), not the `aes-gcm`
      crate's chunked `stream` AEAD construction — that exotic incremental-nonce API was judged
      not worth the added risk of getting subtly wrong with no way to check it against a second
      implementation; per-file AES-GCM blocks are simpler, still satisfy AEAD's
      nonce-uniqueness requirement (a fresh random nonce per file), and are what's actually
      tested. `verify_free_space` goes through `platform::VolumeProvider::free_space`
      (R12.6/R12.5); `restore_unencrypted_backup`/`restore_encrypted_backup` both finish with
      `clean_eject::clean_junk` (R12.5's "run junk-file cleanup"). Every multi-file loop takes a
      `CancelCheck` (`&dyn Fn() -> bool`) polled between files (R12.5's "cancelable"). Scopes
      (R12.2) are a plain enum recorded in the manifest; `default_backup_dir` gives R12.3's
      `%USERPROFILE%\Music\Walkman Studio Backups` default — external-drive/custom-folder
      destinations are just a different `dest_dir`/`dest_file` path, not separate code paths.
      5 tests, including a real encrypted round trip (encrypt → decrypt → compare) and a
      wrong-passphrase rejection — not mocked.
  - [x] 11.3 Write backup→restore round-trip integration tests for both encrypted and unencrypted
        bundles, asserting manifest contents and free-space checks.
    - _Requirements: 12.4, 12.5_
    - Progress: `services/tests/backup_restore_integration.rs`. Builds a small multi-file,
      multi-directory fixture library, backs it up (both formats), asserts the manifest's entry
      count and `total_bytes` against the real file sizes, exercises `verify_free_space` both
      failing (too little free space) and passing, restores, and compares every restored file's
      bytes against the original byte-for-byte. Distinct from 11.2's own unit tests (which cover
      the wrong-passphrase case and internal edge cases) — this is the public-API-only,
      consumer-style round trip the task asks for. 2 tests.

  **Verified:** all of task 11 builds, links, and passes under the MSVC toolchain —
  `cargo test -p walkman-services` (41 unit tests + 2 task-7 + 2 task-11 integration tests, all
  green), `cargo clippy --workspace --tests` clean. `cargo test --workspace` passes in full
  (288 tests). New workspace dependencies: `pbkdf2`, `hmac`, `sha2`, `aes-gcm` (encryption, task
  11.2 only — `hmac` is pulled in by `pbkdf2`/`sha2` but not used directly).

- [x] 12. Implement trim/cut, lyrics, online metadata, and art caches
  - [x] 12.1 Port `trim_cut.rs`: waveform generation (symphonia + ffmpeg fallback) and the ffmpeg
        filtergraph for trim/cut/channel ops/fades/gain, preserving tags+artwork afterward.
    - _Requirements: 14.1, 14.2, 14.3, 14.4, 14.5_
    - Progress: `services/src/trim_cut.rs`. `generate_waveform` decodes real PCM via `symphonia`
      (primary — added as a new workspace dependency with the `wav`/`flac`/`mp3`/`pcm` features;
      verified against a hand-built real WAV fixture, not a mock), falling back to shelling out
      to `ffmpeg` (raw `f32le` PCM on stdout) if that fails; both paths downsample to peak
      amplitudes via a shared `downsample_to_peaks`. `build_ffmpeg_args` always builds a single
      `-filter_complex` graph (even for a plain trim) so `Trim`/`Cut`, the six `ChannelMode`
      options (R14.4), fades, and gain compose into one pass with a final label that's always
      `[out]` regardless of which optional stages ran; `Cut` uses `asplit`+`atrim`+`concat`
      since removing a middle segment isn't expressible as a single `atrim`. `apply_trim_cut`
      reads the source's tags/artwork *before* running `ffmpeg` and re-applies them to the
      processed output afterward via `audio_tagger` (R14.5 — `ffmpeg`'s filter graph re-encodes,
      which drops them). 7 tests. The actual `ffmpeg` process invocation (`apply_trim_cut`, the
      `ffmpeg`-fallback waveform path) isn't exercised against a real binary — ffmpeg isn't
      bundled yet (task 19.1), same documented gap as every other `ffmpeg`-dependent path in
      this port.
  - [x] 12.2 Port `lyrics.rs`: LRCLIB (get/search, duration-matched, same user-agent) + Lyrics.ovh
        fallback, concurrent workers, and synced/plain/skipped/not-found accounting.
    - _Requirements: 15.1, 15.3_
    - Progress: `services/src/lyrics.rs`. URL-building (`build_lrclib_get_url`/
      `build_lrclib_search_url`/`build_lyrics_ovh_url`) and JSON-parsing
      (`parse_lrclib_get_response`/`parse_lrclib_search_response`/`parse_lyrics_ovh_response`)
      are pure, fixture-tested functions; `fetch_one` tries LRCLIB's `/get` (exact match) then
      falls back to `/search` (duration-matched against the closest result) then Lyrics.ovh, all
      through a single shared `reqwest::Client` carrying one `USER_AGENT` constant used by both
      APIs (R15.1). `fetch_batch_async` runs every request concurrently (a `tokio::sync::
      Semaphore` bounding concurrency, `tokio::task::JoinSet` for the actual tasks) on a
      background thread with its own Tokio runtime, publishing a progress update with each
      outcome over a channel — the one network-dependent path in this task that *is* exercised
      end-to-end in a test, since the "already has lyrics" case is a real `Skipped` outcome that
      never touches the network, which verifies the runtime/semaphore/channel plumbing itself
      without needing internet access. `tally` computes R15.3's four metric-pill counts. 10
      tests. The actual LRCLIB/Lyrics.ovh HTTP round trip isn't exercised against the live APIs —
      no internet access to verify against in this session, and no mock-HTTP dependency was
      added for it (documented gap, same shape as the `ffmpeg`-dependent gaps elsewhere).
  - [x] 12.3 Port `online_metadata.rs` (iTunes + Deezer) and `artist_art_cache.rs` /
        `cover_art_cache.rs` (disk + memory caches).
    - _Requirements: 17.1, 17.2, 17.3, 5.4_
    - Progress: `services/src/online_metadata.rs` mirrors `lyrics.rs`'s split (pure/tested
      URL-building + JSON-parsing; `fetch_metadata_candidates`/`fetch_artist_image_url`/
      `download_bytes` are real but unexercised network calls, same documented gap) for the
      iTunes Search API and Deezer's track and artist search endpoints, upgrading iTunes'
      `artworkUrl100` to `600x600` since that API has no explicit size parameter. Either source
      failing contributes no candidates rather than erroring the whole lookup (R17.3).
      `services/src/cover_art_cache.rs` implements the shared two-tier `ArtCache` (memory +
      disk, disk filenames are a hash of the cache key so arbitrarily-shaped keys like file
      paths or `"artist|album"` pairs never collide or need escaping) that both it (keyed by
      track path) and `services/src/artist_art_cache.rs` (keyed by `"artist|album"`, R5.4) use —
      one generic cache implementation behind the two source files design.md names, rather than
      duplicating the cache logic. 13 tests.

  **Verified:** all of task 12 builds, links, and passes under the MSVC toolchain —
  `cargo test -p walkman-services` (30 new unit tests: 7 `trim_cut` + 10 `lyrics` + 4
  `cover_art_cache` + 2 `artist_art_cache` + 7 `online_metadata`, all green), `cargo clippy
  --workspace --tests` clean. `cargo test --workspace` passes in full (318 tests). New workspace
  dependency: `symphonia` (waveform PCM decode, task 12.1 only).

- [x] 13. Implement audio playback (`player.rs`)
  - Implement the playback engine (`rodio`/Media Foundation) with play/pause/seek/next/prev, a queue
    + advance hook, progress reporting, and SMTC integration via `MediaControls`.
  - _Requirements: 7.1, 7.4, 7.5_
  - Progress: `services/src/player.rs`, on `rodio` (new workspace dependency, `symphonia-all` +
    `symphonia-alac` features — covers FLAC/MP3/WAV/AAC/ALAC/M4A per R7.1). `Player` owns the
    output stream/sink behind a `Mutex`; `play_index`/`play`/`pause`/`toggle_play_pause`/`seek`/
    `next`/`previous` cover the transport, `status()` reports position/duration/state for R7.1's
    progress scrubber, and `set_now_playing`/`set_playback_state` push to the `MediaControls`
    trait (task 6.7's `WindowsMediaControls`) on every load/play/pause for R7.4's SMTC
    integration. There's no background "watcher" thread for detecting a finished track;
    `Player::poll()` is a plain method the UI calls once per frame (the same
    channel/poll-driven threading model design.md uses elsewhere in this crate) that detects the
    sink going empty, runs the R7.5 advance hook, and auto-advances the queue. 5 tests —
    genuinely exercised against this dev host's real default audio output device via `rodio`/
    `cpal`/WASAPI (not mocked), including actually waiting for a short real WAV clip to finish
    playing and confirming `poll()` auto-advances; skipped gracefully (not failed) if a host has
    no audio output device, since that's an environment property this code doesn't control.

- [x] 14. Build the Local Library and Walkman library views
  - [x] 14.1 Implement `LocalLibraryView`: display modes (Songs/Albums/Artists/Recently Added/
        Folders), the 8 sort options, search, track/folder context menus (incl. Relocate Folder…),
        DirectTransfer sheet, format-warning + removal confirmation dialogs; Manage Local Folders
        sheet.
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5, 4.6_
    - Progress: `ui/src/views/local_library.rs` (new `ui/src/views/` module, per design.md's
      intended one-module-per-section layout). All 5 display modes (R4.2) and 8 sort options
      (R4.3, pure `sort_tracks`) are implemented; `filter_tracks` is the search bar's
      case/whitespace-insensitive title/artist/album match (R4.4). Songs/Recently Added render a
      sortable row list (plain hand-rolled rows via `theme::glass::liquid_glass`, not
      `egui_extras::TableBuilder` — consistent with the rest of this crate's hand-rolled-widget
      style so far); Albums/Artists render a simple grouped list (an `(album, artist)`/`artist`
      count rollup) rather than AlbumsView's (task 14.4) cover grid, since R4.2 only asks this
      view to offer those *display modes*, not duplicate 14.4's cover-grid presentation; Folders
      lists the registered `LocalMusicFolder`s with a `Relocate Folder…`/Rescan context menu
      (R4.4) and a `Remove Missing Folders and Tracks?` action. All four sheets exist (`Manage
      Local Folders…`, `DirectTransfer`, the format-warning alert with its `Transfer & Convert
      (All)` escalation, `Remove Folder from Local Library?`/`Remove Missing…`/`Audio File Not
      Found` confirmations), built on a new `modal()` helper that reuses `now_playing.rs`'s
      existing `Area`-based fullscreen-overlay pattern with `theme::glass::modal_glass_chassis`
      for the panel. The view owns its own `LocalLibraryViewState` (display mode/sort/search/
      selection/open sheet) and emits `LocalLibraryCommand`s into `state.commands` for whoever
      drains them to act on — real `walkman_services::local_library` wiring (populating
      `AppState::local_tracks`/`local_folders` and draining those commands) is task 18's job, not
      this one's; `AppState` gained those two data fields plus `local_library_view` as plain,
      currently-always-empty plumbing so `detail.rs`'s `NavTab::LocalLibrary` arm can call the
      real view instead of the placeholder. 8 tests, including a headless-egui-frame render
      across every (display mode × sheet) combination — same convention `detail.rs`'s own router
      test already established — which also now exercises this view for real through
      `detail_router_renders_in_headless_frame_for_every_tab` and
      `shell_composes_in_headless_frame_for_representative_states`, both of which still pass.
  - [x] 14.2 Implement `LibraryView` (All Songs): sortable multi-column table + search + batch-lyrics
        button + per-row context menu; shared selection (Shift-range/Ctrl-toggle) and 88pt inset.
    - _Requirements: 5.1, 2.9_
    - Progress: `ui/src/views/library.rs`, plus a new `ui/src/selection.rs` factoring out the
      Shift-range/Ctrl-toggle logic the task explicitly calls "shared" — every multi-select
      table/grid from 14.3 onward reuses `selection::handle_click` rather than reimplementing it.
      `Column` (Title/Artist/Album/Duration/Format) each sort ascending/descending via a
      click-the-header-to-toggle-direction header row (R5.1's "multi-column sortable"); the
      per-row context menu covers all five R5.1 actions (play/edit metadata/trim-cut/transfer/
      delete) plus a `Find Lyrics…` button for the current selection (R5.1's batch-lyrics
      button); the row list reserves `sizing::TABLE_BOTTOM_INSET` (R2.9). `AppState` gained
      `walkman_tracks`/`library_view` (same currently-empty-until-task-18 plumbing pattern as
      14.1's `local_tracks`/`local_folders`); `detail.rs`'s `NavTab::Library` arm now calls the
      real view. 10 new tests (4 in `library.rs`, 6 in `selection.rs`), plus the existing
      `detail_router_renders_in_headless_frame_for_every_tab`/
      `shell_composes_in_headless_frame_for_representative_states` tests now exercise this view
      for real too and still pass.
  - [x] 14.3 Implement `RecentlyAddedView`: time-scope picker (24h/7d/30d/All) + ranked table + batch
        lyrics.
    - _Requirements: 5.2_
    - Progress: `ui/src/views/recently_added.rs`. `ranked_tracks` filters to the picked
      `TimeScope`'s window (relative to a passed-in `now`, so it's deterministic/testable rather
      than calling `Utc::now()` internally) and sorts newest-first; the table shows an explicit
      `#N` rank column plus the shared context menu/selection/batch-lyrics pattern 14.2
      established (reusing `selection::handle_click`). Required promoting `chrono` from a
      test-only to a real `ui` crate dependency, since this is the first view whose production
      code (not just its tests) needs it. 4 new tests; wired into `detail.rs`'s
      `NavTab::RecentlyAdded` arm and a new `AppState::recently_added_view` field (reading the
      same `walkman_tracks` `library_view` does).
  - [x] 14.4 Implement `AlbumsView`: cover grid, 7 sort options, format filter chips, expandable
        search, album detail drill-down, album+track context menus, batch lyrics.
    - _Requirements: 5.3_
    - Progress: `ui/src/views/albums.rs`. `group_into_albums` groups tracks into
      `walkman_domain::models::AlbumItem`s (reusing the real domain model's `id()`/
      `is_hi_res`/`primary_format()` rather than inventing a parallel grouping type), all 7
      sort options, a format filter chip row built from whatever formats are actually present
      (`distinct_formats`) plus an independent Hi-Res toggle chip, an expandable search bar
      (collapsed by default, a button swaps it for a text field), and album-detail drill-down
      (double-click or context menu "Open Album" swaps the grid for a track list with its own
      context menu + a "Back to Albums" button). Grid tiles are placeholder glass tiles (an
      initial-letter monogram) rather than real cover art — bridging `cover_art_cache.rs`'s
      (task 12.3) bytes to an egui texture is a separate concern this task doesn't own. 7 new
      tests, including a headless-frame render of both the grid and the drill-down state; wired
      into `detail.rs`'s `NavTab::Albums` arm and a new `AppState::albums_view` field.
  - [x] 14.5 Implement `ArtistsView`: artist grid with avatars (cache-backed), search, drill-down,
        context menus.
    - _Requirements: 5.4_
    - Progress: `ui/src/views/artists.rs`. `group_into_artists` groups into real
      `walkman_domain::models::ArtistItem`s, reusing `albums::group_into_albums` per-artist so
      each artist's own album list is the same real grouping 14.4 already built, not a
      duplicate. Drill-down is the full artist → albums → tracks chain R5.4 asks for
      ("album/track drill-down"), modeled as an explicit `ArtistDrillDown` enum (`Grid`/
      `Albums`/`Tracks`) rather than a stack, since this view never needs more than one level of
      "back" at a time. Avatars are placeholder initial-monogram circles — bridging
      `artist_art_cache.rs`'s (task 12.3) cache-backed bytes to an egui texture is a separate
      concern this task doesn't own, same caveat as 14.4's cover tiles. 3 new tests, including a
      headless-frame render at all three drill-down levels; wired into `detail.rs`'s
      `NavTab::Artists` arm and a new `AppState::artists_view` field.
  - [x] 14.6 Implement `HiResAudioView`: view modes (Tracks/Albums/Artists), track + album sort
        options, format/quality filter chips, search, context menus, batch lyrics.
    - _Requirements: 5.5_
    - Progress: `ui/src/views/hi_res.rs`. `hi_res_tracks` is the real `Track::is_hi_res()`
      filter (>16-bit or >48kHz), applied once up front before any view mode renders. Tracks
      mode gets its own 6-option sort (R5.5's exact list) and format filter chips; Albums mode
      reuses `albums::{group_into_albums, sort_albums, AlbumSortOption}` directly rather than a
      parallel implementation (so its 7 sort options are 14.4's real ones, already tested
      there); Artists mode reuses `artists::group_into_artists`. Every row carries the Hi-Res
      gold badge, since every row here already passed the filter by definition. 4 new tests,
      including a headless-frame render across all 3 view modes; wired into `detail.rs`'s
      `NavTab::HiRes` arm and a new `AppState::hi_res_view` field.
  - [x] 14.7 Implement `LanguageStudyView`: 5 display modes, 6 sort options, two tables, context
        menus, batch lyrics.
    - _Requirements: 5.6_
    - Progress: `ui/src/views/language_study.rs`. `language_study_tracks` reuses the real
      `Track::is_language_study()` (R5.6's `/LEARNING/` path or language/audiobook/podcast/
      learning genre check, already ported in task 5.1 — not reimplemented here). All 5 display
      modes and all 6 sort options (R5.6's own "Speaker/Artist"/"Course/Album" terminology for
      the same artist/album fields) exist; Albums/Artists modes reuse `albums::
      group_into_albums`/`artists::group_into_artists` directly (same reuse pattern as 14.6);
      Folders mode groups by each track's immediate parent directory name
      (`group_by_folder`) — a reasonable, generically-applicable reading of "Folders" for
      Language Study material given the task's "two tables" note doesn't specify exactly which
      pairing, and this mirrors 14.1's own Folders mode in spirit. 4 new tests, including a
      headless-frame render across all 5 display modes; wired into `detail.rs`'s
      `NavTab::LanguageStudy` arm and a new `AppState::language_study_view` field.
  - [x] 14.8 Implement `DeviceOverviewView`: model image, specs, per-drive storage gauges, quick
        stats, Refresh/Eject, multi-device switcher; and the Android MTP help sheet + custom-folder
        bridge.
    - _Requirements: 3.5, 3.7_
    - Progress: `ui/src/views/device_overview.rs`. Renders a "No Walkman Connected" placeholder
      when `devices` is empty; otherwise a multi-device switcher (shown only when there's more
      than one, matching `sidebar.rs`'s own `DeviceSectionMode` convention) over the real
      `WalkmanDevice`/`WalkmanDeviceInfo`/`WalkmanDrive` domain models — specs (series/OS
      family/firmware/resolution/capacity), per-drive `theme::widgets::StorageGauge`s (reusing
      the existing gauge widget, not a new one), quick stats (album/artist/hi-res counts,
      supplied by the caller via a `DeviceQuickStats` parameter since the real numbers come from
      the task 7.3/18 precomputed caches), and Refresh/Eject buttons (Eject targets the
      internal drive's mount point, falling back to the first drive). An Android device
      (`device_info.is_android()`) shows the MTP banner leading to the help sheet + a
      `PickCustomFolder` command for R3.7's custom-folder bridge (the actual folder picker is
      `platform::FileDialogs`, task 6.6, invoked by whoever drains this view's commands). No
      real device model image is wired up yet — that needs a bundled asset keyed by
      `specific_model`, a packaging concern (task 19.1); the header shows a placeholder glyph
      tile instead. 5 new tests; wired into `detail.rs`'s `NavTab::Overview` arm and new
      `AppState::devices`/`device_quick_stats`/`device_overview_view` fields.

- [x] 15. Build playback UI: now-playing bar, fullscreen player, SensMe channels
  - [x] 15.1 Implement `TrackPreviewBar` bound to the player (artwork, title/artist, transport,
        scrubber, open-inspector, open-fullscreen).
    - _Requirements: 7.2_
    - Progress: `ui/src/now_playing.rs` gains a new `AppState::player: Option<Arc<
      walkman_services::player::Player>>` field (`None` until task 18 constructs one — the
      bar/overlay still fall back to the task 4.4 placeholder behavior with no player bound)
      and `sync_now_playing_from_player`, called once per frame from `shell::layout_shell`
      before the bar/overlay render: it polls the real `Player` (R7.5's auto-advance) and
      refreshes `AppState::now_playing` from `Player::status()`/`queue()` each frame, mapping
      `bits_per_sample > 16 || sample_rate > 48_000` to the bar's existing Hi-Res badge. The
      transport buttons and the scrubber's seek now call the real `Player::previous`/
      `toggle_play_pause`/`next`/`seek` directly when a player is bound, *in addition to* still
      raising the existing one-shot `PlayerRequests` flags, so the task 4.4 flag-based tests
      keep passing either way. 2 new tests (a bound-`Player` round trip, using a real default
      audio output device exactly like `player.rs`'s own tests — skipped on a host with none —
      and a no-player no-op check).
  - [x] 15.2 Implement `FullScreenNowPlayingView`: large artwork, blurred ambient background,
        transport, karaoke auto-scroll synced lyrics (~0.3s lead), active-line highlight.
    - _Requirements: 7.3_
    - Progress: `walkman_engine.rs` gains `parse_lrc` (the read-side counterpart to the
      existing `write_lrc`/task 9.3 — parses multi-timestamp lines and skips non-timestamp
      header tags like `[offset:...]`) and `active_lyric_line_index(lines, position, lead)`
      (R7.3's ~0.3s lead-time lookup), both with 5 new unit tests. `now_playing.rs`'s
      `render_fullscreen_overlay` is rewritten from the task 4.4 title-only placeholder to: a
      large (220x220) artwork tile + HR badge, title/artist, a karaoke lyrics window (the
      active line plus 2 lines of context either side, from a new `AppState::lyrics_lines: Vec<
      LrcLine>` / `lyrics_lead_seconds: f64` — empty/0.3s respectively until task 18 loads a
      real `.lrc` file) with the active line highlighted in silver and the rest muted, and the
      bar's own real `transport_controls`/`scrubber_row` reused verbatim so fullscreen playback
      control is identical to the floating bar's, not a parallel implementation. Two honest
      simplifications, documented in the function's own doc comment: artwork stays a
      placeholder tile (no texture pipeline exists yet — same caveat as 14.4/14.5's grid tiles)
      and the "blurred ambient background" is a flat tint wash (egui's 2D painter has no blur
      filter) rather than a real blur of the (placeholder) artwork. 2 new tests, including a
      headless-frame render across 5 playback positions (before/mid/after the loaded lyrics).
  - [x] 15.3 Implement `SenseMeChannelsView`: 3D cover-flow carousel of the 14 channels + Shuffle
        All, atmospheric backgrounds, gapless highlight-seek playback, live lyrics, analysis
        trigger.
    - _Requirements: 6.4, 6.5_
    - Progress: `ui/src/views/senseme.rs`. Reuses the real `walkman_domain::senseme::
      SenseMeChannel::ALL` (task 5.4's 14-channel catalog, already carrying display names and
      tokens) and filters tracks by whether `Track::senseme_channels` contains a channel's
      `channel_token()` — the same field `classify()` already populates, so "Shuffle All
      Songs" needs no special case (every analyzed track carries that token). A toolbar
      exposes a global "Shuffle All Songs" action and an "Analyze Library (n)" trigger for
      every `!is_senseme_analyzed` track; the carousel itself is a 5-tile flat strip (selected
      \u{B1}2) with Previous/Next buttons and click-to-select, and the selected channel's detail
      panel gets Play/Shuffle buttons. Shuffling uses a small deterministic splitmix64
      Fisher-Yates (`shuffled_ids`) rather than pulling in a `rand` dependency for one call
      site. Three documented honest simplifications: the "3D cover-flow" is a flat strip (no
      perspective transform in egui's 2D painter), "atmospheric backgrounds" are the existing
      glass tiles (no per-channel art asset), and "live lyrics during SensMe playback" is not
      a second lyrics renderer — Play/Shuffle are meant to hand off to the already-player-bound
      fullscreen view from tasks 15.1/15.2, which already does karaoke lyrics; this view only
      resolves *which* track ids to queue, emitting `PlayChannel`/`ShuffleChannel`/
      `AnalyzeUnanalyzed` commands for task 18 to drain against the real player/analysis
      service. 10 new tests, including a headless-frame render across empty/populated/
      boundary-index states; wired into `detail.rs`'s `NavTab::SenseMe` arm (replacing its
      placeholder) and a new `AppState::senseme_view` field.

- [x] 16. Build metadata editors, inspector, and artwork export
  - [x] 16.1 Implement `MetadataStudioView` inspector (300pt): artwork, metadata, specs, Edit button,
        device lyrics display.
    - _Requirements: 8.1, 15.5_
    - Progress: `ui/src/inspector.rs`. Renders the real selected `Track` (resolved in
      `shell.rs` from `AppState::selected_track_id` against `walkman_tracks`/`local_tracks`) —
      metadata (album/track#/year/genre), audio specs (format/bitrate/sample rate/bit depth/
      duration/size, with the existing Hi-Res badge for `Track::is_hi_res()`), an `Edit`
      button that pushes `InspectorCommand::EditRequested(track_id)`, and a lyrics section
      driven by a new `AppState::inspector_lyrics: Option<String>` (empty until task 18 wires
      the real companion-file/embedded-tag reader in — R15.5's actual decoding is a services
      concern this view doesn't own). Artwork stays a placeholder tile, same caveat as every
      other grid/detail view since task 14.4. `shell.rs`'s `layout_shell` now drains
      `EditRequested` directly into the new Song Metadata Editor sheet (task 16.2) each
      frame — opening a sheet is pure UI state with no service call behind it, unlike every
      other view's own `Edit*Metadata` commands, which stay queued for task 18 to drain the
      same way once they *do* need a service. 6 new tests (4 pure formatting-helper tests + a
      command test + a headless-frame render across empty/populated/with-lyrics states) plus
      a new `shell.rs` test proving the drain actually opens the editor with the right track.
  - [x] 16.2 Implement `SongMetadataEditorSheet`: tabs (Tags & Specs / Album Artwork), all editable
        fields, artwork drop zone + compliance badges, export-to-PC, online metadata + lyrics
        search; modal size ≈960×860.
    - _Requirements: 8.2, 13.2, 13.3, 15.1_
    - Progress: `ui/src/sheets/song_metadata.rs` (+ a new `ui/src/sheets/mod.rs` holding the
      shared `modal()`/`text_field()` helpers every task-16 sheet now uses, factored out of
      `views/local_library.rs`'s original private copy rather than reimplemented per sheet).
      A `SongEditorFields` working copy (Title/Artist/Album/Track #/Disc #/Year/Genre/Lyrics)
      seeds from the real `Track` on open and round-trips back out via `Save`, so `Cancel`
      discards edits for free; Disc Number always seeds empty since `Track` doesn't carry one
      (ported 1:1 from the Swift model and `audio_tagger::TagFields`'s own documented
      writer-only field) but still saves through. The Album Artwork tab reuses the *exact*
      `artwork::ArtworkCompliance::badges()`/`is_walkman_ready()` already built in task 8.1
      for its compliance badges rather than re-deriving them, against a placeholder tile (no
      texture pipeline, same caveat as 16.1). Online metadata search and lyrics search render
      whatever `online_metadata::MetadataCandidate`/lyrics text the (not-yet-wired, task 18)
      service call hands back in `online_candidates`/`lyrics_candidate`, with "Apply" buttons
      that copy fields in locally — no network call happens in this crate. Drag-and-drop
      artwork import isn't wired (egui has no native OS drop affordance here yet); Import/
      Export buttons emit commands instead for the real file-dialog platform call to fulfil.
      14 new tests (seeding, stale-result reset, candidate-apply field scoping, close
      semantics, and a headless-frame render across closed/both tabs/with-results).
  - [x] 16.3 Implement `AlbumMetadataEditorSheet`: batch field edit, unified artwork + `folder.jpg`,
        tracklist reorder + renumber (`1..N`/`01..N`).
    - _Requirements: 8.3_
    - Progress: `ui/src/sheets/album_metadata.rs`. Batch fields (Title/Artist/Year/Genre) seed
      from the album's first track (editing an album as a unit, any track's album-level
      fields are an equally valid seed); a `Vec<AlbumEditorTrackRow>` tracklist supports
      Up/Down reordering (`move_row_up`/`move_row_down`, each a boundary-safe no-op) and
      `renumber()` with both `1..N` and zero-padded `01..N` styles (width derived from the row
      count, so a 100+ track album pads to 3 digits, not hardcoded to 2) — `Save` carries the
      final `track_ids`/`track_numbers` in display order. Reuses the same placeholder-tile +
      `ArtworkCompliance` badge pattern as 16.2 for the unified artwork + its companion
      `folder.jpg` note, not a parallel implementation. 10 new tests (renumbering at several
      row counts, reordering at both boundaries, empty-album seeding, and a headless-frame
      render).
  - [x] 16.4 Implement `TransferQueueItemEditorSheet`: per-item tabs + online metadata/lyrics fetch.
    - _Requirements: 9.1_
    - Progress: `ui/src/sheets/transfer_item.rs`, opening directly from the real
      `walkman_domain::models::TransferItem` (task 10.2) via a new
      `SongEditorFields::from_transfer_item` (seeding the *same*
      `song_metadata::SongEditorFields` working copy the Song editor already uses, since a
      transfer item's editable tag set is identical) rather than a parallel field set.
      `status_label()` covers every `TransferStatus` variant (including the `Skipped`/
      `Failed` reason text) for the sheet's header, and is re-exported from
      `views/transfer_queue.rs` (task 17.1) so the queue's row list and this sheet never
      drift. Online metadata/lyrics search reuse the identical pattern as 16.2's (candidates/
      lyrics rendered from state, "Apply" copies fields locally, no network call in this
      crate). 8 new tests (status labels, seeding/reset/close lifecycle, and a headless-frame
      render across closed/pending/failed-with-results states); wired into
      `AppState::transfer_item_editor` and rendered from `shell.rs`, opened by 17.1's
      `OpenEditor` command.

- [ ] 17. Build tool views and sheets
  - [x] 17.1 Implement `TransferQueueView`: queue list with status, expandable rows, per-item editor,
        preview context menu, duplicate-import guard sheet (Skip/Overwrite/Keep Both), clear-queue
        confirm, `TransferSyncSheet` progress.
    - _Requirements: 9.1, 9.5, 9.6_
    - Progress: `ui/src/views/transfer_queue.rs`, over the real `walkman_domain::models::
      TransferItem`/`TransferTargetMode` and `walkman_services::{transcode::Preset,
      duplicate_guard::DuplicateResolution}` task 10 already built — no parallel queue-item
      type. This also prompted fixing 16.4's `TransferItemEditorSheet` (committed in task 16
      before `TransferItem` was noticed to already exist in `models.rs`) to seed its working
      copy from the real type via a new `SongEditorFields::from_transfer_item`, dropping the
      placeholder `TransferItemSummary` it had invented. Rows render `TransferItem::
      is_expanded`/`is_duplicate_on_walkman`/`duplicate_match_info` directly from the real
      struct (no separate view-local expansion map); a target-mode toggle, a 16-preset combo
      (`preset_label`, R9.3's 4-minimum presets first), and a loudness-normalize checkbox
      drive `StartTransfer`. `ClearConfirm` is fully local (no service round trip needed);
      `DuplicateGuard`/`Progress` are populated externally by whichever task-18 code runs
      `duplicate_guard::check_batch_for_duplicates`/`transfer_batch_async` and streams
      `TransferProgress` back in — this view only renders `TransferQueueViewState::sheet`
      exactly as told, same as every other task-14/15 view's "empty until task 18" data
      fields. Drag-and-drop enqueueing isn't wired (same egui limitation as every import path
      since 16.2). 4 new tests (preset-label distinctness, default state, and a headless-frame
      render across empty/populated/every-sheet-variant combinations); wired into `detail.rs`'s
      `NavTab::Transfer` arm and new `AppState::transfer_queue_items`/`transfer_queue_view`
      fields, with `shell.rs` draining `OpenEditor` straight into the transfer-item editor
      (same "opening a sheet needs no service call" reasoning as the inspector's `Edit` drain
      from 16.1) while every other command stays queued for task 18.
  - [x] 17.2 Implement `DuplicatesView`: scope/filter picker, 4 sort options, per-group modal
        resolution, recoverable totals, audition, reveal-in-Explorer, Auto-Resolve Quality, delete
        confirms.
    - _Requirements: 10.4, 10.5_
    - Progress: `ui/src/views/duplicates.rs`, over the real `duplicate_finder::{find_duplicates,
      sort_groups, total_recoverable_bytes, DuplicateGroup}` clustering already built in task
      11.1 — grouping/sorting/recoverable-bytes math isn't re-derived. Since `DuplicateGroup`
      carries no id, a new `group_key()` (every member's full path, sorted and joined) gives
      the expand/collapse state a stable identity across re-sorts within a frame. Per-group
      "Keep Best Quality" and the toolbar's whole-library "Auto-Resolve Quality" both resolve
      through the same `best_quality_track()` the service already exposes, so the UI never
      decides quality itself; both go through an in-app confirm sheet first (R10.5's "native
      confirmation dialogs" — the real OS dialog is a `platform::FileDialogs`/task 6.6
      wiring concern for task 18, not a UI-layer one; this view only renders its own confirm
      and emits the delete command on confirmation). Reuses `inspector::format_size` for
      byte totals rather than a second formatter. 4 new tests (group-key stability/
      distinctness, default state, and a headless-frame render across empty/every-sheet-
      variant/expanded-group states); wired into `detail.rs`'s `NavTab::Duplicates` arm and a
      new `AppState::duplicates_view` field.
  - [x] 17.3 Implement `PlaylistsView`: create/rename/delete/edit `.m3u8`, add-tracks browser modes,
        multi-select (Shift/Ctrl + header Select), reorder, floating batch action bar (Move to Top /
        Up / Down / To Bottom / remove) with instant sync.
    - _Requirements: 11.1, 11.2, 11.3_
    - Progress: `ui/src/views/playlists.rs`, over the real `walkman_domain::models::Playlist`
      and the `.m3u8` read/write/rename/delete functions already in `walkman_engine.rs`
      (task 9.3) — no parallel playlist format logic. A working-copy `active_tracks: Vec<
      String>` re-seeds from the selected `Playlist`'s real track list only when the
      selection changes (`sync_active_tracks`, tracked via a private `loaded_for` field), so
      the batch-bar's local edits survive re-renders without a stale selection silently
      overwriting a freshly-picked playlist. Since a playlist's relative paths aren't unique
      (the same track can be queued twice), selection/reorder use the row's *index* as the
      synthetic identity for `selection::handle_click`, not the path string. `shift_selected`
      moves the whole selected block past one adjacent row per click (not each row
      independently), matching "group Up/Down shift"; `move_selected_to_top`/`_to_bottom`/
      `remove_selected` are the other 3 pure reorder ops, each one call from
      `apply_reorder`, which also emits the resulting order as a single `Reorder` command —
      R11.1/11.3's "instant sync" is therefore literally one command per change, not a
      batched diff. The add-tracks browser is a mode-tabbed search+checklist over whatever
      `Track` slice the caller passes (`tracks_for_add_mode`); Albums/Artists modes sort by
      their grouping key rather than a full drill-down browser nested inside this sheet — a
      documented honest simplification of R11.2's "the other add-browsing modes" wording. 9
      new tests (the 4 reorder ops at their boundaries, add-mode sorting, the sync/no-op
      selection-change cases, and a headless-frame render across every sheet variant); wired
      into `detail.rs`'s `NavTab::Playlists` arm and new `AppState::playlists`/
      `playlists_view` fields.
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
- Status (workspace at `windows-app/`): tasks 1, 2, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, and 16
  complete; 5.2, 5.3, 6.1, 6.2, 6.4, 6.5, 6.6, 7.1, 7.2, 7.3, 7.4, 8.1, 8.2, 8.3, 8.4, 8.5, 9.1,
  9.2, 9.3, 9.4, 10.1, 10.2, 10.3, 11.1, 11.2, 11.3, 12.1, 12.2, 12.3, and every 14.x/15.x/16.x
  subtask complete;
  3.1, 5.1, 5.4, 6.3, and 6.7 partially done (see per-task Progress notes). The dev host switched
  to the MSVC toolchain (Visual Studio Build Tools installed,
  `rustup default stable-x86_64-pc-windows-msvc`), which resolved the `windows-gnu`
  dlltool/raw-dylib linking issue and the missing-`gcc.exe` issue together (both `rusqlite`'s
  bundled SQLite and task 7/8's own needs depend on a working C compiler). `cargo test
  --workspace` passes in full (`domain`/`platform`/`services`/`ui`, 0 failed, 429 tests) —
  including task 13's player tests against this host's *real* audio output device (not mocked),
  task 15.1's test that the now-playing bar mirrors that same real, bound `Player`, and task
  14/15/16's headless-egui-frame render tests across every view/mode/sheet/carousel-position/
  editor-state combination. 6.3
  and 6.7 stay `[~]` because they're intentionally partial (poll-only device events; no
  artwork/command callbacks yet); task 8.5 is checked off despite a known, documented gap (no
  real macOS-app-captured golden-byte fixtures exist in this repo, so the writers are
  round-trip-tested, not byte-diffed against the Swift app's own output) rather than marked
  unverified, since everything that *can* be tested without those fixtures is; task 9's
  `detect_devices` device/drive-grouping heuristic, task 10.1's un-exercised-against-a-real-
  `ffmpeg.exe` transcode path, task 11.2's per-file (not `aes-gcm`'s chunked `stream`
  construction) interpretation of "streamed" encryption, task 12's un-exercised-against-the-
  live-network LRCLIB/Lyrics.ovh/iTunes/Deezer calls (and `trim_cut.rs`'s `ffmpeg` paths), and
  task 14's placeholder cover-art/avatar/device-image tiles (real texture loading from the
  task-12.3 art caches is a separate concern no task 14 subtask owns) have similarly documented
  limitations (see their own Progress notes) rather than being unverified. New workspace
  dependencies: task 7 added `walkdir` (recursive scan), `lofty` (tag reading only — task 8's
  native readers/writers are separate), `uuid` (folder ids), `tempfile` (test fixtures); task 11
  added `pbkdf2`, `hmac`, `sha2`, `aes-gcm` (encryption); task 12 added `symphonia` (waveform PCM
  decode); task 13 added `rodio` (playback engine); task 14 added none (it builds entirely on
  the existing `theme`/`shell`/`now_playing` vocabulary plus a new shared `ui/src/selection.rs`);
  tasks 8, 9, and 10 added none either. Task 14 also established `ui/src/views/` (one module per
  nav section, per design.md) and the house style every subtask followed: each view owns its own
  `*ViewState` struct (not bolted onto `AppState` beyond a couple of plain, currently-empty data
  fields + the view state itself) and emits a `*Command` enum into `state.commands` for task 18
  to drain against real `walkman_services` calls — none of those commands are consumed yet.
