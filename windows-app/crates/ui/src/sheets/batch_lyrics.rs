//! `BatchLyricsGeneratorSheet`, ported from
//! `Sources/WalkmanStudio/Views/BatchLyricsGeneratorSheet.swift`.
//!
//! Spec task **17.6** (requirements 15.2, 15.3): launchable from All Songs, Recently Added,
//! Albums, Artists, Hi-Res Audio, Playlists, and Language Study (every one of those views
//! already emits a `BatchLyrics(ids)` command, tasks 14.1-14.7/17.3), with 4 scopes, a
//! `Choose Specific…` checklist (live search, Select/Deselect All, live counts, missing-
//! lyrics diagnostics), live progress/speed/ETA, the 4 metric pills (R15.3's `Synced .LRC`/
//! `Plain Text`/`Skipped`/`Not Found`), and a filterable results breakdown — over the real
//! `walkman_services::lyrics::{LyricsOutcome, LyricsOutcomeKind, LyricsBatchCounts, tally}`
//! built in task 12.2.
//!
//! One sheet, shared by every view: each caller resolves its own `all_in_view_ids`/
//! `selected_ids`/`active_item_id` and passes them into [`render`] — this sheet only turns
//! whichever scope is picked into a concrete id list via [`resolve_track_ids`].

use eframe::egui;

use walkman_domain::models::Track;
use walkman_services::lyrics::{LyricsBatchCounts, LyricsOutcome, LyricsOutcomeKind};

use crate::theme::{button, color, font};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LyricsGeneratorScope {
    #[default]
    AllInView,
    Selected,
    ActiveItem,
    ChooseSpecific,
}

impl LyricsGeneratorScope {
    pub const ALL: [LyricsGeneratorScope; 4] =
        [LyricsGeneratorScope::AllInView, LyricsGeneratorScope::Selected, LyricsGeneratorScope::ActiveItem, LyricsGeneratorScope::ChooseSpecific];

    pub fn title(self) -> &'static str {
        match self {
            LyricsGeneratorScope::AllInView => "All Tracks in View",
            LyricsGeneratorScope::Selected => "Selected Track(s)",
            LyricsGeneratorScope::ActiveItem => "Active Item",
            LyricsGeneratorScope::ChooseSpecific => "Choose Specific\u{2026}",
        }
    }
}

/// Which metric pill (if any) the results breakdown is filtered to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MetricFilter {
    #[default]
    All,
    Synced,
    Plain,
    Skipped,
    NotFound,
}

/// Resolves `scope` into a concrete track-id list. `all_in_view`/`selected`/`active_item`
/// are the caller's own view-state slices; `choose_specific_selected` is this sheet's own
/// checklist selection.
pub fn resolve_track_ids(
    scope: LyricsGeneratorScope,
    all_in_view: &[String],
    selected: &[String],
    active_item: Option<&str>,
    choose_specific_selected: &[String],
) -> Vec<String> {
    match scope {
        LyricsGeneratorScope::AllInView => all_in_view.to_vec(),
        LyricsGeneratorScope::Selected => selected.to_vec(),
        LyricsGeneratorScope::ActiveItem => active_item.map(|id| vec![id.to_owned()]).unwrap_or_default(),
        LyricsGeneratorScope::ChooseSpecific => choose_specific_selected.to_vec(),
    }
}

/// Items/second given `completed` items done over `elapsed_seconds`. `0.0` while nothing has
/// completed yet or no time has passed (avoids a divide-by-zero `NaN`/`inf` on the UI).
pub fn items_per_second(completed: usize, elapsed_seconds: f64) -> f64 {
    if completed == 0 || elapsed_seconds <= 0.0 {
        0.0
    } else {
        completed as f64 / elapsed_seconds
    }
}

/// Estimated seconds remaining, extrapolating the current rate. `0.0` once `completed >=
/// total` or before any rate is known.
pub fn eta_seconds(completed: usize, total: usize, elapsed_seconds: f64) -> f64 {
    let rate = items_per_second(completed, elapsed_seconds);
    if rate <= 0.0 || completed >= total {
        return 0.0;
    }
    (total - completed) as f64 / rate
}

