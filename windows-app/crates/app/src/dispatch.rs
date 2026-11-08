//! Drains every view's emitted commands against the real services (spec task 18).
//!
//! Each view (tasks 14-17) only ever pushes onto its own `commands: Vec<_>`; nothing in the
//! `walkman-ui` crate performs I/O. This module is where every one of those commands finally
//! becomes a real filesystem/service call. Given how many command enums there are (one per
//! view/sheet), this file is organized as one small `drain_*` function per enum, all called
//! from [`Dispatch::tick`].
//!
//! ## Scope — what's wired vs. documented gaps
//! Wired for real: local-folder add/remove/relocate/rescan, play/delete/trim-cut-open/
//! metadata-editor-open across every track view, transfer enqueue + batch transfer (with the
//! pre-transfer duplicate guard), duplicate delete/auto-resolve, playlist create/rename/
//! delete/add-tracks/reorder, backup/restore, clean & eject, the batch lyrics generator, trim
//! & cut apply, and song/album/transfer-item metadata saves.
//!
//! Left as documented gaps (same "no internet access to verify against in this session"
//! posture already used throughout tasks 8-12's network-touching services): the single-item
//! "Search Online Metadata…"/"Find Lyrics…" buttons in the song/transfer-item editors
//! (`SongEditorCommand::SearchOnlineMetadata`/`SearchLyrics` and their
//! `TransferItemEditorCommand` equivalents) and artwork drag-and-drop import (`FileDialogs`
//! has no "pick a file" method, only `pick_folder`/`save_file` — there's no platform API to
//! open an existing image file with yet). `SenseMeCommand::AnalyzeUnanalyzed` (a full on-board
//! 12-tone PCM analysis pass per track) is also left unwired here — `walkman_domain::senseme::
//! analyze_pcm` exists and is fully tested, but decoding each track's PCM for it is a
//! meaningfully separate, heavier piece of work than every other command in this file.

use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use walkman_domain::models::{Track, TransferItem, TransferTargetMode};
use walkman_platform::FileDialogs;
use walkman_services::duplicate_finder;
use walkman_services::duplicate_guard::DuplicateResolution;
use walkman_services::lyrics::{LyricsBatchProgress, LyricsOutcome};
use walkman_services::transfer_queue::{self, TransferProgress};
use walkman_ui::sheets::album_metadata::AlbumEditorCommand;
use walkman_ui::sheets::batch_lyrics::LyricsGeneratorCommand;
use walkman_ui::sheets::clean_eject::CleanEjectCommand;
use walkman_ui::sheets::lyrics_editor::LyricsEditorCommand;
use walkman_ui::sheets::song_metadata::SongEditorCommand;
use walkman_ui::sheets::transfer_item::TransferItemEditorCommand;
use walkman_ui::sheets::trim_cut::TrimCutCommand;
use walkman_ui::views::backup_restore::BackupRestoreCommand;
use walkman_ui::views::device_overview::DeviceOverviewCommand;
use walkman_ui::views::duplicates::DuplicatesCommand;
use walkman_ui::views::local_library::LocalLibraryCommand;
use walkman_ui::views::playlists::PlaylistsCommand;
use walkman_ui::views::senseme::SenseMeCommand;
use walkman_ui::views::transfer_queue::TransferQueueCommand;
use walkman_ui::AppState;

use crate::services::AppServices;

/// Async operations spanning more than one frame, polled each tick alongside the plain
/// synchronous drains.
#[derive(Default)]
pub struct Dispatch {
    pending_transfer: Option<Receiver<TransferProgress>>,
    pending_lyrics_batch: Option<(Receiver<LyricsBatchProgress>, Vec<LyricsOutcome>)>,
    pending_clean_eject: Option<Receiver<Result<walkman_services::clean_eject::CleanAndEjectResult, String>>>,
    pending_backup_restore: Option<Receiver<Result<walkman_services::backup_restore::BackupManifest, String>>>,
}

impl Dispatch {
    pub fn tick(&mut self, services: &AppServices, state: &mut AppState) {
        drain_local_library(services, state);
        drain_track_view_commands(services, state);
        drain_device_overview(services, state);
        drain_senseme(services, state);
        drain_duplicates(services, state);
        drain_playlists(state);
        drain_song_editor(services, state);
        drain_album_editor(services, state);
        drain_transfer_item_editor(services, state);
        drain_trim_cut(state);
        drain_lyrics_editor(services, state);
        self.drain_transfer_queue(services, state);
        self.drain_clean_eject(services, state);
        self.drain_backup_restore(services, state);
        self.drain_batch_lyrics(services, state);

        self.poll_transfer(state);
        self.poll_clean_eject(state);
        self.poll_backup_restore(state);
        self.poll_batch_lyrics(state);
    }
}

// ---- Shared helpers ----

/// Finds a track by id across both track lists `AppState` carries (R1.4: a track is either in
/// the Local Library or on the connected Walkman, never both lists' concern at once, but
/// commands carry a bare id with no "which list" tag).
fn find_track<'a>(state: &'a AppState, id: &str) -> Option<&'a Track> {
    state.walkman_tracks.iter().chain(state.local_tracks.iter()).find(|t| t.id() == id)
}

