// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Requirement tests for the toolbar popups (`docs/wiki/browser-shell.md`, "Toolbar popups").
//!
//! They use the public interface only and name the rule they check. The expected numbers
//! (84, 124, 4 px, 8 px) come from the rules, not from the code.

use eepview_lib::layout::{self, Align, Rect};
use eepview_lib::popup::{Closed, Kind, Popups};
use proptest::prelude::*;

const EPS: f64 = 1e-6;
/// Rule 3: the gap between the anchor and the popup.
const GAP: f64 = 4.0;
/// Rule 4 and rule 5: the distance to the window edges.
const MARGIN: f64 = 8.0;

const ALL: [Kind; 4] = [Kind::Suggestions, Kind::Menu, Kind::Router, Kind::Hint];

fn close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() < EPS,
        "{what}: expected {expected}, got {actual}"
    );
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect { x, y, w, h }
}

/// A toolbar button, in the nav row.
fn button() -> Rect {
    rect(700.0, 44.0, 32.0, 32.0)
}

/// The address field, in the nav row.
fn address() -> Rect {
    rect(120.0, 48.0, 500.0, 28.0)
}

/// The popup rect for a card of `size` under `anchor`.
fn place(anchor: Rect, size: (f64, f64), window: (f64, f64), align: Align) -> Rect {
    layout::popup(anchor, size, window, align)
}

fn assert_inside(r: Rect, window: (f64, f64)) {
    assert!(
        r.x >= MARGIN - EPS,
        "left edge {} is closer than 8 px to the window",
        r.x
    );
    assert!(
        r.x + r.w <= window.0 - MARGIN + EPS,
        "right edge {} is closer than 8 px to the window width {}",
        r.x + r.w,
        window.0
    );
    assert!(
        r.y + r.h <= window.1 - MARGIN + EPS,
        "bottom edge {} is below the window bottom minus 8 ({})",
        r.y + r.h,
        window.1
    );
}

// ---- Rule 2: the toolbar height ----

#[test]
fn rule_2_toolbar_constants_are_84_and_124() {
    // Rule 2: The toolbar is always exactly 84 px high, or 124 px while the find bar is open.
    close(layout::TOOLBAR, 84.0, "TOOLBAR");
    close(layout::TOOLBAR_FIND, 124.0, "TOOLBAR_FIND");
}

#[test]
fn rule_2_toolbar_is_84_whatever_a_popup_asks_for() {
    // Rule 2: No popup changes the toolbar height. A suggestion list asks for a lot.
    for requested in [0.0, 1.0, 84.0, 100.0, 123.9] {
        close(
            layout::toolbar_height(false, requested),
            84.0,
            "find closed",
        );
    }
}

#[test]
fn rule_2_toolbar_is_124_with_find_open_whatever_is_asked() {
    // Rule 2: 124 px while the find bar is open, and not a pixel more or less.
    for requested in [0.0, 84.0, 124.0, 300.0, 480.0, 10_000.0] {
        close(layout::toolbar_height(true, requested), 124.0, "find open");
    }
}

#[test]
fn rule_2_toolbar_height_is_never_anything_but_84_or_124() {
    // Rule 2: the interface says "Nothing else".
    for find in [false, true] {
        for requested in [-5.0, 0.0, 40.0, 84.0, 123.0, 124.0, 125.0, 480.0, 1.0e6] {
            let h = layout::toolbar_height(find, requested);
            assert!(
                (h - 84.0).abs() < EPS || (h - 124.0).abs() < EPS,
                "find={find} requested={requested} gave {h}"
            );
        }
    }
}

#[test]
fn rule_2_a_request_of_124_or_more_gives_the_find_height() {
    // Rule 2 (interface): 124 when `requested` is 124 or more, else 84.
    close(layout::toolbar_height(false, 124.0), 124.0, "requested 124");
    close(layout::toolbar_height(false, 480.0), 124.0, "requested 480");
    close(
        layout::toolbar_height(false, 123.9),
        84.0,
        "requested 123.9",
    );
}

#[test]
fn rule_2_content_starts_right_under_the_toolbar_and_fills_the_window() {
    // Rule 2: The content area starts right under the toolbar and fills the rest of the window.
    for toolbar in [layout::TOOLBAR, layout::TOOLBAR_FIND] {
        let (bar, content) = layout::split(1000.0, 700.0, toolbar);
        close(bar.x, 0.0, "bar x");
        close(bar.y, 0.0, "bar y");
        close(bar.w, 1000.0, "bar width");
        close(bar.h, toolbar, "bar height");
        close(content.x, 0.0, "content x");
        close(content.y, toolbar, "content y");
        close(content.w, 1000.0, "content width");
        close(content.h, 700.0 - toolbar, "content height");
    }
}