/// The outcomes matching `filter` (`All` returns every outcome).
pub fn filter_outcomes(outcomes: &[LyricsOutcome], filter: MetricFilter) -> Vec<&LyricsOutcome> {
    outcomes
        .iter()
        .filter(|o| match filter {
            MetricFilter::All => true,
            MetricFilter::Synced => matches!(o.kind, LyricsOutcomeKind::Synced(_)),
            MetricFilter::Plain => matches!(o.kind, LyricsOutcomeKind::Plain(_)),
            MetricFilter::Skipped => o.kind == LyricsOutcomeKind::Skipped,
            MetricFilter::NotFound => o.kind == LyricsOutcomeKind::NotFound,
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub enum LyricsGeneratorCommand {
    Start { track_ids: Vec<String> },
    Cancel,
}

#[derive(Debug, Clone, Default)]
pub struct BatchLyricsGeneratorState {
    pub open: bool,
    pub scope: LyricsGeneratorScope,
    pub choose_specific_search: String,
    pub choose_specific_selected: Vec<String>,
    pub is_running: bool,
    pub completed: usize,
    pub total: usize,
    /// Wall-clock seconds since the run started, kept current by whoever drives
    /// `fetch_batch_async`'s channel (task 18) — this UI crate has no clock dependency of
    /// its own.
    pub elapsed_seconds: f64,
    pub outcomes: Vec<LyricsOutcome>,
    pub filter: MetricFilter,
    pub commands: Vec<LyricsGeneratorCommand>,
}

impl BatchLyricsGeneratorState {
    /// Opens the sheet fresh (clears any previous run's results/selection).
    pub fn open(&mut self) {
        *self = Self { open: true, ..Self::default() };
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}

/// Renders the sheet if [`BatchLyricsGeneratorState::open`]; a no-op otherwise.
///
/// `all_in_view_ids`/`selected_ids`/`active_item_id` are the opening view's own context
/// (resolved by [`resolve_track_ids`] for the first 3 scopes); `all_tracks` backs the
/// `Choose Specific…` checklist, and `missing_lyrics_ids` drives its diagnostic markers.
pub fn render(
    ctx: &egui::Context,
    all_tracks: &[Track],
    all_in_view_ids: &[String],
    selected_ids: &[String],
    active_item_id: Option<&str>,
    missing_lyrics_ids: &[String],
    state: &mut BatchLyricsGeneratorState,
) {
    if !state.open {
        return;
    }
    crate::sheets::modal(ctx, "batch_lyrics_generator_sheet", egui::vec2(640.0, 620.0), |ui| {
        ui.label(egui::RichText::new("Batch Lyrics Generator").font(font::title2()).color(color::SILVER_LIGHT));
        ui.add_space(10.0);

        if state.is_running || !state.outcomes.is_empty() {
            render_progress_and_results(ui, state);
        } else {
            render_scope_picker(ui, all_tracks, missing_lyrics_ids, state);
        }

        ui.add_space(14.0);
        render_footer(ui, all_in_view_ids, selected_ids, active_item_id, state);
    });
}

fn render_scope_picker(ui: &mut egui::Ui, all_tracks: &[Track], missing_lyrics_ids: &[String], state: &mut BatchLyricsGeneratorState) {
    ui.horizontal(|ui| {
        for scope in LyricsGeneratorScope::ALL {
            let variant = if state.scope == scope { button::Variant::Capsule } else { button::Variant::Glass };
            if ui.add(button::Button::new(scope.title(), variant)).clicked() {
                state.scope = scope;
            }
        }
    });

    if state.scope == LyricsGeneratorScope::ChooseSpecific {
        ui.add_space(10.0);
        render_choose_specific(ui, all_tracks, missing_lyrics_ids, state);
    }
}

fn render_choose_specific(ui: &mut egui::Ui, all_tracks: &[Track], missing_lyrics_ids: &[String], state: &mut BatchLyricsGeneratorState) {
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut state.choose_specific_search).hint_text("Search").desired_width(300.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let query = state.choose_specific_search.trim().to_lowercase();
            let visible_ids: Vec<String> = all_tracks
                .iter()
                .filter(|t| query.is_empty() || t.title.to_lowercase().contains(&query) || t.artist.to_lowercase().contains(&query))
                .map(|t| t.id().to_owned())
                .collect();
            let all_selected = !visible_ids.is_empty() && visible_ids.iter().all(|id| state.choose_specific_selected.contains(id));
            if ui.add(button::Button::new(if all_selected { "Deselect All" } else { "Select All" }, button::Variant::Glass)).clicked() {
                if all_selected {
                    state.choose_specific_selected.retain(|id| !visible_ids.contains(id));
                } else {
                    for id in visible_ids {
                        if !state.choose_specific_selected.contains(&id) {
                            state.choose_specific_selected.push(id);
                        }
                    }
                }
            }
        });
    });
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(format!("{} of {} selected", state.choose_specific_selected.len(), all_tracks.len()))
            .font(font::caption())
            .color(color::SILVER_MUTED),
    );
    ui.add_space(6.0);

    let query = state.choose_specific_search.trim().to_lowercase();
    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        for track in all_tracks {
            if !query.is_empty() && !track.title.to_lowercase().contains(&query) && !track.artist.to_lowercase().contains(&query) {
                continue;
            }
            let id = track.id().to_owned();
            let mut checked = state.choose_specific_selected.contains(&id);
            ui.horizontal(|ui| {
                if ui.checkbox(&mut checked, "").changed() {
                    if checked {
                        state.choose_specific_selected.push(id.clone());
                    } else {
                        state.choose_specific_selected.retain(|s| s != &id);
                    }
                }
                ui.label(egui::RichText::new(&track.title).font(font::body()).color(color::SILVER_LIGHT));
                ui.label(egui::RichText::new(&track.artist).font(font::caption()).color(color::SILVER_MUTED));
                if missing_lyrics_ids.iter().any(|m| m == &id) {
                    ui.label(egui::RichText::new("No lyrics").font(font::caption_bold()).color(color::HIRES_GOLD));
                }
            });
        }
    });
}