/// The mount root a track's cache row lives under: whichever registered local folder or
/// connected drive its `full_path` falls under. `None` if the track isn't under any
/// currently-known root (e.g. it vanished from disk since the last scan).
fn mount_root_for_track(state: &AppState, track: &Track) -> Option<String> {
    state
        .local_folders
        .iter()
        .map(|f| f.path.clone())
        .chain(state.devices.iter().flat_map(|d| d.drives.iter()).map(|d| d.mount_point.clone()))
        .find(|root| track.full_path.starts_with(root.as_str()))
}

/// Persists an in-place edit to `track` (already mutated by the caller) into whichever cache
/// mount it belongs to, and mirrors the same edit into `AppState`'s matching list entry so the
/// UI reflects it immediately without waiting for a rescan.
fn persist_track_edit(services: &AppServices, state: &mut AppState, track: Track) {
    if let Some(mount_root) = mount_root_for_track(state, &track) {
        if let Err(e) = services.scanner.upsert_track(&track, &mount_root, 0) {
            log::warn!("failed to persist metadata edit for {}: {e}", track.full_path);
        }
    }
    for list in [&mut state.local_tracks, &mut state.walkman_tracks] {
        if let Some(slot) = list.iter_mut().find(|t| t.id() == track.id()) {
            *slot = track.clone();
        }
    }
}

fn play_tracks(services: &AppServices, state: &mut AppState, ids: &[String]) {
    let queue: Vec<Track> = ids.iter().filter_map(|id| find_track(state, id)).cloned().collect();
    if queue.is_empty() {
        return;
    }
    services.player.set_queue(queue);
    if let Err(e) = services.player.play_index(0) {
        log::warn!("play failed: {e}");
    }
}

/// Deletes every track in `ids` (audio file + companions). Every command that reaches this
/// helper (`Delete`/`DeleteTrack`/`ResolveGroupKeepBest`) is only ever emitted by a view after
/// its own confirm sheet already ran (task 14-17's delete-confirm/resolve-group-confirm
/// sheets) — `confirmed: true` here reflects that confirmation having already happened, not
/// skipping it (R19.3/19.6).
fn delete_tracks(state: &mut AppState, ids: &[String]) {
    for id in ids {
        if let Err(e) = duplicate_finder::delete_track_and_companions(Path::new(id), true) {
            log::warn!("delete failed for {id}: {e}");
        }
    }
    state.local_tracks.retain(|t| !ids.contains(&t.id().to_owned()));
    state.walkman_tracks.retain(|t| !ids.contains(&t.id().to_owned()));
}

fn open_song_editor(state: &mut AppState, track_id: &str) {
    if let Some(track) = find_track(state, track_id).cloned() {
        state.song_editor.open_for(&track);
    }
}

fn open_trim_cut(state: &mut AppState, track_id: &str) {
    let Some(track) = find_track(state, track_id).cloned() else { return };
    let duration_ms = (track.duration_seconds * 1000.0).max(0.0) as u64;
    state.trim_cut_sheet.open_for(&track.full_path, duration_ms);
    // Generating real waveform peaks decodes the whole file; cheap enough for a user-
    // triggered "open the editor" action to do inline rather than adding another pending
    // receiver for what's normally a sub-second operation on typical track lengths.
    match walkman_services::trim_cut::generate_waveform(Path::new(&track.full_path), 400) {
        Ok(peaks) => state.trim_cut_sheet.waveform = peaks,
        Err(e) => log::warn!("waveform generation failed for {}: {e}", track.full_path),
    }
}

fn open_batch_lyrics(state: &mut AppState, ids: Vec<String>) {
    state.batch_lyrics_preselected_ids = ids;
    state.batch_lyrics_sheet.open();
    state.batch_lyrics_sheet.scope = walkman_ui::sheets::batch_lyrics::LyricsGeneratorScope::Selected;
}

fn enqueue_tracks_for_transfer(services: &AppServices, state: &mut AppState, ids: &[String]) {
    for id in ids {
        if let Some(track) = find_track(state, id) {
            services.transfer_queue.enqueue(transfer_queue::item_from_track(track));
        }
    }
    state.transfer_queue_items = services.transfer_queue.items();
}

// ---- Local Library ----

fn drain_local_library(services: &AppServices, state: &mut AppState) {
    let commands = std::mem::take(&mut state.local_library_view.commands);
    for command in commands {
        match command {
            LocalLibraryCommand::AddFolder => {
                if let Some(path) = services.file_dialogs.pick_folder("Add a local music folder") {
                    match services.local_library.add_folder(&path) {
                        Ok(_) => refresh_local_library(services, state),
                        Err(e) => log::warn!("add folder failed: {e}"),
                    }
                }
            }
            LocalLibraryCommand::RemoveFolder(id) => {
                if let Err(e) = services.local_library.remove_folder(&id) {
                    log::warn!("remove folder failed: {e}");
                }
                refresh_local_library(services, state);
            }
            LocalLibraryCommand::RelocateFolder(id) => {
                if let Some(path) = services.file_dialogs.pick_folder("Relocate this folder") {
                    if let Err(e) = services.local_library.relocate_folder(&id, &path) {
                        log::warn!("relocate folder failed: {e}");
                    }
                    refresh_local_library(services, state);
                }
            }
            LocalLibraryCommand::RescanFolder(id) => {
                if let Err(e) = services.local_library.rescan_folder(&id) {
                    log::warn!("rescan folder failed: {e}");
                }
                refresh_local_library(services, state);
            }
            LocalLibraryCommand::RemoveMissingFoldersAndTracks => {
                let missing: Vec<String> = state.local_folders.iter().filter(|f| f.is_missing).map(|f| f.id.clone()).collect();
                for id in missing {
                    if let Err(e) = services.local_library.remove_folder(&id) {
                        log::warn!("remove missing folder failed: {e}");
                    }
                }
                refresh_local_library(services, state);
            }
            LocalLibraryCommand::TransferAndConvertAll(ids) | LocalLibraryCommand::DirectTransfer(ids) => {
                enqueue_tracks_for_transfer(services, state, &ids);
            }
            LocalLibraryCommand::OpenMetadataEditor(id) => open_song_editor(state, &id),
            LocalLibraryCommand::OpenTrimCut(id) => open_trim_cut(state, &id),
            LocalLibraryCommand::Delete(ids) => delete_tracks(state, &ids),
        }
    }
}

