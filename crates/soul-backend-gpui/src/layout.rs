//! Chrome geometry constants shared by the GPUI window chrome and page hit-testing.
//!
//! These mirror the vertical layout in [`crate::view::PageView`]: a tab strip
//! above a toolbar, with page content starting below both. Keeping them in one
//! place prevents the hit-test offset from drifting out of sync with the chrome
//! it measures (which previously caused clicks in the toolbar to leak through
//! to page link regions).

/// Height of the tab strip row in logical pixels.
pub const TAB_STRIP_HEIGHT: f32 = 32.0;

/// Height of the navigation toolbar row in logical pixels.
pub const TOOLBAR_HEIGHT: f32 = 44.0;

/// Combined height of the tab strip and toolbar ("the chrome").
pub const CHROME_HEIGHT: f32 = TAB_STRIP_HEIGHT + TOOLBAR_HEIGHT;

/// Maps a window client coordinate to page coordinates.
///
/// Returns `Some((x, y))` when the point lies below the chrome (in the page
/// viewport), or `None` when the point is inside the tab strip or toolbar.
#[must_use]
pub fn page_coordinate(x: f32, y: f32) -> Option<(f32, f32)> {
    if y > CHROME_HEIGHT {
        Some((x, y - CHROME_HEIGHT))
    } else {
        None
    }
}