fn render_progress_and_results(ui: &mut egui::Ui, state: &mut BatchLyricsGeneratorState) {
    let fraction = if state.total == 0 { 0.0 } else { state.completed as f32 / state.total as f32 };
    ui.add(egui::ProgressBar::new(fraction).text(format!("{} / {}", state.completed, state.total)));
    ui.add_space(6.0);

    let speed = items_per_second(state.completed, state.elapsed_seconds);
    let eta = eta_seconds(state.completed, state.total, state.elapsed_seconds);
    ui.label(
        egui::RichText::new(format!("{speed:.1} tracks/sec \u{2022} ETA {eta:.0}s"))
            .font(font::caption())
            .color(color::SILVER_MUTED),
    );
    ui.add_space(10.0);

    let counts = walkman_services::lyrics::tally(&state.outcomes);
    render_metric_pills(ui, counts, state);
    ui.add_space(10.0);

    let filtered = filter_outcomes(&state.outcomes, state.filter);
    egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
        for outcome in filtered {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&outcome.id).font(font::caption()).color(color::SILVER_LIGHT));
                ui.label(egui::RichText::new(outcome.kind.metric_label()).font(font::caption_bold()).color(color::SILVER_MUTED));
            });
        }
    });
}

fn render_metric_pills(ui: &mut egui::Ui, counts: LyricsBatchCounts, state: &mut BatchLyricsGeneratorState) {
    ui.horizontal(|ui| {
        for (filter, label, count) in [
            (MetricFilter::All, "All", state.outcomes.len()),
            (MetricFilter::Synced, "Synced .LRC", counts.synced),
            (MetricFilter::Plain, "Plain Text", counts.plain),
            (MetricFilter::Skipped, "Skipped", counts.skipped),
            (MetricFilter::NotFound, "Not Found", counts.not_found),
        ] {
            let variant = if state.filter == filter { button::Variant::Capsule } else { button::Variant::Glass };
            if ui.add(button::Button::new(format!("{label} ({count})"), variant)).clicked() {
                state.filter = filter;
            }
        }
    });
}

