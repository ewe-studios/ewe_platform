//! Resolve a generated web operation's name for a fixture assertion.
pub fn wire_name<'a>(
    id: u16,
    custom: Option<&'a str>,
    names: &'static [&'static str],
) -> Option<&'a str> {
    if id == 0 {
        custom
    } else {
        names.get(usize::from(id) - 1).copied()
    }
}
