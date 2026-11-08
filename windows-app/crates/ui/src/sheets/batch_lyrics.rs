//! Pure types/logic behind `BatchLyricsGeneratorSheet`, ported from
//! `Sources/WalkmanStudio/Views/BatchLyricsGeneratorSheet.swift`. The rendering itself
//! lives in `iced_ui::sheets::batch_lyrics` (the egui `render` this module used to carry
//! was removed once the iced migration reached parity — see Milestone 8 of the
//! egui-to-iced migration plan).
//!
//! One sheet, shared by every view: each caller resolves its own `all_in_view_ids`/
//! `selected_ids`/`active_item_id` and passes them in — this sheet only turns whichever
//! scope is picked into a concrete id list via [`resolve_track_ids`].

use walkman_services::lyrics::{LyricsOutcome, LyricsOutcomeKind};

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
    /// `fetch_batch_async`'s channel — this crate has no clock dependency of its own.
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn items_per_second_is_zero_before_any_progress() {
        assert_eq!(items_per_second(0, 5.0), 0.0);
        assert_eq!(items_per_second(3, 0.0), 0.0);
        assert_eq!(items_per_second(10, 5.0), 2.0);
    }

    #[test]
    fn eta_seconds_extrapolates_the_current_rate_and_is_zero_when_done() {
        assert_eq!(eta_seconds(10, 30, 5.0), 10.0);
        assert_eq!(eta_seconds(30, 30, 5.0), 0.0);
        assert_eq!(eta_seconds(0, 30, 0.0), 0.0);
    }

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
}
