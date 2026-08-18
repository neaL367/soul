//! Integration tests for Windows 11 system theme query.

use platform_windows::{SystemTheme, query_system_theme};

#[test]
fn test_query_system_theme() {
    let theme = query_system_theme();
    // On Windows desktop, query returns either Light or Dark mode deterministically
    assert!(theme == SystemTheme::Light || theme == SystemTheme::Dark);
    assert_eq!(theme.is_dark(), theme == SystemTheme::Dark);
}
