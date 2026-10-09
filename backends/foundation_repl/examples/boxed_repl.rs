//! A REPL showing the boxed input area, the shaded output region and the
//! streaming activity indicator.
//!
//! Run it with `cargo run -p foundation_repl --example boxed_repl`, or pass a
//! palette name to compare them: `-- dusk`, `-- abyss`, `-- slate`. Then type
//! something and press Enter. Try:
//!
//! * a long line, to watch the box wrap and grow
//! * Shift+Enter, for a multiline message
//! * `/slow`, to watch the spinner stream a reply in a piece at a time
//! * `/download`, to watch the spinner become a progress bar
//! * Ctrl+U, to clear the line without submitting it
//! * `/help`, `/clear`, `/exit`

use std::thread::sleep;
use std::time::Duration;

use foundation_repl::{Repl, ReplTheme};

fn main() {
    let requested = std::env::args().nth(1);
    let theme = match requested.as_deref() {
        None => ReplTheme::default(),
        Some(name) => ReplTheme::by_name(name).unwrap_or_else(|| {
            let known = ReplTheme::palettes()
                .iter()
                .map(|(known, _)| *known)
                .collect::<Vec<_>>()
                .join(", ");
            eprintln!("unknown palette {name:?}; try one of: {known}");
            std::process::exit(2);
        }),
    };

    let palette = requested.as_deref().unwrap_or("nebula");
    let repl = Repl::builder()
        .prompt("| ")
        .continuation_prompt("|... ")
        .banner(format!(
            "foundation_repl demo ({palette}) — /slow, /download, /help, /exit"
        ))
        .goodbye("Goodbye!")
        .theme(theme)
        .build();

    for input in repl.messages() {
        match input.trim() {
            "/slow" => stream_reply(&repl),
            "/download" => show_progress(&repl),
            "" => {}
            other => repl.reply(format!("You said: {other}")),
        }
    }
}

/// Stream a reply a word at a time, the way a token stream arrives.
fn stream_reply(repl: &Repl) {
    let stream = repl.animation("thinking");

    for word in "Streaming works by committing each row to the terminal's \
                 scrollback as soon as it can no longer change, so a long \
                 reply scrolls like ordinary output instead of being trapped \
                 in a redrawn region."
        .split_inclusive(' ')
    {
        stream.push(word);
        sleep(Duration::from_millis(40));
    }

    stream.finish();
}

/// Report real progress, which turns the spinner into a bar.
fn show_progress(repl: &Repl) {
    let work = repl.animation("downloading");

    for step in 0..=20 {
        work.set_progress(Some(f64::from(step) / 20.0));
        if step == 10 {
            work.set_label("downloading (halfway)");
        }
        sleep(Duration::from_millis(80));
    }

    work.finish();
    repl.reply("Download complete.");
}