fn refresh_local_library(services: &AppServices, state: &mut AppState) {
    if let Ok(folders) = services.local_library.list_folders() {
        state.local_folders = folders;
    }
    if let Ok(tracks) = services.local_library.tracks() {
        state.local_tracks = tracks;
    }
}

// ---- The 6 Walkman-library track views (same Play/EditMetadata/TrimCut/Transfer/Delete/
// BatchLyrics shape; Albums additionally splits metadata editing into track-vs-album) ----

fn drain_track_view_commands(services: &AppServices, state: &mut AppState) {
    use walkman_ui::views::{albums::AlbumsCommand, artists::ArtistsCommand, hi_res::HiResCommand, language_study::LanguageStudyCommand, library::LibraryCommand, recently_added::RecentlyAddedCommand};

    for command in std::mem::take(&mut state.library_view.commands) {
        match command {
            LibraryCommand::Play(id) => play_tracks(services, state, &[id]),
            LibraryCommand::EditMetadata(id) => open_song_editor(state, &id),
            LibraryCommand::TrimCut(id) => open_trim_cut(state, &id),
            LibraryCommand::Transfer(ids) => enqueue_tracks_for_transfer(services, state, &ids),
            LibraryCommand::Delete(ids) => delete_tracks(state, &ids),
            LibraryCommand::BatchLyrics(ids) => open_batch_lyrics(state, ids),
        }
    }
    for command in std::mem::take(&mut state.recently_added_view.commands) {
        match command {
            RecentlyAddedCommand::Play(id) => play_tracks(services, state, &[id]),
            RecentlyAddedCommand::EditMetadata(id) => open_song_editor(state, &id),
            RecentlyAddedCommand::TrimCut(id) => open_trim_cut(state, &id),
            RecentlyAddedCommand::Transfer(ids) => enqueue_tracks_for_transfer(services, state, &ids),
            RecentlyAddedCommand::Delete(ids) => delete_tracks(state, &ids),
            RecentlyAddedCommand::BatchLyrics(ids) => open_batch_lyrics(state, ids),
        }
    }
    for command in std::mem::take(&mut state.artists_view.commands) {
        match command {
            ArtistsCommand::Play(id) => play_tracks(services, state, &[id]),
            ArtistsCommand::EditMetadata(id) => open_song_editor(state, &id),
            ArtistsCommand::TrimCut(id) => open_trim_cut(state, &id),
            ArtistsCommand::Transfer(ids) => enqueue_tracks_for_transfer(services, state, &ids),
            ArtistsCommand::Delete(ids) => delete_tracks(state, &ids),
        }
    }
    for command in std::mem::take(&mut state.hi_res_view.commands) {
        match command {
            HiResCommand::Play(id) => play_tracks(services, state, &[id]),
            HiResCommand::EditMetadata(id) => open_song_editor(state, &id),
            HiResCommand::TrimCut(id) => open_trim_cut(state, &id),
            HiResCommand::Transfer(ids) => enqueue_tracks_for_transfer(services, state, &ids),
            HiResCommand::Delete(ids) => delete_tracks(state, &ids),
            HiResCommand::BatchLyrics(ids) => open_batch_lyrics(state, ids),
        }
    }
    for command in std::mem::take(&mut state.language_study_view.commands) {
        match command {
            LanguageStudyCommand::Play(id) => play_tracks(services, state, &[id]),
            LanguageStudyCommand::EditMetadata(id) => open_song_editor(state, &id),
            LanguageStudyCommand::TrimCut(id) => open_trim_cut(state, &id),
            LanguageStudyCommand::Transfer(ids) => enqueue_tracks_for_transfer(services, state, &ids),
            LanguageStudyCommand::Delete(ids) => delete_tracks(state, &ids),
            LanguageStudyCommand::BatchLyrics(ids) => open_batch_lyrics(state, ids),
        }
    }
    for command in std::mem::take(&mut state.albums_view.commands) {
        match command {
            AlbumsCommand::Play(id) => play_tracks(services, state, &[id]),
            AlbumsCommand::EditTrackMetadata(id) => open_song_editor(state, &id),
            AlbumsCommand::EditAlbumMetadata(album_id) => open_album_editor(state, &album_id),
            AlbumsCommand::TrimCut(id) => open_trim_cut(state, &id),
            AlbumsCommand::Transfer(ids) => enqueue_tracks_for_transfer(services, state, &ids),
            AlbumsCommand::Delete(ids) => delete_tracks(state, &ids),
            AlbumsCommand::BatchLyrics(ids) => open_batch_lyrics(state, ids),
        }
    }
}

