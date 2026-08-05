//! F02 history persistence test (AC #5): history saved to disk survives a
//! reload, as would happen across REPL restarts. Only built with the
//! `history-file` feature.
#![cfg(feature = "history-file")]

use foundation_repl::ReplHistory;

#[test]
fn history_survives_save_and_reload() {
    let path = std::env::temp_dir().join(format!("repl_hist_{}.json", std::process::id()));
    let _ = std::fs::remove_file(&path);

    let mut history = ReplHistory::new(100);
    history.push("first".into());
    history.push("second".into());
    history.save_to_file(&path).expect("save history");

    // A fresh instance loaded from disk (i.e. the next process run).
    let mut reloaded = ReplHistory::load_from_file(&path).expect("load history");
    assert_eq!(reloaded.up(), Some("second"), "newest recalled first");
    assert_eq!(reloaded.up(), Some("first"));

    let _ = std::fs::remove_file(&path);
}
