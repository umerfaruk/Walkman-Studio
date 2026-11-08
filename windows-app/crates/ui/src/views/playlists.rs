//! Pure types/logic behind `PlaylistsView`, ported from
//! `Sources/WalkmanStudio/Views/PlaylistsView.swift`. The rendering itself lives in
//! `iced_ui::views::playlists` (the egui `render` this module used to carry was removed
//! once the iced migration reached parity — see Milestone 8 of the egui-to-iced
//! migration plan).

use walkman_domain::models::{Playlist, Track};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AddTracksMode {
    #[default]
    Songs,
    RecentlyAdded,
    Albums,
    Artists,
}

impl AddTracksMode {
    pub const ALL: [AddTracksMode; 4] = [AddTracksMode::Songs, AddTracksMode::RecentlyAdded, AddTracksMode::Albums, AddTracksMode::Artists];

    pub fn title(self) -> &'static str {
        match self {
            AddTracksMode::Songs => "Songs",
            AddTracksMode::RecentlyAdded => "Recently Added",
            AddTracksMode::Albums => "Albums",
            AddTracksMode::Artists => "Artists",
        }
    }
}

/// Tracks for `mode`'s add-browser list, pure so the mode-filtering is unit-testable without
/// a live `Ui`. `Albums`/`Artists` just sort by their grouping key (a full drill-down browser
/// inside this already-nested sheet was judged more complexity than the requirement's own
/// wording, "the other add-browsing modes," demands).
pub fn tracks_for_add_mode<'a>(tracks: &'a [Track], mode: AddTracksMode) -> Vec<&'a Track> {
    let mut sorted: Vec<&Track> = tracks.iter().collect();
    match mode {
        AddTracksMode::Songs => sorted.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase())),
        AddTracksMode::RecentlyAdded => sorted.sort_by(|a, b| b.date_added.cmp(&a.date_added)),
        AddTracksMode::Albums => sorted.sort_by(|a, b| a.album.to_lowercase().cmp(&b.album.to_lowercase())),
        AddTracksMode::Artists => sorted.sort_by(|a, b| a.artist.to_lowercase().cmp(&b.artist.to_lowercase())),
    }
    sorted
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum PlaylistsSheet {
    #[default]
    None,
    CreatePlaylist { name: String },
    RenamePlaylist { playlist_id: String, name: String },
    DeleteConfirm { playlist_id: String },
    AddTracks { mode: AddTracksMode, search: String, selected: Vec<String> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlaylistsCommand {
    Create(String),
    Rename { playlist_id: String, new_name: String },
    Delete(String),
    /// Appends `relative_paths` to the end of the active playlist.
    AddTracks { playlist_id: String, relative_paths: Vec<String> },
    /// Replaces the active playlist's full track list, in the given order (also used for
    /// single/batch removal — the caller just omits the removed entries) — R11.1/11.3's
    /// "syncing instantly after each change".
    Reorder { playlist_id: String, relative_paths: Vec<String> },
}

#[derive(Debug, Clone, Default)]
pub struct PlaylistsViewState {
    pub selected_playlist_id: Option<String>,
    /// A working copy of the selected playlist's track list, reordered/trimmed locally
    /// before each change is emitted as a [`PlaylistsCommand::Reorder`]. Re-seeded from the
    /// real `Playlist::tracks` whenever `selected_playlist_id` changes (tracked via
    /// `loaded_for`, below) so an external update (e.g. another device writing the file) is
    /// picked up on next selection rather than silently overwritten.
    pub active_tracks: Vec<String>,
    loaded_for: Option<String>,
    pub selected_rows: Vec<String>,
    pub selection_anchor: Option<usize>,
    pub sheet: PlaylistsSheet,
    pub commands: Vec<PlaylistsCommand>,
}

/// Re-seeds `active_tracks` from the real `Playlist` whenever the selection changes to a
/// different playlist (or the selection is cleared), so edits made via the batch bar aren't
/// clobbered on every call but a freshly-opened playlist always starts from its real list.
/// Called by both the egui-era caller (historically, every frame) and the iced port (on the
/// message that actually changes the selection) — either way it's a no-op once the working
/// copy for the current selection is already loaded.
pub fn sync_active_tracks(playlists: &[Playlist], state: &mut PlaylistsViewState) {
    if state.loaded_for == state.selected_playlist_id {
        return;
    }
    state.loaded_for = state.selected_playlist_id.clone();
    state.active_tracks = state
        .selected_playlist_id
        .as_deref()
        .and_then(|id| playlists.iter().find(|p| p.full_path == id))
        .map(|p| p.tracks.clone())
        .unwrap_or_default();
    state.selected_rows.clear();
    state.selection_anchor = None;
}

/// Moves every selected index to the front, preserving their relative order, then the
/// unselected rows in their original relative order.
pub fn move_selected_to_top(tracks: &mut Vec<String>, selected: &[usize]) {
    let selected_set: std::collections::HashSet<usize> = selected.iter().copied().collect();
    let mut moved: Vec<String> = selected.iter().filter_map(|&i| tracks.get(i).cloned()).collect();
    let rest: Vec<String> = tracks.iter().enumerate().filter(|(i, _)| !selected_set.contains(i)).map(|(_, t)| t.clone()).collect();
    moved.extend(rest);
    *tracks = moved;
}

/// Moves every selected index to the back, preserving their relative order.
pub fn move_selected_to_bottom(tracks: &mut Vec<String>, selected: &[usize]) {
    let selected_set: std::collections::HashSet<usize> = selected.iter().copied().collect();
    let mut rest: Vec<String> = tracks.iter().enumerate().filter(|(i, _)| !selected_set.contains(i)).map(|(_, t)| t.clone()).collect();
    let moved: Vec<String> = selected.iter().filter_map(|&i| tracks.get(i).cloned()).collect();
    rest.extend(moved);
    *tracks = rest;
}

/// Shifts every selected index by `delta` (`-1` up, `1` down), as a group — the whole
/// selected block moves past the single adjacent unselected row each click, matching the
/// macOS "group Up/Down shift" (R11.3), rather than each row moving independently (which
/// would let a multi-row selection interleave with unselected rows).
pub fn shift_selected(tracks: &mut Vec<String>, selected: &[usize], delta: i32) {
    if selected.is_empty() {
        return;
    }
    if delta < 0 {
        if selected[0] == 0 {
            return;
        }
        for &i in selected {
            tracks.swap(i, i - 1);
        }
    } else {
        if *selected.last().unwrap() + 1 >= tracks.len() {
            return;
        }
        for &i in selected.iter().rev() {
            tracks.swap(i, i + 1);
        }
    }
}

/// Removes every selected index (descending, so earlier removals don't shift later ones).
pub fn remove_selected(tracks: &mut Vec<String>, selected: &[usize]) {
    for &i in selected.iter().rev() {
        if i < tracks.len() {
            tracks.remove(i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_playlist(path: &str, tracks: Vec<&str>) -> Playlist {
        Playlist {
            name: path.trim_end_matches(".m3u8").to_owned(),
            filename: format!("{path}.m3u8"),
            track_count: tracks.len(),
            tracks: tracks.into_iter().map(String::from).collect(),
            full_path: format!("/MUSIC/{path}.m3u8"),
        }
    }

    fn sample_track(title: &str) -> Track {
        Track {
            filename: format!("{title}.flac"),
            relative_path: format!("Artist/Album/{title}.flac"),
            full_path: format!("D:/Music/Artist/Album/{title}.flac"),
            format: "FLAC".into(),
            title: title.into(),
            artist: "Artist".into(),
            album: "Album".into(),
            track_num: "1".into(),
            year: "2024".into(),
            genre: String::new(),
            duration_seconds: 200.0,
            bitrate_kbps: 900,
            sample_rate: 44_100,
            bits_per_sample: 16,
            has_artwork: false,
            size_bytes: 1,
            date_added: Utc::now(),
            is_senseme_analyzed: false,
            senseme_bpm: None,
            senseme_valence: None,
            senseme_arousal: None,
            senseme_channels: Vec::new(),
            senseme_highlight_sec: None,
        }
    }

    // ---- Reorder helpers ----

    #[test]
    fn move_selected_to_top_preserves_relative_order() {
        let mut tracks = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        move_selected_to_top(&mut tracks, &[1, 3]);
        assert_eq!(tracks, vec!["b", "d", "a", "c"]);
    }

    #[test]
    fn move_selected_to_bottom_preserves_relative_order() {
        let mut tracks = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        move_selected_to_bottom(&mut tracks, &[0, 2]);
        assert_eq!(tracks, vec!["b", "d", "a", "c"]);
    }

    #[test]
    fn shift_selected_up_moves_the_whole_block_and_stops_at_the_top() {
        let mut tracks = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        shift_selected(&mut tracks, &[1, 2], -1);
        assert_eq!(tracks, vec!["b", "c", "a", "d"]);

        // Already at the top: no-op.
        let mut at_top = vec!["a".into(), "b".into()];
        shift_selected(&mut at_top, &[0], -1);
        assert_eq!(at_top, vec!["a", "b"]);
    }

    #[test]
    fn shift_selected_down_moves_the_whole_block_and_stops_at_the_bottom() {
        let mut tracks = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        shift_selected(&mut tracks, &[1, 2], 1);
        assert_eq!(tracks, vec!["a", "d", "b", "c"]);

        let mut at_bottom = vec!["a".into(), "b".into()];
        shift_selected(&mut at_bottom, &[1], 1);
        assert_eq!(at_bottom, vec!["a", "b"]);
    }

    #[test]
    fn remove_selected_drops_every_selected_index() {
        let mut tracks = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        remove_selected(&mut tracks, &[0, 2]);
        assert_eq!(tracks, vec!["b", "d"]);
    }

    // ---- Add-mode sorting ----

    #[test]
    fn tracks_for_songs_mode_sorts_by_title() {
        let tracks = vec![sample_track("Zebra"), sample_track("Apple")];
        let sorted = tracks_for_add_mode(&tracks, AddTracksMode::Songs);
        assert_eq!(sorted[0].title, "Apple");
        assert_eq!(sorted[1].title, "Zebra");
    }

    // ---- active_tracks sync ----

    #[test]
    fn selecting_a_playlist_loads_its_tracks_and_clears_selection() {
        let playlists = vec![sample_playlist("Mix", vec!["a.flac", "b.flac"])];
        let mut state = PlaylistsViewState::default();
        state.selected_playlist_id = Some(playlists[0].full_path.clone());
        state.selected_rows.push("0".into());

        sync_active_tracks(&playlists, &mut state);

        assert_eq!(state.active_tracks, vec!["a.flac", "b.flac"]);
        assert!(state.selected_rows.is_empty());
    }

    #[test]
    fn sync_is_a_noop_when_the_selection_has_not_changed() {
        let playlists = vec![sample_playlist("Mix", vec!["a.flac"])];
        let mut state = PlaylistsViewState::default();
        state.selected_playlist_id = Some(playlists[0].full_path.clone());
        sync_active_tracks(&playlists, &mut state);

        // Simulate a local edit (as the batch bar would make) and confirm a second sync with
        // the same selection doesn't clobber it.
        state.active_tracks.push("b.flac".into());
        sync_active_tracks(&playlists, &mut state);
        assert_eq!(state.active_tracks, vec!["a.flac", "b.flac"]);
    }
}