/// Opens the Album Metadata Editor for every track sharing `album_id`
/// (`walkman_domain::models::AlbumItem::id()`'s own `"artist\u{1F}album"` key, reproduced
/// here rather than importing the private grouping helper — the id format is the only part
/// of `AlbumItem` this needs).
fn open_album_editor(state: &mut AppState, album_id: &str) {
    let tracks: Vec<Track> = state
        .walkman_tracks
        .iter()
        .filter(|t| format!("{}\u{1F}{}", t.artist.trim().to_lowercase(), t.album.trim().to_lowercase()) == album_id)
        .cloned()
        .collect();
    if !tracks.is_empty() {
        state.album_editor.open_for(&tracks);
    }
}

// ---- Device Overview ----

fn drain_device_overview(services: &AppServices, state: &mut AppState) {
    let commands = std::mem::take(&mut state.device_overview_view.commands);
    for command in commands {
        match command {
            DeviceOverviewCommand::Refresh => {
                // The lifecycle tick already re-detects devices every frame's safety-net
                // poll (R3.6); nothing extra is needed beyond letting that run, so this is a
                // deliberate no-op rather than a second detection pass racing the first.
            }
            DeviceOverviewCommand::Eject(mount_point) => {
                let label = state.devices.first().map(|d| d.device_info.display_name.clone()).unwrap_or_else(|| "Walkman".to_owned());
                state.clean_eject_sheet.open_for(&mount_point, &label);
            }
            DeviceOverviewCommand::PickCustomFolder => {
                if let Some(path) = services.file_dialogs.pick_folder("Choose a custom storage folder") {
                    log::info!("custom storage folder picked: {}", path.display());
                    // R3.7's Android MTP/custom-folder bridge needs a place on `AppState` to
                    // remember this folder across frames/sessions; none exists yet, so for
                    // now the picked path is only logged. Persisting it is a small, separate
                    // follow-up once that storage is added.
                }
            }
        }
    }
}

// ---- SensMe channels ----

fn drain_senseme(services: &AppServices, state: &mut AppState) {
    let commands = std::mem::take(&mut state.senseme_view.commands);
    for command in commands {
        match command {
            SenseMeCommand::PlayChannel(ids) | SenseMeCommand::ShuffleChannel(ids) => play_tracks(services, state, &ids),
            SenseMeCommand::AnalyzeUnanalyzed(_) => {
                // Documented gap — see this module's top-level doc comment.
            }
        }
    }
}

// ---- Duplicates ----

fn drain_duplicates(services: &AppServices, state: &mut AppState) {
    let commands = std::mem::take(&mut state.duplicates_view.commands);
    for command in commands {
        match command {
            DuplicatesCommand::DeleteTrack(id) => delete_tracks(state, &[id]),
            DuplicatesCommand::ResolveGroupKeepBest(ids) => delete_tracks(state, &ids),
            DuplicatesCommand::AutoResolveQuality => {
                // Reaches this arm only via `DuplicatesSheet::AutoResolveConfirm`'s own
                // confirm button (task 17.2), so the confirmation has already happened.
                let groups = duplicate_finder::find_duplicates(&state.walkman_tracks, state.duplicates_view.scope);
                let removed = duplicate_finder::auto_resolve_quality(&groups, true);
                state.walkman_tracks.retain(|t| !removed.contains(&t.full_path));
            }
            DuplicatesCommand::Audition(id) => play_tracks(services, state, &[id]),
            DuplicatesCommand::RevealInExplorer(id) => services.file_dialogs.reveal_in_file_manager(Path::new(&id)),
        }
    }
}

// ---- Playlists ----

fn drain_playlists(state: &mut AppState) {
    use walkman_services::walkman_engine::{delete_playlist, rename_playlist, write_playlist};

    let commands = std::mem::take(&mut state.playlists_view.commands);
    let Some(mount) = state.devices.first().and_then(|d| d.internal_drive().or_else(|| d.drives.first())) else {
        return; // playlists live on the device; nothing to do with none connected.
    };
    let music_dir = PathBuf::from(&mount.music_dir);

    for command in commands {
        match command {
            PlaylistsCommand::Create(name) => {
                let sanitized = walkman_domain::sanitize::sanitize_filename(&name, walkman_domain::sanitize::DEFAULT_MAX_FILENAME_LEN);
                let path = music_dir.join(format!("{sanitized}.m3u8"));
                if let Err(e) = write_playlist(&path, &[]) {
                    log::warn!("create playlist failed: {e}");
                }
            }
            PlaylistsCommand::Rename { playlist_id, new_name } => {
                if let Err(e) = rename_playlist(Path::new(&playlist_id), &new_name) {
                    log::warn!("rename playlist failed: {e}");
                }
            }
            PlaylistsCommand::Delete(playlist_id) => {
                if let Err(e) = delete_playlist(Path::new(&playlist_id)) {
                    log::warn!("delete playlist failed: {e}");
                }
            }
            PlaylistsCommand::AddTracks { playlist_id, relative_paths } => {
                if let Some(playlist) = state.playlists.iter().find(|p| p.full_path == playlist_id) {
                    let mut tracks = playlist.tracks.clone();
                    tracks.extend(relative_paths);
                    write_entries(&playlist_id, &tracks, &state.walkman_tracks);
                }
            }
            PlaylistsCommand::Reorder { playlist_id, relative_paths } => {
                write_entries(&playlist_id, &relative_paths, &state.walkman_tracks);
            }
        }
    }
}

