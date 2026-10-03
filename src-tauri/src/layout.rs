//! Window layout: the toolbar strip, the content area and the status bubble. Pure math in
//! logical pixels.

/// Toolbar height with the find bar closed.
pub const TOOLBAR: f64 = 84.0;
/// Toolbar height with the find bar open.
pub const TOOLBAR_FIND: f64 = 124.0;
/// Largest toolbar height the UI may ask for (suggestion list).
pub const TOOLBAR_MAX: f64 = 480.0;
/// Status bubble height.
pub const STATUS_HEIGHT: f64 = 24.0;
const STATUS_CHAR: f64 = 7.0;
const STATUS_PAD: f64 = 20.0;

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

/// The toolbar height: the UI request when there is one, else by find-bar state.
#[must_use]
pub fn toolbar_height(find_open: bool, requested: f64) -> f64 {
    let base = if find_open { TOOLBAR_FIND } else { TOOLBAR };
    if requested > 0.0 {
        requested.clamp(base, TOOLBAR_MAX)
    } else {
        base
    }
}

/// The toolbar strip and the content area of a window.
///
/// The content area keeps its place under the default toolbar when the toolbar grows for the
/// suggestion list, so the page does not jump; the toolbar then covers its top.
#[must_use]
pub fn split(width: f64, height: f64, toolbar: f64, find_open: bool) -> (Rect, Rect) {
    let top = if find_open { TOOLBAR_FIND } else { TOOLBAR };
    let bar = Rect {
        x: 0.0,
        y: 0.0,
        w: width,
        h: toolbar.min(height),
    };
    let content = Rect {
        x: 0.0,
        y: top.min(height),
        w: width,
        h: (height - top).max(0.0),
    };
    (bar, content)
}

/// The status bubble at the bottom left of the content area, sized for `chars` characters.
#[must_use]
pub fn status(content: Rect, chars: usize) -> Rect {
    let count = f64::from(u32::try_from(chars).unwrap_or(u32::MAX));
    let w = (count * STATUS_CHAR + STATUS_PAD)
        .min(content.w * 0.6)
        .max(40.0);
    let h = STATUS_HEIGHT.min(content.h);
    Rect {
        x: content.x,
        y: content.y + content.h - h,
        w,
        h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_heights() {
        assert!((toolbar_height(false, 0.0) - TOOLBAR).abs() < 1e-9);
        assert!((toolbar_height(true, 0.0) - TOOLBAR_FIND).abs() < 1e-9);
        assert!((toolbar_height(false, 300.0) - 300.0).abs() < 1e-9);
        assert!((toolbar_height(false, 9000.0) - TOOLBAR_MAX).abs() < 1e-9);
        assert!((toolbar_height(true, 10.0) - TOOLBAR_FIND).abs() < 1e-9);
    }

    #[test]
    fn split_window() {
        let (bar, content) = split(1200.0, 800.0, 300.0, false);
        assert_eq!(
            bar,
            Rect {
                x: 0.0,
                y: 0.0,
                w: 1200.0,
                h: 300.0
            }
        );
        assert_eq!(
            content,
            Rect {
                x: 0.0,
                y: 84.0,
                w: 1200.0,
                h: 716.0
            }
        );
        let (_, find) = split(1200.0, 800.0, TOOLBAR_FIND, true);
        assert!((find.y - TOOLBAR_FIND).abs() < 1e-9);
        let (tiny_bar, tiny) = split(100.0, 50.0, TOOLBAR, false);
        assert!((tiny_bar.h - 50.0).abs() < 1e-9 && tiny.h.abs() < 1e-9);
    }

    #[test]
    fn status_bubble() {
        let content = Rect {
            x: 0.0,
            y: 84.0,
            w: 1000.0,
            h: 700.0,
        };
        let r = status(content, 10);
        assert!((r.w - 90.0).abs() < 1e-9);
        assert!((r.y + r.h - 784.0).abs() < 1e-9);
        assert!((status(content, 500).w - 600.0).abs() < 1e-9);
        assert!((status(content, 0).w - 40.0).abs() < 1e-9);
    }
}