#[test]
fn rule_2_content_height_is_never_negative() {
    // Rule 2 (interface): Heights are never negative.
    for height in [0.0, 10.0, 84.0, 100.0, 123.0] {
        let (_, content) = layout::split(800.0, height, layout::TOOLBAR_FIND);
        assert!(
            content.h >= 0.0,
            "content height {} in a window of {height}",
            content.h
        );
    }
}

#[test]
fn rule_2_content_keeps_its_rect_while_a_popup_is_open() {
    // Rule 1 and rule 2: Opening a popup never hides or moves the page.
    let window = (1000.0, 700.0);
    let toolbar = layout::toolbar_height(false, 480.0);
    let before = layout::split(window.0, window.1, toolbar);
    let mut popups = Popups::default();
    for kind in ALL {
        let (id, _) = popups.open(kind, button());
        popups.size(id, (320.0, 900.0), window);
        let after = layout::split(window.0, window.1, layout::toolbar_height(false, 480.0));
        assert_eq!(before, after, "the layout changed while {kind:?} was open");
    }
}

// ---- Rule 3: where a popup opens ----

#[test]
fn rule_3_popup_opens_4_px_below_its_anchor() {
    // Rule 3: A popup opens 4 px below its anchor.
    for align in [Align::Start, Align::End] {
        let r = place(button(), (240.0, 200.0), (1000.0, 700.0), align);
        close(r.y, 44.0 + 32.0 + 4.0, "popup top");
    }
}

#[test]
fn rule_3_suggestions_line_up_with_the_left_edge_of_the_field() {
    // Rule 3: The suggestions line up with the left edge of the address field and take its width.
    let a = address();
    let r = place(a, (a.w, 240.0), (1000.0, 700.0), Align::Start);
    close(r.x, a.x, "left edge");
    close(r.w, a.w, "width");
}

#[test]
fn rule_3_menu_router_and_hint_line_up_with_the_right_edge_of_the_button() {
    // Rule 3: ... line up with the right edge of their button.
    let b = button();
    let r = place(b, (240.0, 200.0), (1000.0, 700.0), Align::End);
    close(r.x + r.w, b.x + b.w, "right edge");
    close(r.w, 240.0, "width");
}

#[test]
fn rule_3_alignment_per_kind() {
    // Rule 3 (interface): `Start` for the suggestions, `End` for the others.
    assert_eq!(Kind::Suggestions.align(), Align::Start);
    assert_eq!(Kind::Menu.align(), Align::End);
    assert_eq!(Kind::Router.align(), Align::End);
    assert_eq!(Kind::Hint.align(), Align::End);
}

#[test]
fn rule_3_popups_place_by_the_alignment_of_their_kind() {
    // Rule 3: the open popup is placed under its anchor, by the alignment of its kind.
    let window = (1000.0, 700.0);
    for (kind, anchor) in [(Kind::Suggestions, address()), (Kind::Menu, button())] {
        let mut popups = Popups::default();
        let (id, _) = popups.open(kind, anchor);
        let size = (anchor.w.min(300.0), 150.0);
        let placed = popups
            .size(id, size, window)
            .expect("the open popup gets a rect");
        assert_eq!(
            placed,
            place(anchor, size, window, kind.align()),
            "{kind:?}"
        );
    }
}

// ---- Rule 4: a popup never leaves the window ----

#[test]
fn rule_4_popup_stays_8_px_from_the_left_edge() {
    // Rule 4: at least 8 px from the left window edge.
    let anchor = rect(2.0, 44.0, 200.0, 32.0);
    let r = place(anchor, (200.0, 100.0), (1000.0, 700.0), Align::Start);
    close(r.x, 8.0, "left edge");
}

#[test]
fn rule_4_popup_stays_8_px_from_the_right_edge() {
    // Rule 4: at least 8 px from the right window edge.
    let anchor = rect(968.0, 44.0, 30.0, 32.0);
    let r = place(anchor, (260.0, 100.0), (1000.0, 700.0), Align::End);
    assert!(
        r.x + r.w <= 992.0 + EPS,
        "right edge {} is past 992",
        r.x + r.w
    );
    close(r.x + r.w, 992.0, "right edge sits at the margin");
}