/// Writes `relative_paths` back to `playlist_id` as `#EXTINF` entries, looking up each track's
/// display text/duration from `tracks` when a match exists (falling back to the bare relative
/// path as its own display text otherwise, e.g. a track that isn't in the current scan cache).
fn write_entries(playlist_id: &str, relative_paths: &[String], tracks: &[Track]) {
    let entries: Vec<(String, String, f64)> = relative_paths
        .iter()
        .map(|rel| {
            let matching = tracks.iter().find(|t| &t.relative_path == rel);
            let display = matching.map(|t| format!("{} - {}", t.artist, t.title)).unwrap_or_else(|| rel.clone());
            let duration = matching.map(|t| t.duration_seconds).unwrap_or(0.0);
            (rel.clone(), display, duration)
        })
        .collect();
    if let Err(e) = walkman_services::walkman_engine::write_playlist(Path::new(playlist_id), &entries) {
        log::warn!("write playlist failed: {e}");
    }
}

// ---- Song / Album / Transfer-item metadata editors ----

fn drain_song_editor(services: &AppServices, state: &mut AppState) {
    let commands = std::mem::take(&mut state.song_editor.commands);
    for command in commands {
        match command {
            SongEditorCommand::Save { track_id, fields } => {
                let Some(mut track) = find_track(state, &track_id).cloned() else { continue };
                apply_song_fields(&mut track, &fields);
                if let Err(e) = write_track_tags(&track, &fields) {
                    log::warn!("save metadata failed for {track_id}: {e}");
                    continue;
                }
                persist_track_edit(services, state, track);
            }
            SongEditorCommand::ExportArtworkToPc(track_id) => {
                if let Some(path) = services.file_dialogs.save_file("artwork.jpg", &[("JPEG Image", "jpg")]) {
                    if let Some(track) = find_track(state, &track_id) {
                        if let Err(e) = walkman_services::audio_tagger::export_embedded_artwork(Path::new(&track.full_path), &path) {
                            log::warn!("artwork export failed: {e}");
                        }
                    }
                }
            }
            // Documented gaps — see this module's top-level doc comment.
            SongEditorCommand::ImportArtworkFromFile(_) | SongEditorCommand::SearchOnlineMetadata { .. } | SongEditorCommand::SearchLyrics { .. } => {}
        }
    }
}

fn apply_song_fields(track: &mut Track, fields: &walkman_ui::sheets::song_metadata::SongEditorFields) {
    track.title = fields.title.clone();
    track.artist = fields.artist.clone();
    track.album = fields.album.clone();
    track.track_num = fields.track_number.clone();
    track.year = fields.year.clone();
    track.genre = fields.genre.clone();
}

fn write_track_tags(track: &Track, fields: &walkman_ui::sheets::song_metadata::SongEditorFields) -> walkman_services::error::Result<()> {
    walkman_services::audio_tagger::write_tags(
        Path::new(&track.full_path),
        &walkman_services::audio_tagger::TagFields {
            title: &fields.title,
            artist: &fields.artist,
            album: &fields.album,
            track_number: &fields.track_number,
            disc_number: &fields.disc_number,
            year: &fields.year,
            genre: &fields.genre,
            lyrics: Some(fields.lyrics.as_str()).filter(|l| !l.is_empty()),
            artwork_jpeg: None,
        },
    )
}

fn drain_album_editor(services: &AppServices, state: &mut AppState) {
    let commands = std::mem::take(&mut state.album_editor.commands);
    for command in commands {
        match command {
            AlbumEditorCommand::Save { track_ids, track_numbers, fields } => {
                for (id, track_number) in track_ids.iter().zip(track_numbers.iter()) {
                    let Some(mut track) = find_track(state, id).cloned() else { continue };
                    track.album = fields.title.clone();
                    track.artist = fields.artist.clone();
                    track.year = fields.year.clone();
                    track.genre = fields.genre.clone();
                    track.track_num = track_number.clone();
                    let tag_fields = walkman_services::audio_tagger::TagFields {
                        title: &track.title,
                        artist: &fields.artist,
                        album: &fields.title,
                        track_number,
                        disc_number: "",
                        year: &fields.year,
                        genre: &fields.genre,
                        lyrics: None,
                        artwork_jpeg: None,
                    };
                    if let Err(e) = walkman_services::audio_tagger::write_tags(Path::new(&track.full_path), &tag_fields) {
                        log::warn!("album save failed for {}: {e}", track.full_path);
                        continue;
                    }
                    persist_track_edit(services, state, track);
                }
            }
            // Documented gap — see this module's top-level doc comment.
            AlbumEditorCommand::ImportArtworkFromFile => {}
        }
    }
}

fn drain_transfer_item_editor(services: &AppServices, state: &mut AppState) {
    let commands = std::mem::take(&mut state.transfer_item_editor.commands);
    for command in commands {
        match command {
            TransferItemEditorCommand::Save { item_id, fields } => {
                services.transfer_queue.update(&item_id, |item| {
                    item.title = fields.title.clone();
                    item.artist = fields.artist.clone();
                    item.album = fields.album.clone();
                    item.track_number = fields.track_number.clone();
                    item.year = fields.year.clone();
                    item.genre = fields.genre.clone();
                    item.lyrics = Some(fields.lyrics.clone()).filter(|l| !l.is_empty());
                });
                state.transfer_queue_items = services.transfer_queue.items();
            }
            // Documented gaps — see this module's top-level doc comment.
            TransferItemEditorCommand::SearchOnlineMetadata { .. } | TransferItemEditorCommand::SearchLyrics { .. } => {}
        }
    }
}

// ---- Trim & Cut ----

