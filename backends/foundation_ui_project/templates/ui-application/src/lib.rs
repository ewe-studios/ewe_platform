//! Shared application identity and starter behavior.
pub const TITLE: &str = include_str!("../assets/title.txt");

pub fn increment(value: u32) -> u32 {
    value.saturating_add(1)
}
