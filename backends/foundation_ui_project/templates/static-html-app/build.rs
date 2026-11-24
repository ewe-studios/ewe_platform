//! Stage the public files before the server embeds them.
fn main() -> std::io::Result<()> {
    foundation_resources::build::stage_from_cargo()?;
    Ok(())
}