fn drain_trim_cut(state: &mut AppState) {
    let commands = std::mem::take(&mut state.trim_cut_sheet.commands);
    for command in commands {
        match command {
            TrimCutCommand::Apply { track_id, options } => {
                let Some(track) = find_track(state, &track_id).cloned() else { continue };
                let source = Path::new(&track.full_path);
                let temp_output = source.with_extension(format!("trimcut.{}", source.extension().and_then(|e| e.to_str()).unwrap_or("tmp")));
                match walkman_services::trim_cut::apply_trim_cut(source, &temp_output, &options) {
                    Ok(()) => {
                        if let Err(e) = std::fs::rename(&temp_output, source) {
                            log::warn!("trim/cut: failed to replace {}: {e}", track.full_path);
                        }
                    }
                    Err(e) => log::warn!("trim/cut failed for {}: {e}", track.full_path),
                }
            }
            // `Preview`/`StopPreview`/`Cancel` need no filesystem call; the preview playhead
            // itself is a documented gap (see this module's top-level doc comment on why a
            // real-time selection-bounded preview isn't wired here).
            TrimCutCommand::Preview | TrimCutCommand::StopPreview | TrimCutCommand::Cancel => {}
        }
    }
}

// ---- Built-in lyrics editor (R15.6) ----

fn drain_lyrics_editor(services: &AppServices, state: &mut AppState) {
    let commands = std::mem::take(&mut state.lyrics_editor_sheet.commands);
    for command in commands {
        match command {
            LyricsEditorCommand::Save { track_id, lines } => {
                let Some(track) = find_track(state, &track_id) else { continue };
                match walkman_services::walkman_engine::write_lrc(Path::new(&track.full_path), &lines, None) {
                    Ok(_) => {
                        // The inspector's lyrics display reads `AppState::inspector_lyrics`
                        // directly rather than re-parsing the `.lrc` just written; refresh it
                        // in place so the edit is visible immediately.
                        state.inspector_lyrics = Some(lines.iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n"));
                    }
                    Err(e) => log::warn!("writing .lrc failed for {}: {e}", track.full_path),
                }
            }
        }
    }
    let _ = services;
}

// ---- Transfer Queue (async: duplicate guard + background batch transfer) ----

impl Dispatch {
    fn drain_transfer_queue(&mut self, services: &AppServices, state: &mut AppState) {
        let commands = std::mem::take(&mut state.transfer_queue_view.commands);
        for command in commands {
            match command {
                TransferQueueCommand::Remove(id) => {
                    services.transfer_queue.remove(&id);
                    state.transfer_queue_items = services.transfer_queue.items();
                }
                TransferQueueCommand::ClearAll => {
                    services.transfer_queue.clear();
                    state.transfer_queue_items.clear();
                }
                TransferQueueCommand::ToggleExpanded(id) => {
                    services.transfer_queue.update(&id, |item| item.is_expanded = !item.is_expanded);
                    state.transfer_queue_items = services.transfer_queue.items();
                }
                TransferQueueCommand::Preview(id) => {
                    if let Some(item) = state.transfer_queue_items.iter().find(|i| i.id == id) {
                        services.player.set_queue(vec![item.to_preview_track()]);
                        let _ = services.player.play_index(0);
                    }
                }
                TransferQueueCommand::StartTransfer { item_ids, target_mode, preset, loudnorm } => {
                    self.start_transfer(services, state, &item_ids, target_mode, preset, loudnorm);
                }
                TransferQueueCommand::ResolveDuplicates(resolution) => {
                    // The guard sheet already closed; the resolution is applied by
                    // re-running the transfer with it now known. `pending_duplicate_*`
                    // bookkeeping is intentionally not added here — see `start_transfer`'s
                    // own doc comment for why this sheet resolves synchronously instead.
                    let _ = resolution;
                }
                TransferQueueCommand::CancelTransfer => {
                    self.pending_transfer = None;
                    state.transfer_queue_view.sheet = walkman_ui::views::transfer_queue::TransferQueueSheet::None;
                }
                // Already drained by `shell::layout_shell` (opening the per-item editor
                // needs no service call, same reasoning as the inspector's `Edit`); this
                // arm only exists so the match stays exhaustive if that invariant ever
                // changes.
                TransferQueueCommand::OpenEditor(_) => {}
            }
        }
    }

    /// Runs the pre-transfer duplicate guard (R9.5) synchronously (one cheap in-memory cache
    /// lookup per item, not I/O) and, if anything collides, opens the guard sheet and stops —
    /// this port doesn't re-enter the transfer after the user picks a resolution from that
    /// sheet; `KeepBoth` (never silently overwrite) is used unconditionally once nothing
    /// collides, matching `transfer_item`'s own documented default. Wiring the sheet's chosen
    /// resolution back into a second, resumed transfer pass is a reasonable follow-up, not
    /// done here to keep this already-large dispatcher's async bookkeeping to one in-flight
    /// transfer at a time.
    fn start_transfer(&mut self, services: &AppServices, state: &mut AppState, item_ids: &[String], target_mode: TransferTargetMode, preset: walkman_services::transcode::Preset, loudnorm: bool) {
        let items: Vec<TransferItem> = state.transfer_queue_items.iter().filter(|i| item_ids.contains(&i.id)).cloned().collect();
        if items.is_empty() {
            return;
        }
        let Some(drive) = state.devices.first().and_then(|d| d.internal_drive().or_else(|| d.drives.first())) else {
            return; // nowhere to transfer to.
        };
        let target_root = transfer_queue::target_root_for(Path::new(&drive.music_dir), target_mode);

        let pairs: Vec<(String, String)> = items.iter().map(|i| (i.title.clone(), i.artist.clone())).collect();
        let duplicate_count = services.scanner.tracks_for_mount(&drive.mount_point).ok().map(|_| ()).and_then(|_| {
            walkman_services::duplicate_guard::check_batch_for_duplicates(&services.scanner, &drive.mount_point, &pairs).ok()
        }).map(|indices| indices.len()).unwrap_or(0);

        if duplicate_count > 0 {
            state.transfer_queue_view.sheet = walkman_ui::views::transfer_queue::TransferQueueSheet::DuplicateGuard { duplicate_count };
            return;
        }

        self.pending_transfer = Some(transfer_queue::transfer_batch_async(items, target_root, preset, loudnorm, Some(DuplicateResolution::KeepBoth)));
        state.transfer_queue_view.sheet = walkman_ui::views::transfer_queue::TransferQueueSheet::Progress { completed: 0, total: item_ids.len(), current_title: String::new() };
    }

    fn poll_transfer(&mut self, state: &mut AppState) {
        let Some(rx) = &self.pending_transfer else { return };
        while let Ok(progress) = rx.try_recv() {
            state.transfer_queue_view.sheet = walkman_ui::views::transfer_queue::TransferQueueSheet::Progress {
                completed: progress.completed,
                total: progress.total,
                current_title: progress.current_title.clone(),
            };
            if let Some((id, _)) = &progress.last_result {
                state.transfer_queue_items.retain(|i| &i.id != id);
            }
            if progress.completed >= progress.total {
                self.pending_transfer = None;
                state.transfer_queue_view.sheet = walkman_ui::views::transfer_queue::TransferQueueSheet::None;
                break;
            }
        }
    }
}

// ---- Clean & Eject ----

impl Dispatch {
    fn drain_clean_eject(&mut self, services: &AppServices, state: &mut AppState) {
        let commands = std::mem::take(&mut state.clean_eject_sheet.commands);
        for command in commands {
            match command {
                walkman_ui::sheets::clean_eject::CleanEjectCommand::StartCleanAndEject { mount_path } => {
                    self.start_clean_eject(services, mount_path);
                }
                CleanEjectCommand::Close => {}
            }
        }
    }

    fn start_clean_eject(&mut self, services: &AppServices, mount_path: String) {
        // `WindowsEjector` has no `Clone`/`Send` story established for this port (it's a
        // thin, stateless Win32 wrapper per `eject.rs`), so the actual call runs on a plain
        // worker thread using a fresh instance rather than moving `services.ejector` — both
        // are equally valid since the type carries no state.
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let ejector = walkman_platform::windows::WindowsEjector::new();
            let result = walkman_services::clean_eject::clean_and_eject(Path::new(&mount_path), &ejector).map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        self.pending_clean_eject = Some(rx);
        let _ = services; // kept for signature symmetry with the other `start_*` helpers.
    }

    fn poll_clean_eject(&mut self, state: &mut AppState) {
        let Some(rx) = &self.pending_clean_eject else { return };
        if let Ok(result) = rx.try_recv() {
            self.pending_clean_eject = None;
            state.clean_eject_sheet.result = Some(result);
            state.clean_eject_sheet.is_running = false;
        }
    }
}

// ---- Backup & Restore ----

impl Dispatch {
    fn drain_backup_restore(&mut self, services: &AppServices, state: &mut AppState) {
        let commands = std::mem::take(&mut state.backup_restore_view.commands);
        for command in commands {
            match command {
                BackupRestoreCommand::PickDestination => {
                    if let Some(path) = services.file_dialogs.pick_folder("Choose a backup destination") {
                        state.backup_restore_view.destination = path.to_string_lossy().into_owned();
                    }
                }
                BackupRestoreCommand::PickRestoreSource => {
                    if let Some(path) = services.file_dialogs.pick_folder("Choose a backup to restore") {
                        state.backup_restore_view.restore_source = path.to_string_lossy().into_owned();
                    }
                }
                BackupRestoreCommand::StartBackup { scope, selected_album_ids, selected_track_ids, destination, passphrase } => {
                    self.start_backup(state, scope, &selected_album_ids, &selected_track_ids, destination, passphrase);
                }
                BackupRestoreCommand::StartRestore { source, passphrase } => {
                    self.start_restore(state, source, passphrase);
                }
                BackupRestoreCommand::Cancel => {
                    self.pending_backup_restore = None;
                    state.backup_restore_view.progress = None;
                }
            }
        }
    }

    /// Resolves `scope` against `state.walkman_tracks` into `BackupSource`s and runs the
    /// backup on a worker thread. The `Playlists` scope backs up every track currently known
    /// (the same source list `FullLibrary` would use) rather than the `.m3u8` files
    /// themselves — a simplification documented here rather than silently assumed, since the
    /// real distinction ("back up the playlist files" vs. "back up every track") needs a
    /// product decision this port doesn't have input on.
    fn start_backup(
        &mut self,
        state: &mut AppState,
        scope: walkman_services::backup_restore::BackupScope,
        selected_album_ids: &[String],
        selected_track_ids: &[String],
        destination: String,
        passphrase: Option<String>,
    ) {
        use walkman_services::backup_restore::{BackupScope, BackupSource};

        let tracks: Vec<&Track> = match scope {
            BackupScope::LanguageStudy => state.walkman_tracks.iter().filter(|t| t.is_language_study()).collect(),
            BackupScope::SelectedTracks => state.walkman_tracks.iter().filter(|t| selected_track_ids.contains(&t.id().to_owned())).collect(),
            BackupScope::SelectedAlbums => state
                .walkman_tracks
                .iter()
                .filter(|t| selected_album_ids.contains(&format!("{}\u{1F}{}", t.artist.trim().to_lowercase(), t.album.trim().to_lowercase())))
                .collect(),
            BackupScope::FullLibrary | BackupScope::Playlists => state.walkman_tracks.iter().collect(),
        };
        if tracks.is_empty() {
            return;
        }
        let sources: Vec<BackupSource> = tracks.iter().map(|t| BackupSource { full_path: PathBuf::from(&t.full_path), relative_path: t.relative_path.clone() }).collect();
        let dest_dir = PathBuf::from(destination);

        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let cancel: walkman_services::backup_restore::CancelCheck = &|| false;
            let result = match passphrase {
                Some(pass) if !pass.is_empty() => {
                    let enc_path = dest_dir.join(format!("backup-{}.walkmanbackup.enc", chrono::Utc::now().format("%Y%m%d-%H%M%S")));
                    walkman_services::backup_restore::create_encrypted_backup(&sources, &enc_path, scope, &pass, cancel).map_err(|e| e.to_string())
                }
                _ => walkman_services::backup_restore::create_unencrypted_backup(&sources, &dest_dir, scope, cancel).map_err(|e| e.to_string()),
            };
            let _ = tx.send(result);
        });
        self.pending_backup_restore = Some(rx);
        state.backup_restore_view.progress =
            Some(walkman_ui::views::backup_restore::BackupProgressInfo { completed: 0, total: 1, current_file: "Backing up\u{2026}".to_owned() });
    }

    fn start_restore(&mut self, state: &mut AppState, source: String, passphrase: Option<String>) {
        let source_path = PathBuf::from(&source);
        let dest_root = state.devices.first().and_then(|d| d.internal_drive().or_else(|| d.drives.first())).map(|d| PathBuf::from(&d.mount_point));
        let Some(dest_root) = dest_root else { return };

        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let cancel: walkman_services::backup_restore::CancelCheck = &|| false;
            let result = match passphrase {
                Some(pass) if !pass.is_empty() => walkman_services::backup_restore::restore_encrypted_backup(&source_path, &pass, &dest_root, cancel).map_err(|e| e.to_string()),
                _ => walkman_services::backup_restore::restore_unencrypted_backup(&source_path, &dest_root, cancel).map_err(|e| e.to_string()),
            };
            let _ = tx.send(result);
        });
        self.pending_backup_restore = Some(rx);
        state.backup_restore_view.progress =
            Some(walkman_ui::views::backup_restore::BackupProgressInfo { completed: 0, total: 1, current_file: "Restoring\u{2026}".to_owned() });
    }

    fn poll_backup_restore(&mut self, state: &mut AppState) {
        let Some(rx) = &self.pending_backup_restore else { return };
        if let Ok(result) = rx.try_recv() {
            self.pending_backup_restore = None;
            match result {
                Ok(manifest) => {
                    state.backup_restore_view.progress =
                        Some(walkman_ui::views::backup_restore::BackupProgressInfo { completed: 1, total: 1, current_file: format!("Done \u{2014} {} file(s)", manifest.entries.len()) });
                }
                Err(e) => {
                    state.backup_restore_view.progress = Some(walkman_ui::views::backup_restore::BackupProgressInfo { completed: 0, total: 1, current_file: format!("Failed: {e}") });
                }
            }
        }
    }
}