#[test]
fn rule_4_popup_is_at_most_the_window_width_minus_16() {
    // Rule 4: it is at most the window width minus 16 px wide.
    let r = place(address(), (5000.0, 100.0), (400.0, 700.0), Align::Start);
    close(r.w, 384.0, "width");
    close(r.x, 8.0, "left edge");
}

#[test]
fn rule_4_popup_that_fits_keeps_its_width() {
    // Rule 4: only a popup wider than the window minus 16 px is narrowed.
    let r = place(button(), (240.0, 100.0), (1000.0, 700.0), Align::End);
    close(r.w, 240.0, "width");
}

// ---- Rule 5: a popup is never cut ----

#[test]
fn rule_5_popup_gets_the_height_it_needs() {
    // Rule 5: It gets the height it needs, up to the window bottom minus 8 px.
    let r = place(button(), (240.0, 200.0), (1000.0, 700.0), Align::End);
    close(r.h, 200.0, "height");
}

#[test]
fn rule_5_tall_popup_reaches_the_window_bottom_minus_8() {
    // Rule 5: Up to the window bottom minus 8 px. The anchor bottom is 76, the top is 80.
    let r = place(button(), (240.0, 5000.0), (1000.0, 700.0), Align::End);
    close(r.y, 80.0, "top");
    close(r.y + r.h, 692.0, "bottom");
}

#[test]
fn rule_5_tall_router_panel_in_a_small_window_stays_inside() {
    // Rule 5: its last row (the router panel's action buttons) can always be reached, so the
    // panel is cut to the window and scrolls inside itself.
    let window = (420.0, 300.0);
    let anchor = rect(380.0, 44.0, 28.0, 28.0);
    let r = place(anchor, (320.0, 900.0), window, Align::End);
    assert_inside(r, window);
    close(r.y + r.h, 292.0, "bottom");
    assert!(r.h > 0.0, "the popup has a height");
}

#[test]
fn rule_5_popup_through_the_state_is_clamped_too() {
    // Rule 5: the shell places the popup the same way, whatever size the page reports.
    let window = (420.0, 300.0);
    let mut popups = Popups::default();
    let (id, _) = popups.open(Kind::Router, rect(380.0, 44.0, 28.0, 28.0));
    let r = popups
        .size(id, (320.0, 900.0), window)
        .expect("the open popup gets a rect");
    assert_inside(r, window);
}

// ---- Rule 6: one popup at a time ----

#[test]
fn rule_6_ids_start_at_1_and_only_go_up() {
    // Rule 6 (interface): ids start at 1 and only go up.
    let mut popups = Popups::default();
    let (first, closed) = popups.open(Kind::Menu, button());
    assert_eq!(first, 1);
    assert_eq!(closed, None);
    popups.close(None);
    let (second, _) = popups.open(Kind::Menu, button());
    assert!(second > first, "id {second} does not follow {first}");
}

#[test]
fn rule_6_opening_another_kind_closes_the_open_one() {
    // Rule 6: Opening a popup of another kind closes the open one first.
    let mut popups = Popups::default();
    let (first, _) = popups.open(Kind::Suggestions, address());
    let (second, closed) = popups.open(Kind::Menu, button());
    assert_eq!(
        closed,
        Some(Closed {
            id: first,
            kind: Kind::Suggestions
        })
    );
    assert_ne!(first, second);
    assert_eq!(popups.current().map(|c| c.kind), Some(Kind::Menu));
}

#[test]
fn rule_6_only_one_popup_is_open_through_a_whole_run() {
    // Rule 6: One popup shows at a time, whatever the order of the kinds.
    let mut popups = Popups::default();
    let mut last: Option<Closed> = None;
    for kind in [
        Kind::Menu,
        Kind::Router,
        Kind::Hint,
        Kind::Suggestions,
        Kind::Router,
    ] {
        let (id, closed) = popups.open(kind, button());
        assert_eq!(closed, last, "opening {kind:?} closes the previous popup");
        last = Some(Closed { id, kind });
        assert_eq!(popups.current(), last);
    }
}

#[test]
fn rule_6_the_closed_popup_has_no_place_any_more() {
    // Rule 6 and rule 12: the closed popup's id is stale, so its size report does nothing.
    let mut popups = Popups::default();
    let (first, _) = popups.open(Kind::Suggestions, address());
    popups.open(Kind::Menu, button());
    assert_eq!(popups.size(first, (200.0, 100.0), (1000.0, 700.0)), None);
}