fn render_footer(ui: &mut egui::Ui, all_in_view_ids: &[String], selected_ids: &[String], active_item_id: Option<&str>, state: &mut BatchLyricsGeneratorState) {
    ui.horizontal(|ui| {
        if state.is_running {
            if ui.add(button::Button::new("Cancel", button::Variant::Destructive)).clicked() {
                state.commands.push(LyricsGeneratorCommand::Cancel);
                state.is_running = false;
            }
        } else if state.outcomes.is_empty() {
            let track_ids = resolve_track_ids(state.scope, all_in_view_ids, selected_ids, active_item_id, &state.choose_specific_selected);
            if ui.add(button::Button::new("Generate", button::Variant::Prominent).enabled(!track_ids.is_empty())).clicked() {
                state.is_running = true;
                state.completed = 0;
                state.total = track_ids.len();
                state.commands.push(LyricsGeneratorCommand::Start { track_ids });
            }
            ui.add_space(8.0);
            if ui.add(button::Button::new("Cancel", button::Variant::Glass)).clicked() {
                state.close();
            }
        } else if ui.add(button::Button::new("Done", button::Variant::Prominent)).clicked() {
            state.close();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_track(id: &str) -> Track {
        Track {
            filename: id.into(),
            relative_path: id.into(),
            full_path: id.into(),
            format: "FLAC".into(),
            title: format!("Title {id}"),
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

    // ---- Scope resolution ----

    #[test]
    fn resolve_track_ids_covers_every_scope() {
        let all_in_view = vec!["a".to_string(), "b".to_string()];
        let selected = vec!["b".to_string()];
        let choose_specific = vec!["c".to_string()];

        assert_eq!(resolve_track_ids(LyricsGeneratorScope::AllInView, &all_in_view, &selected, Some("x"), &choose_specific), all_in_view);
        assert_eq!(resolve_track_ids(LyricsGeneratorScope::Selected, &all_in_view, &selected, Some("x"), &choose_specific), selected);
        assert_eq!(resolve_track_ids(LyricsGeneratorScope::ActiveItem, &all_in_view, &selected, Some("x"), &choose_specific), vec!["x".to_string()]);
        assert_eq!(resolve_track_ids(LyricsGeneratorScope::ActiveItem, &all_in_view, &selected, None, &choose_specific), Vec::<String>::new());
        assert_eq!(resolve_track_ids(LyricsGeneratorScope::ChooseSpecific, &all_in_view, &selected, Some("x"), &choose_specific), choose_specific);
    }

    // ---- Speed / ETA ----

    #[test]
    fn items_per_second_is_zero_before_any_progress() {
        assert_eq!(items_per_second(0, 5.0), 0.0);
        assert_eq!(items_per_second(3, 0.0), 0.0);
        assert_eq!(items_per_second(10, 5.0), 2.0);
    }

    #[test]
    fn eta_seconds_extrapolates_the_current_rate_and_is_zero_when_done() {
        // 10 done in 5s -> 2/s; 20 remaining -> 10s.
        assert_eq!(eta_seconds(10, 30, 5.0), 10.0);
        assert_eq!(eta_seconds(30, 30, 5.0), 0.0); // already done
        assert_eq!(eta_seconds(0, 30, 0.0), 0.0); // no rate yet
    }

    // ---- Filtering ----

    #[test]
    fn filter_outcomes_matches_each_metric() {
        let outcomes = vec![
            LyricsOutcome { id: "a".into(), kind: LyricsOutcomeKind::Synced("x".into()) },
            LyricsOutcome { id: "b".into(), kind: LyricsOutcomeKind::Plain("x".into()) },
            LyricsOutcome { id: "c".into(), kind: LyricsOutcomeKind::Skipped },
            LyricsOutcome { id: "d".into(), kind: LyricsOutcomeKind::NotFound },
        ];
        assert_eq!(filter_outcomes(&outcomes, MetricFilter::All).len(), 4);
        assert_eq!(filter_outcomes(&outcomes, MetricFilter::Synced).len(), 1);
        assert_eq!(filter_outcomes(&outcomes, MetricFilter::Plain).len(), 1);
        assert_eq!(filter_outcomes(&outcomes, MetricFilter::Skipped).len(), 1);
        assert_eq!(filter_outcomes(&outcomes, MetricFilter::NotFound).len(), 1);
    }

    // ---- open/close ----

    #[test]
    fn open_resets_a_previous_runs_results() {
        let mut state = BatchLyricsGeneratorState::default();
        state.outcomes.push(LyricsOutcome { id: "stale".into(), kind: LyricsOutcomeKind::Skipped });
        state.is_running = true;

        state.open();

        assert!(state.open);
        assert!(state.outcomes.is_empty());
        assert!(!state.is_running);
    }

    // ---- Headless render smoke test ----

    #[test]
    fn batch_lyrics_sheet_renders_in_headless_frame_for_every_state() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);

        let tracks = vec![sample_track("a"), sample_track("b")];
        let all_in_view: Vec<String> = tracks.iter().map(|t| t.id().to_owned()).collect();

        let mut closed = BatchLyricsGeneratorState::default();
        let mut picking_scope = BatchLyricsGeneratorState { open: true, ..BatchLyricsGeneratorState::default() };
        let mut choosing_specific = BatchLyricsGeneratorState { open: true, scope: LyricsGeneratorScope::ChooseSpecific, ..BatchLyricsGeneratorState::default() };
        let mut running = BatchLyricsGeneratorState { open: true, is_running: true, completed: 1, total: 2, elapsed_seconds: 1.0, ..BatchLyricsGeneratorState::default() };
        let mut finished = BatchLyricsGeneratorState {
            open: true,
            completed: 2,
            total: 2,
            outcomes: vec![
                LyricsOutcome { id: "a".into(), kind: LyricsOutcomeKind::Synced("x".into()) },
                LyricsOutcome { id: "b".into(), kind: LyricsOutcomeKind::NotFound },
            ],
            ..BatchLyricsGeneratorState::default()
        };

        for state in [&mut closed, &mut picking_scope, &mut choosing_specific, &mut running, &mut finished] {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |_ui| {});
                render(ctx, &tracks, &all_in_view, &[], None, &[], state);
            });
        }
    }
}
