//! A gap in an embedded group must fail the build.

use foundation_db::MigrationSet;

static BAD: MigrationSet =
    foundation_db::embed_migrations!("backends/foundation_macros/tests/ui/embed_migrations_gap_data");

fn main() {
    let _ = &BAD;
}
