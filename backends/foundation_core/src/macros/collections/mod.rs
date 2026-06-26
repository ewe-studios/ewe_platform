/// `make_collection` helps generate different underlying rust collection
/// types by providing a macro that will match the underlying types implementation
/// of the From trait
///
/// Directly lifted from <https://stackoverflow.com/questions/27582739/how-do-i-create-a-hashmap-literal>
#[macro_export]
macro_rules! make_collection {
    // map-like
    ($($k:expr => $v:expr),* $(,)?) => {{
        core::convert::From::from([$(($k, $v),)*])
    }};
    // set-like
    ($($v:expr),* $(,)?) => {{
        core::convert::From::from([$($v,)*])
    }};
}

