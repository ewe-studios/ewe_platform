//! Database schema and migrations.

mod migrations;

pub use migrations::{Migration, MigrationRunner, MIGRATIONS};
