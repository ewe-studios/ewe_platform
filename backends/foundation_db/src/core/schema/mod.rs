//! Database schema and migrations.

mod fleet;
mod layout;
mod migrations;

pub use fleet::{DatabaseResolver, FleetConfig, FleetOutcome, FleetPlan, FleetReport, FleetRunner};
pub use layout::{EmbeddedLayout, MigrationLayout, MigrationLayoutConfig};
pub use migrations::{
    embed_migration_layout, embed_migrations, parse_migration_name, EmbeddedMigration, Migration,
    MigrationConfig, MigrationError, MigrationName, MigrationProblem, MigrationRunner,
    MigrationSet, MigrationSource, PlannedMigration, RunSummary, DB_CORE, DB_CORE_GROUP,
    MIGRATIONS,
};
