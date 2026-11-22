//! The shared table and `new_mime_guess` agree on common extensions.
//!
//! The embedded-asset pipeline in `foundation_macros` keeps `new_mime_guess`.
//! Both tables come from mime-db, but they resolve an ambiguous extension by a
//! different rule. This test names the extensions where the two agree, so a
//! change to either table shows up here.
//!
//! One common extension is deliberately absent:
//!
//! - `mjs`: the shared table picks `text/javascript` (the Internet Assigned
//!   Numbers Authority (IANA) entry, and the reason for issue #578).
//!   `new_mime_guess` picks `application/javascript`.

use foundation_netio::shared::mime::from_extension;
use new_mime_guess::MimeGuess;

/// Twenty-two common extensions where both tables give the same answer.
const COMMON: [&str; 22] = [
    "html", "htm", "css", "js", "json", "png", "jpg", "jpeg", "gif", "svg", "ico", "woff", "woff2",
    "ttf", "txt", "xml", "pdf", "zip", "wasm", "webp", "mp4", "mpg4",
];

#[test]
fn common_extensions_agree_with_new_mime_guess() {
    for extension in COMMON {
        let theirs = MimeGuess::from_ext(extension)
            .first_raw()
            .unwrap_or_else(|| panic!("new_mime_guess holds the {extension:?} row"));
        let ours = from_extension(extension)
            .unwrap_or_else(|| panic!("the shared table holds the {extension:?} row"));
        assert_eq!(
            ours, theirs,
            "extension {extension:?}: the shared table says {ours:?}, new_mime_guess says {theirs:?}"
        );
    }
}
