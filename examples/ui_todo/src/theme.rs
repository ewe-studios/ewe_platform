//! Define the shared Todo theme for browser and native packages.
/// Defines the optional application theme for the three renderers.
#[must_use]
pub fn todo_theme() -> foundation_theme::GeneratedTheme {
    foundation_theme::Theme::new()
        .color("primary", "#155f59", Some("#9ae4d8"))
        .spacing("md", "8px")
        .spacing("lg", "12px")
        .spacing("sm", "4px")
        .build()
}
