//! Import Todo records or convert numeric backup identifiers.
#[path = "maintenance/backup.rs"]
mod backup;
use std::path::PathBuf;

#[foundation_core::valtron::valtron(seed = 0)]
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut args = std::env::args_os().skip(1);
    let operation = args
        .next()
        .ok_or("Use todo-maintenance convert INPUT OUTPUT or import INPUT DATABASE")?;
    let input = PathBuf::from(args.next().ok_or("Pass the input backup")?);
    let output = PathBuf::from(args.next().ok_or("Pass the destination")?);
    if args.next().is_some() {
        return Err("Unexpected maintenance argument".into());
    }
    let limit = foundation_ui_wire::WireLimits::default().frame_bytes() as u64;
    let ctx = foundation_context::Ctx::try_host()?;
    foundation_core::env::install(&ctx, foundation_core::env::system)?;
    if operation == "convert" {
        backup::convert(&ctx, &input, &output, limit, std::io::stdout().lock())
    } else if operation == "import" {
        let records = backup::restore(&input, limit)?;
        foundation_core::valtron::block_on_future(async move {
            ui_todo::TodoStore::import(&ctx, &output, records).await
        })
    } else {
        Err("Use todo-maintenance convert INPUT OUTPUT or import INPUT DATABASE".into())
    }
}
