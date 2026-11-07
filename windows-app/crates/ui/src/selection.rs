//! Shared multi-select logic (spec task 14.2's "shared selection"): Shift-range and Ctrl-toggle
//! over an ordered id list, reused by every table/grid view from here on (14.2–14.7) rather than
//! each reimplementing it.

/// Applies one row click to `selected`, given the full ordered `all_ids` list `index`/`id`
/// belong to. `shift`/`ctrl` are the modifier keys held during the click.
///
/// - Plain click: selects only `id`.
/// - Ctrl/Cmd-click: toggles `id`'s membership, leaving the rest of the selection alone.
/// - Shift-click: selects the contiguous range between the last plain/ctrl anchor and `index`
///   (replacing the current selection), matching the common "range select" convention. With no
///   prior anchor, it behaves like a plain click.
///
/// Updates `*anchor` to `index` after a plain or Ctrl click (not after a Shift-range select, so
/// a second Shift-click extends/shrinks from the *same* anchor, matching most desktop file
/// managers).
pub fn handle_click(
    selected: &mut Vec<String>,
    anchor: &mut Option<usize>,
    all_ids: &[String],
    index: usize,
    id: &str,
    shift: bool,
    ctrl: bool,
) {
    if shift {
        if let Some(a) = *anchor {
            let (lo, hi) = (a.min(index), a.max(index));
            selected.clear();
            selected.extend(all_ids[lo..=hi].iter().cloned());
        } else {
            selected.clear();
            selected.push(id.to_string());
            *anchor = Some(index);
        }
        return;
    }

    if ctrl {
        if let Some(pos) = selected.iter().position(|s| s == id) {
            selected.remove(pos);
        } else {
            selected.push(id.to_string());
        }
    } else {
        selected.clear();
        selected.push(id.to_string());
    }
    *anchor = Some(index);
}

/// The header "Select" button some views add (R11.2's playlist add-tracks browser, among
/// others): selects every id, or clears the selection if everything is already selected.
pub fn toggle_select_all(selected: &mut Vec<String>, all_ids: &[String]) {
    if selected.len() == all_ids.len() {
        selected.clear();
    } else {
        selected.clear();
        selected.extend(all_ids.iter().cloned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("id-{i}")).collect()
    }

    #[test]
    fn plain_click_selects_only_that_row() {
        let all = ids(5);
        let mut selected = vec!["id-0".to_string(), "id-1".to_string()];
        let mut anchor = Some(0);
        handle_click(&mut selected, &mut anchor, &all, 3, "id-3", false, false);
        assert_eq!(selected, vec!["id-3".to_string()]);
        assert_eq!(anchor, Some(3));
    }

    #[test]
    fn ctrl_click_toggles_membership_without_touching_the_rest() {
        let all = ids(5);
        let mut selected = vec!["id-1".to_string()];
        let mut anchor = Some(1);

        handle_click(&mut selected, &mut anchor, &all, 3, "id-3", false, true);
        assert_eq!(selected, vec!["id-1".to_string(), "id-3".to_string()]);

        handle_click(&mut selected, &mut anchor, &all, 1, "id-1", false, true);
        assert_eq!(selected, vec!["id-3".to_string()]);
    }

    #[test]
    fn shift_click_selects_the_contiguous_range_from_the_anchor() {
        let all = ids(6);
        let mut selected = vec!["id-1".to_string()];
        let mut anchor = Some(1);
        handle_click(&mut selected, &mut anchor, &all, 4, "id-4", true, false);
        assert_eq!(selected, vec!["id-1", "id-2", "id-3", "id-4"]);
        // Anchor is unchanged by a shift-select, so a second shift-click re-ranges from id-1.
        assert_eq!(anchor, Some(1));
    }

    #[test]
    fn shift_click_works_backwards_from_the_anchor_too() {
        let all = ids(6);
        let mut selected = vec!["id-4".to_string()];
        let mut anchor = Some(4);
        handle_click(&mut selected, &mut anchor, &all, 1, "id-1", true, false);
        assert_eq!(selected, vec!["id-1", "id-2", "id-3", "id-4"]);
    }

    #[test]
    fn shift_click_with_no_anchor_behaves_like_a_plain_click() {
        let all = ids(5);
        let mut selected = Vec::new();
        let mut anchor = None;
        handle_click(&mut selected, &mut anchor, &all, 2, "id-2", true, false);
        assert_eq!(selected, vec!["id-2".to_string()]);
        assert_eq!(anchor, Some(2));
    }

    #[test]
    fn toggle_select_all_selects_then_clears() {
        let all = ids(3);
        let mut selected = Vec::new();
        toggle_select_all(&mut selected, &all);
        assert_eq!(selected.len(), 3);
        toggle_select_all(&mut selected, &all);
        assert!(selected.is_empty());
    }
}