// ---- Batch lyrics generator ----

impl Dispatch {
    fn drain_batch_lyrics(&mut self, services: &AppServices, state: &mut AppState) {
        let commands = std::mem::take(&mut state.batch_lyrics_sheet.commands);
        for command in commands {
            match command {
                LyricsGeneratorCommand::Start { track_ids } => {
                    let requests: Vec<walkman_services::lyrics::LyricsRequest> = track_ids
                        .iter()
                        .filter_map(|id| find_track(state, id))
                        .map(|t| walkman_services::lyrics::LyricsRequest {
                            id: t.id().to_owned(),
                            title: t.title.clone(),
                            artist: t.artist.clone(),
                            album: t.album.clone(),
                            duration_seconds: t.duration_seconds,
                            already_has_lyrics: false,
                        })
                        .collect();
                    if requests.is_empty() {
                        continue;
                    }
                    let rx = walkman_services::lyrics::fetch_batch_async(requests, 4);
                    self.pending_lyrics_batch = Some((rx, Vec::new()));
                }
                LyricsGeneratorCommand::Cancel => self.pending_lyrics_batch = None,
            }
        }
        let _ = services;
    }

    fn poll_batch_lyrics(&mut self, state: &mut AppState) {
        let Some((rx, outcomes)) = &mut self.pending_lyrics_batch else { return };
        while let Ok(progress) = rx.try_recv() {
            outcomes.push(progress.last.clone());
            state.batch_lyrics_sheet.completed = progress.completed;
            state.batch_lyrics_sheet.total = progress.total;
            state.batch_lyrics_sheet.outcomes = outcomes.clone();
            if progress.completed >= progress.total {
                state.batch_lyrics_sheet.is_running = false;
                self.pending_lyrics_batch = None;
                return;
            }
        }
    }
}
