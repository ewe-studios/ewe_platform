//! A gap in an embedded group must fail the build.

use foundation_db::MigrationSet;

static BAD: MigrationSet = foundation_db::embed_migrations!("migrations");

fn main() {
    let _ = &BAD;
}
