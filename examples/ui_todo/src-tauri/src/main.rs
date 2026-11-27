use std::error::Error;
use std::io::{Error as IoError, ErrorKind};

use tauri::{Url, WebviewUrl, WebviewWindowBuilder};

fn main() -> Result<(), Box<dyn Error>> {
    let address = std::env::args().nth(1).ok_or_else(|| {
        IoError::new(
            ErrorKind::InvalidInput,
            "Use ui-todo-desktop <TODO_URL>. Copy the URL printed by ui_todo.",
        )
    })?;
    let service = Url::parse(&address)?;
    if !matches!(service.scheme(), "http" | "https") || service.host().is_none() {
        return Err(IoError::new(ErrorKind::InvalidInput, "Use an HTTP or HTTPS Todo URL.").into());
    }

    tauri::Builder::default()
        .setup(move |app| {
            WebviewWindowBuilder::new(app, "todo", WebviewUrl::External(service))
                .title("Foundation Todo Desktop")
                .inner_size(820.0, 760.0)
                .center()
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())?;
    Ok(())
}
