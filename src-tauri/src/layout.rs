// SPDX-FileCopyrightText: 2026 The eepview contributors
// SPDX-License-Identifier: MIT

//! Window layout: the toolbar strip, the content area and the status bubble. Pure math in
//! logical pixels.

/// Toolbar height with the find bar closed.
pub const TOOLBAR: f64 = 84.0;
/// Toolbar height with the find bar open.
pub const TOOLBAR_FIND: f64 = 124.0;
/// The space a popup keeps from the window edges.
pub const POPUP_MARGIN: f64 = 8.0;
/// The gap between a popup and its anchor.
pub const POPUP_GAP: f64 = 4.0;
/// Status bubble height.
pub const STATUS_HEIGHT: f64 = 24.0;
/// The vertical center of the 44 px tab row, where the macOS window buttons sit.
pub const TAB_ROW_CENTER: f64 = 22.0;

/// The space the tab strip leaves for the window buttons: their right edge plus a gap equal
/// to their left margin, so the gap after them matches the margin before them. 0 without
/// buttons (a native title bar) and in full screen.
#[must_use]
pub fn chrome_inset(buttons: Option<(f64, f64)>, fullscreen: bool) -> f64 {
    match buttons {
        Some((left, right)) if !fullscreen => (right + left).max(0.0),
        _ => 0.0,
    }
}

/// A rectangle in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left.
    pub x: f64,
    /// Top.
    pub y: f64,
    /// Width.
    pub w: f64,
    /// Height.
    pub h: f64,
}

/// The toolbar height: 124 with the find bar open (`find_open`, or a find-bar request of
/// 124 or more from the UI), else 84. A popup never changes it.
#[must_use]
pub fn toolbar_height(find_open: bool, requested: f64) -> f64 {
    if find_open || requested >= TOOLBAR_FIND {
        TOOLBAR_FIND
    } else {
        TOOLBAR
    }
}

/// The toolbar strip and the content area right under it, to the window bottom.
#[must_use]
pub fn split(width: f64, height: f64, toolbar: f64) -> (Rect, Rect) {
    let top = toolbar.clamp(0.0, height.max(0.0));
    let bar = Rect {
        x: 0.0,
        y: 0.0,
        w: width,
        h: top,
    };
    let content = Rect {
        x: 0.0,
        y: top,
        w: width,
        h: (height - top).max(0.0),
    };
    (bar, content)
}

/// A size or a window extent: (width, height) in logical pixels.
pub type Size = (f64, f64);

/// Which edge of its anchor a popup lines up with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// The left edges.
    Start,
    /// The right edges.
    End,
}

/// The popup rectangle for a card of natural `size` (width, height) under `anchor`, in a
/// window of `window` (width, height). It sits [`POPUP_GAP`] under the anchor, keeps
/// [`POPUP_MARGIN`] from the window edges, and is never larger than the window allows.
#[must_use]
pub fn popup(anchor: Rect, size: Size, window: Size, align: Align) -> Rect {
    let (win_w, win_h) = window;
    let w = size.0.min(win_w - 2.0 * POPUP_MARGIN).max(0.0);
    let y = anchor.y + anchor.h + POPUP_GAP;
    let h = size.1.min(win_h - y - POPUP_MARGIN).max(0.0);
    let wanted = match align {
        Align::Start => anchor.x,
        Align::End => anchor.x + anchor.w - w,
    };
    let x = wanted.min(win_w - POPUP_MARGIN - w).max(POPUP_MARGIN);
    Rect { x, y, w, h }
}

/// The widest status bubble, as a share of the content width.
pub const STATUS_MAX_SHARE: f64 = 0.5;

/// The bottom corner of the content area that holds the status bubble.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Bottom left (the default).
    Left,
    /// Bottom right: the mouse is over the bottom-left spot.
    Right,
}

impl Side {
    /// The name the status page uses.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

/// The status bubble for a pill of `size` (width, height) measured by the status page. It
/// is at most [`STATUS_MAX_SHARE`] of the content width (the page ends longer text with an
/// ellipsis). It sits bottom left, or bottom right when `cursor` is over that spot.
#[must_use]
pub fn status(content: Rect, size: (f64, f64), cursor: Option<(f64, f64)>) -> (Rect, Side) {
    let w = size.0.min(content.w * STATUS_MAX_SHARE).max(1.0);
    let h = size.1.min(STATUS_HEIGHT * 2.0).min(content.h).max(1.0);
    let left = Rect {
        x: content.x,
        y: content.y + content.h - h,
        w,
        h,
    };
    let covered = cursor.is_some_and(|(x, y)| {
        x >= left.x && x <= left.x + left.w && y >= left.y && y <= left.y + left.h
    });
    if covered {
        let right = Rect {
            x: content.x + content.w - w,
            ..left
        };
        return (right, Side::Right);
    }
    (left, Side::Left)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inset_mirrors_the_left_margin() {
        assert!((chrome_inset(Some((14.0, 66.0)), false) - 80.0).abs() < f64::EPSILON);
        assert!(chrome_inset(Some((14.0, 66.0)), true).abs() < f64::EPSILON);
        assert!(chrome_inset(None, false).abs() < f64::EPSILON);
    }

    fn content() -> Rect {
        Rect {
            x: 0.0,
            y: 84.0,
            w: 1000.0,
            h: 700.0,
        }
    }

    #[test]
    fn status_bubble_fits_the_pill() {
        let (r, side) = status(content(), (90.0, 20.0), None);
        assert_eq!(side, Side::Left);
        assert!((r.w - 90.0).abs() < 1e-9 && r.x.abs() < 1e-9);
        assert!((r.y + r.h - 784.0).abs() < 1e-9);
    }

    #[test]
    fn status_bubble_width_is_capped() {
        let (r, _) = status(content(), (5000.0, 20.0), None);
        assert!((r.w - 500.0).abs() < 1e-9);
        let (tiny, _) = status(content(), (0.0, 0.0), None);
        assert!((tiny.w - 1.0).abs() < 1e-9);
    }

    #[test]
    fn status_bubble_moves_away_from_the_mouse() {
        let (r, side) = status(content(), (90.0, 20.0), Some((40.0, 775.0)));
        assert_eq!((side, side.name()), (Side::Right, "right"));
        assert!((r.x - 910.0).abs() < 1e-9);
        let (_, away) = status(content(), (90.0, 20.0), Some((400.0, 775.0)));
        assert_eq!(away.name(), "left");
    }
}