#[test]
fn rule_6_opening_the_same_kind_updates_in_place() {
    // Rule 6: Opening the same kind again (the suggestions while you type) updates it in place.
    let mut popups = Popups::default();
    let (first, _) = popups.open(Kind::Suggestions, address());
    let (second, closed) = popups.open(Kind::Suggestions, address());
    assert_eq!(closed, None, "the same kind is not closed");
    assert!(second > first, "ids only go up");
    assert_eq!(popups.current().map(|c| c.kind), Some(Kind::Suggestions));
    assert!(
        popups
            .size(second, (300.0, 100.0), (1000.0, 700.0))
            .is_some()
    );
}

#[test]
fn rule_6_takes_focus_for_menu_and_router_only() {
    // Rule 6 (interface): the menu and the router panel take focus. Rule 7: the hint never does.
    assert!(Kind::Menu.takes_focus());
    assert!(Kind::Router.takes_focus());
    assert!(!Kind::Hint.takes_focus());
    assert!(!Kind::Suggestions.takes_focus());
}

// ---- Rule 12: a close or a size report for a popup that is no longer open ----

#[test]
fn rule_12_close_by_id_closes_that_popup() {
    // Rule 12 (interface): closes the open popup when `id` is it.
    let mut popups = Popups::default();
    let (id, _) = popups.open(Kind::Router, button());
    assert_eq!(
        popups.close(Some(id)),
        Some(Closed {
            id,
            kind: Kind::Router
        })
    );
    assert_eq!(popups.current(), None);
}

#[test]
fn rule_12_close_without_id_closes_any_popup() {
    // Rule 12 (interface): ... or when `id` is `None`.
    for kind in ALL {
        let mut popups = Popups::default();
        let (id, _) = popups.open(kind, button());
        assert_eq!(popups.close(None), Some(Closed { id, kind }));
        assert_eq!(popups.current(), None);
    }
}

#[test]
fn rule_12_stale_close_does_nothing() {
    // Rule 12: A close for a popup that is no longer open does nothing.
    let mut popups = Popups::default();
    let (old, _) = popups.open(Kind::Suggestions, address());
    let (new, _) = popups.open(Kind::Menu, button());
    assert_eq!(popups.close(Some(old)), None);
    assert_eq!(
        popups.current(),
        Some(Closed {
            id: new,
            kind: Kind::Menu
        })
    );
}

#[test]
fn rule_12_close_twice_closes_once() {
    // Rule 12: the second close for the same id is stale.
    let mut popups = Popups::default();
    let (id, _) = popups.open(Kind::Menu, button());
    assert!(popups.close(Some(id)).is_some());
    assert_eq!(popups.close(Some(id)), None);
    assert_eq!(popups.close(None), None);
}

#[test]
fn rule_12_close_with_nothing_open_does_nothing() {
    // Rule 12: no popup is open, so nothing closes.
    let mut popups = Popups::default();
    assert_eq!(popups.current(), None);
    assert_eq!(popups.close(None), None);
    assert_eq!(popups.close(Some(7)), None);
}

#[test]
fn rule_12_size_for_a_closed_popup_does_nothing() {
    // Rule 12: A size report for a popup that is no longer open does nothing.
    let mut popups = Popups::default();
    let (id, _) = popups.open(Kind::Menu, button());
    popups.close(Some(id));
    assert_eq!(popups.size(id, (200.0, 100.0), (1000.0, 700.0)), None);
    assert_eq!(popups.current(), None, "a size report does not reopen it");
}

#[test]
fn rule_12_size_for_an_unknown_id_does_nothing() {
    // Rule 12: an id that was never handed out is not the open popup.
    let mut popups = Popups::default();
    let (id, _) = popups.open(Kind::Menu, button());
    assert_eq!(popups.size(id + 100, (200.0, 100.0), (1000.0, 700.0)), None);
    assert!(popups.size(id, (200.0, 100.0), (1000.0, 700.0)).is_some());
}

#[test]
fn rule_12_size_that_is_not_finite_and_positive_is_refused() {
    // Rule 12 (interface): `None` when the size is not finite and positive.
    let window = (1000.0, 700.0);
    let mut popups = Popups::default();
    let (id, _) = popups.open(Kind::Menu, button());
    for size in [
        (0.0, 100.0),
        (100.0, 0.0),
        (-5.0, 100.0),
        (100.0, -5.0),
        (f64::NAN, 100.0),
        (100.0, f64::NAN),
        (f64::INFINITY, 100.0),
        (100.0, f64::INFINITY),
    ] {
        assert_eq!(popups.size(id, size, window), None, "size {size:?}");
    }
    assert_eq!(
        popups.current().map(|c| c.id),
        Some(id),
        "a bad size does not close it"
    );
}

// ---- Rules 1, 3, 4 and 5 as properties ----

/// A window and an anchor in its toolbar: the anchor bottom plus the 4 px gap stays above the
/// bottom margin.
fn scene() -> impl Strategy<Value = ((f64, f64), Rect)> {
    (200.0..3000.0_f64, 150.0..2000.0_f64).prop_flat_map(|window| {
        (
            0.0..window.0 - 20.0,
            0.0..44.0_f64,
            20.0..window.0.min(600.0),
            20.0..40.0_f64,
        )
            .prop_map(move |(x, y, w, h)| (window, rect(x, y, w, h)))
    })
}

proptest! {
    #[test]
    fn rule_4_and_5_popup_is_always_inside_the_window(
        (window, anchor) in scene(),
        size in (1.0..6000.0_f64, 1.0..6000.0_f64),
        end in any::<bool>(),
    ) {
        // Rule 4 and rule 5: for any window, anchor and size the popup is inside the window,
        // 8 px from the sides and the bottom, and never wider than the window minus 16 px.
        let align = if end { Align::End } else { Align::Start };
        let r = place(anchor, size, window, align);
        assert_inside(r, window);
        prop_assert!(r.w <= window.0 - 16.0 + EPS, "width {} in window {}", r.w, window.0);
        prop_assert!(r.w > 0.0 && r.h > 0.0, "{r:?}");
        prop_assert!((r.y - (anchor.y + anchor.h + GAP)).abs() < EPS, "{r:?} under {anchor:?}");
    }

    #[test]
    fn rule_3_and_5_popup_that_fits_keeps_its_size_and_alignment(
        (window, anchor) in scene(),
        size in (1.0..150.0_f64, 1.0..100.0_f64),
    ) {
        // Rule 5: a popup that fits keeps its size. Rule 3: it keeps its alignment when no
        // window edge pushes it.
        let start = place(anchor, size, window, Align::Start);
        prop_assert!((start.w - size.0).abs() < EPS && (start.h - size.1).abs() < EPS, "{start:?}");
        if anchor.x >= MARGIN && anchor.x + size.0 <= window.0 - MARGIN {
            prop_assert!((start.x - anchor.x).abs() < EPS, "{start:?} under {anchor:?}");
        }
        let end = place(anchor, size, window, Align::End);
        let right = anchor.x + anchor.w;
        if right - size.0 >= MARGIN && right <= window.0 - MARGIN {
            prop_assert!((end.x + end.w - right).abs() < EPS, "{end:?} under {anchor:?}");
        }
    }

    #[test]
    fn rule_1_and_2_content_never_changes_when_a_popup_opens(
        (window, anchor) in scene(),
        kind in prop::sample::select(ALL.to_vec()),
        find in any::<bool>(),
        requested in 0.0..1000.0_f64,
        size in (1.0..6000.0_f64, 1.0..6000.0_f64),
    ) {
        // Rule 1 and rule 2: the toolbar is 84 (124 with find) and the content area is the
        // same before and while a popup is open, whatever height the popup asks for.
        let toolbar = layout::toolbar_height(find, requested);
        prop_assert!((toolbar - 84.0).abs() < EPS || (toolbar - 124.0).abs() < EPS, "{toolbar}");
        if find {
            prop_assert!((toolbar - 124.0).abs() < EPS, "find open gave {toolbar}");
        }
        let before = layout::split(window.0, window.1, toolbar);
        let mut popups = Popups::default();
        let (id, _) = popups.open(kind, anchor);
        let placed = popups.size(id, size, window);
        let after = layout::split(window.0, window.1, layout::toolbar_height(find, requested));
        prop_assert_eq!(before, after);
        prop_assert!((after.1.y - toolbar.min(window.1)).abs() < EPS, "{:?}", after.1);
        prop_assert!(after.1.h >= 0.0);
        let rect = placed.expect("a finite positive size gets a rect");
        assert_inside(rect, window);
    }
}
