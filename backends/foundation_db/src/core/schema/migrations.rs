//! Database migrations, grouped per database.
//!
//! One database has an ordered list of groups. A template group (for example
//! `shared`) applies first, then the database's own group. Each group numbers
//! its migrations from 1 with no gap and no repeat. [`MigrationRunner`] checks
//! the rules, reads the record table, and applies each pending migration with
//! its record row in one transaction where the backend supports it.
//!
//! A group comes from code ([`MigrationSource::Static`]), from files embedded at
//! build time ([`MigrationSource::Embedded`], made by `embed_migrations!`), or
//! from a directory read through the virtual file system (VFS) trait of
//! `foundation_vfs` ([`MigrationSource::Directory`]).
//!
//! Written in ASD-STE100 Simplified Technical English (STE).

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use foundation_compact::time::unix_seconds;
use foundation_context::{Clock, MonotonicClock};
use foundation_vfs::async_traits::{AsyncVfsDirectory, AsyncVfsFileSystem};
use foundation_vfs::types::VfsFileType;
use futures_lite::StreamExt;
use sha2::{Digest, Sha256};

use crate::core::errors::{StorageError, StorageResult};
use crate::core::storage_provider::{DataValue, QueryStore};

/// Re-exported so a caller writes `foundation_db::embed_migrations!`.
pub use foundation_macros::{embed_migration_layout, embed_migrations};

/// A single database migration written in code.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    /// `<number>_<name>`, for example `001_create_kv_store`.
    pub id: &'static str,
    /// A short description for an operator.
    pub name: &'static str,
    /// Structured Query Language (SQL) statements of this migration.
    pub sql: &'static str,
}

macro_rules! core_migration {
    ($id:literal, $name:literal, $file:literal) => {
        Migration {
            id: $id,
            name: $name,
            sql: include_str!(concat!("sql/", $file)),
        }
    };
}

/// The core migrations of this crate, in number order.
pub static MIGRATIONS: &[Migration] = &[
    core_migration!(
        "001_create_kv_store",
        "Create key-value store table",
        "001_create_kv_store.sql"
    ),
    core_migration!(
        "002_create_users",
        "Create users table",
        "002_create_users.sql"
    ),
    core_migration!(
        "003_create_sessions",
        "Create sessions table",
        "003_create_sessions.sql"
    ),
    core_migration!(
        "004_create_accounts",
        "Create accounts table for OAuth",
        "004_create_accounts.sql"
    ),
    core_migration!(
        "005_create_verification_tokens",
        "Create verification tokens table",
        "005_create_verification_tokens.sql"
    ),
    core_migration!(
        "006_create_oauth_credentials",
        "Create OAuth credentials table",
        "006_create_oauth_credentials.sql"
    ),
    core_migration!(
        "007_create_oauth_states",
        "Create OAuth states table for PKCE",
        "007_create_oauth_states.sql"
    ),
    core_migration!(
        "008_create_jwt_tokens",
        "Create JWT tokens table",
        "008_create_jwt_tokens.sql"
    ),
    core_migration!(
        "009_create_auth_states",
        "Create auth states table",
        "009_create_auth_states.sql"
    ),
    core_migration!(
        "010_create_api_keys",
        "Create API keys table",
        "010_create_api_keys.sql"
    ),
    core_migration!(
        "011_create_two_factor",
        "Create two-factor authentication tables",
        "011_create_two_factor.sql"
    ),
    core_migration!(
        "012_create_email_otps",
        "Create email OTPs table",
        "012_create_email_otps.sql"
    ),
    core_migration!(
        "013_create_magic_links",
        "Create magic links table",
        "013_create_magic_links.sql"
    ),
    core_migration!(
        "014_create_rate_limits",
        "Create rate limits table",
        "014_create_rate_limits.sql"
    ),
    core_migration!(
        "015_create_audit_logs",
        "Create audit logs table",
        "015_create_audit_logs.sql"
    ),
    core_migration!(
        "016_create_oauth_clients",
        "Create OAuth clients table",
        "016_create_oauth_clients.sql"
    ),
    core_migration!(
        "017_create_authorization_codes",
        "Create authorization codes table",
        "017_create_authorization_codes.sql"
    ),
    core_migration!(
        "018_create_refresh_tokens",
        "Create refresh tokens table",
        "018_create_refresh_tokens.sql"
    ),
    core_migration!(
        "019_create_device_codes",
        "Create device codes table",
        "019_create_device_codes.sql"
    ),
    core_migration!(
        "020_create_documents",
        "Create documents table for DocumentStore",
        "020_create_documents.sql"
    ),
    core_migration!(
        "021_promote_document_columns",
        "Add promoted searchable columns to documents",
        "021_promote_document_columns.sql"
    ),
    core_migration!(
        "022_add_documents_r2_key",
        "Add r2_key column for R2 blob offload",
        "022_add_documents_r2_key.sql"
    ),
    core_migration!(
        "023_create_vectors",
        "Create vectors table and namespace index",
        "023_create_vectors.sql"
    ),
    core_migration!(
        "024_create_upstream_providers",
        "Create upstream identity providers table for social login",
        "024_create_upstream_providers.sql"
    ),
    core_migration!(
        "025_create_user_provider_links",
        "Create user-provider links table for account linking",
        "025_create_user_provider_links.sql"
    ),
    core_migration!(
        "026_auth_timestamps_milliseconds",
        "Convert legacy auth-table timestamps to Unix milliseconds",
        "026_auth_timestamps_milliseconds.sql"
    ),
    core_migration!(
        "027_rebuild_rate_limits",
        "Rebuild the rate limits table with the documented shape",
        "027_rebuild_rate_limits.sql"
    ),
    core_migration!(
        "028_create_tos_acceptances",
        "Create the Terms of Service acceptances table",
        "028_create_tos_acceptances.sql"
    ),
];

/// The group name of the core migrations.
pub const DB_CORE_GROUP: &str = "db_core";

/// The core migrations of `foundation_db` as one group, numbered 1 to 28.
pub static DB_CORE: MigrationSet = MigrationSet {
    group: Cow::Borrowed(DB_CORE_GROUP),
    source: MigrationSource::Static(MIGRATIONS),
};

/// A group: a name and its migrations.
#[derive(Debug, Clone)]
pub struct MigrationSet {
    /// The group name, stored in every record row of this group.
    /// `[a-z0-9_]{1,64}`. Example: `shared`.
    pub group: Cow<'static, str>,
    /// Where the migrations of this group are read from.
    pub source: MigrationSource,
}

/// Where the migrations of one group are read from.
#[derive(Debug, Clone)]
pub enum MigrationSource {
    /// Migrations written in code. The number is the leading digits of `id`.
    Static(&'static [Migration]),
    /// Files read at build time by `embed_migrations!`. Works on every target,
    /// including WebAssembly (wasm) targets that have no file system.
    Embedded(&'static [EmbeddedMigration]),
    /// A directory read at run time through the VFS trait given to `plan`.
    /// The path is portable; only reading it needs a file system.
    Directory(PathBuf),
}

/// One migration embedded by `embed_migrations!`.
#[derive(Debug, Clone, Copy)]
pub struct EmbeddedMigration {
    /// The number in the file name, read as an integer.
    pub number: u64,
    /// The `<name>` part of `<number>_<name>.sql`.
    pub name: &'static str,
    /// The Structured Query Language (SQL) text of the file.
    pub sql: &'static str,
    /// Lowercase hexadecimal Secure Hash Algorithm with 256 bits (SHA-256) of
    /// `sql`, computed at build time.
    pub sha256: &'static str,
}

/// One migration after `plan` read it and checked it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedMigration {
    /// The group name.
    pub group: String,
    /// 1, 2, 3, ... inside the group.
    pub number: u64,
    /// The `<name>` part of `<number>_<name>.sql`, or the code description for
    /// a `Static` migration.
    pub name: String,
    /// The Structured Query Language (SQL) text.
    pub sql: Cow<'static, str>,
    /// Lowercase hexadecimal Secure Hash Algorithm with 256 bits (SHA-256) of
    /// `sql`.
    pub checksum: String,
    /// A file path, or `group[number]` for a migration in code.
    pub origin: String,
}

/// The ordered groups of one database, plus the rules. Cheap to clone.
#[derive(Debug, Clone)]
pub struct MigrationRunner {
    groups: Vec<Arc<MigrationSet>>,
    config: MigrationConfig,
}

/// Rules and limits of the runner.
///
/// The defaults match the design: four digits in a file number, checksum
/// verification, a one-Mebibyte file cap and the table `_migrations`.
#[derive(Clone)]
pub struct MigrationConfig {
    /// Fewest digits in a directory or embedded file number. Default 4, the
    /// form that `wrangler d1 migrations` makes. A `Static` source may use
    /// fewer digits, as the three-digit core identifiers do.
    pub min_number_digits: usize,
    /// Refuse to run if the Structured Query Language (SQL) of an applied
    /// migration changed. Default `true`.
    pub verify_checksums: bool,
    /// Treat an unknown group in the record table as an error. Default `false`
    /// (it becomes a warning in [`RunSummary`]).
    pub strict_groups: bool,
    /// Largest migration file that a `Directory` source reads. Default 1
    /// Mebibyte (MiB), the default `client_max_body_size` of nginx. A schema
    /// change is small.
    pub max_file_bytes: u64,
    /// The record table name. Default `_migrations`.
    pub table: String,
    /// The wall clock for the `applied_at` column. A host passes the clock of
    /// its `Ctx`; a test passes a simulated clock.
    pub clock: Arc<dyn Clock>,
}

impl fmt::Debug for MigrationConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MigrationConfig")
            .field("min_number_digits", &self.min_number_digits)
            .field("verify_checksums", &self.verify_checksums)
            .field("strict_groups", &self.strict_groups)
            .field("max_file_bytes", &self.max_file_bytes)
            .field("table", &self.table)
            .finish_non_exhaustive()
    }
}

impl Default for MigrationConfig {
    fn default() -> Self {
        Self {
            min_number_digits: 4,
            verify_checksums: true,
            strict_groups: false,
            max_file_bytes: 1024 * 1024,
            table: "_migrations".to_string(),
            clock: Arc::new(MonotonicClock::new()),
        }
    }
}

impl MigrationConfig {
    /// Set the fewest digits in a file number.
    #[must_use]
    pub fn with_min_number_digits(mut self, digits: usize) -> Self {
        self.min_number_digits = digits;
        self
    }

    /// Turn checksum verification on or off.
    #[must_use]
    pub fn with_verify_checksums(mut self, verify: bool) -> Self {
        self.verify_checksums = verify;
        self
    }

    /// Treat an unknown group as an error (`true`) or as a warning (`false`).
    #[must_use]
    pub fn with_strict_groups(mut self, strict: bool) -> Self {
        self.strict_groups = strict;
        self
    }

    /// Set the largest file that a `Directory` source reads, in bytes.
    #[must_use]
    pub fn with_max_file_bytes(mut self, bytes: u64) -> Self {
        self.max_file_bytes = bytes;
        self
    }

    /// Set the record table name.
    #[must_use]
    pub fn with_table(mut self, table: impl Into<String>) -> Self {
        self.table = table.into();
        self
    }

    /// Set the clock for the `applied_at` column.
    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }
}

/// What one [`MigrationRunner::run_async`] did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunSummary {
    /// `(group, number)` of each migration applied now, in order.
    pub applied: Vec<(String, u64)>,
    /// Groups in the record table that this runner does not have.
    pub unknown_groups: Vec<String>,
}

/// Every problem found by one call, in a stable order.
#[derive(Debug)]
pub struct MigrationError {
    /// Every problem, in group order and then number order.
    pub problems: Vec<MigrationProblem>,
}

impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} migration problem(s):", self.problems.len())?;
        for problem in &self.problems {
            writeln!(f, "- {problem}")?;
        }
        Ok(())
    }
}

impl std::error::Error for MigrationError {}

impl From<StorageError> for MigrationError {
    fn from(error: StorageError) -> Self {
        Self {
            problems: vec![MigrationProblem::Storage(error)],
        }
    }
}

impl From<MigrationError> for StorageError {
    fn from(error: MigrationError) -> Self {
        Self::Migration(error.to_string())
    }
}

/// One migration problem.
#[derive(Debug)]
pub enum MigrationProblem {
    /// The group name is not `[a-z0-9_]{1,64}`.
    BadGroupName {
        /// The group name.
        group: String,
    },
    /// The same group appears more than once in one runner.
    DuplicateGroup {
        /// The group name.
        group: String,
    },
    /// A file name is not `<digits>_<name>.sql`.
    BadName {
        /// The file path or code location.
        origin: String,
    },
    /// A directory or embedded number has too few digits.
    ShortNumber {
        /// The file path.
        origin: String,
        /// The digit count that the file has.
        digits: usize,
    },
    /// The same number appears twice in one group.
    DuplicateNumber {
        /// The group name.
        group: String,
        /// The number.
        number: u64,
        /// Every origin that uses the number.
        origins: Vec<String>,
    },
    /// A number in the range 1 to the last number is missing.
    Gap {
        /// The group name.
        group: String,
        /// The missing number.
        missing: u64,
    },
    /// A file is larger than the configured limit.
    TooLarge {
        /// The file path.
        origin: String,
        /// The file size in bytes.
        bytes: u64,
    },
    /// A file is not valid UTF-8 text.
    NotUtf8 {
        /// The file path.
        origin: String,
    },
    /// The Structured Query Language (SQL) of an applied migration changed.
    ChecksumChanged {
        /// The group name.
        group: String,
        /// The migration number.
        number: u64,
        /// The stored checksum.
        stored: String,
        /// The checksum of the current text.
        now: String,
    },
    /// An applied number has a different name now.
    NameChanged {
        /// The group name.
        group: String,
        /// The migration number.
        number: u64,
        /// The name in the record table.
        applied: String,
        /// The name in the plan.
        planned: String,
    },
    /// The applied numbers of a group are not 1 to k.
    AppliedNotPrefix {
        /// The group name.
        group: String,
        /// Every missing number below the last applied number.
        missing: Vec<u64>,
    },
    /// An applied number is not in the plan.
    AppliedButMissing {
        /// The group name.
        group: String,
        /// The migration number.
        number: u64,
    },
    /// The record table has a group that the runner does not have.
    UnknownGroup {
        /// The group name.
        group: String,
    },
    /// A `Directory` group could not be read.
    Io {
        /// The path that failed.
        origin: String,
        /// The message of the host file system.
        message: String,
    },
    /// A layout group folder contains a folder.
    NestedFolder {
        /// The nested folder path.
        origin: String,
    },
    /// The Structured Query Language (SQL) of a migration failed.
    Apply {
        /// The group name.
        group: String,
        /// The migration number.
        number: u64,
        /// The backend message.
        message: String,
    },
    /// The record table could not be read or written.
    Storage(StorageError),
}

impl fmt::Display for MigrationProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadGroupName { group } => write!(
                f,
                "bad group name {group:?}: use lowercase letters, digits and _ (1 to 64 characters)"
            ),
            Self::DuplicateGroup { group } => {
                write!(f, "group {group:?} appears more than once in this runner")
            }
            Self::BadName { origin } => write!(
                f,
                "{origin}: a migration file name must be <digits>_<name>.sql, with a lowercase name"
            ),
            Self::ShortNumber { origin, digits } => write!(
                f,
                "{origin}: the number has {digits} digits, but at least the configured minimum is required"
            ),
            Self::DuplicateNumber {
                group,
                number,
                origins,
            } => write!(
                f,
                "group {group:?} uses number {number} more than once: {}",
                origins.join(", ")
            ),
            Self::Gap { group, missing } => {
                write!(f, "group {group:?} is missing migration number {missing}")
            }
            Self::TooLarge { origin, bytes } => write!(
                f,
                "{origin}: the file is {bytes} bytes, more than the configured limit"
            ),
            Self::NotUtf8 { origin } => write!(f, "{origin}: the file is not UTF-8 text"),
            Self::ChecksumChanged {
                group,
                number,
                stored,
                now,
            } => write!(
                f,
                "group {group:?} number {number} changed after it ran (stored {stored}, now {now})"
            ),
            Self::NameChanged {
                group,
                number,
                applied,
                planned,
            } => write!(
                f,
                "group {group:?} number {number} ran as {applied:?}, but the plan has {planned:?}"
            ),
            Self::AppliedNotPrefix { group, missing } => write!(
                f,
                "group {group:?} has a gap below its last applied number: missing {}",
                join_numbers(missing)
            ),
            Self::AppliedButMissing { group, number } => write!(
                f,
                "group {group:?} number {number} is in the record table but not in the plan"
            ),
            Self::UnknownGroup { group } => {
                write!(f, "the record table has group {group:?}, but the runner does not")
            }
            Self::Io { origin, message } => write!(f, "{origin}: {message}"),
            Self::NestedFolder { origin } => write!(
                f,
                "{origin}: a group folder must not contain a folder"
            ),
            Self::Apply {
                group,
                number,
                message,
            } => write!(
                f,
                "group {group:?} number {number} failed and was not recorded: {message}"
            ),
            Self::Storage(error) => write!(f, "storage: {error}"),
        }
    }
}

fn join_numbers(numbers: &[u64]) -> String {
    numbers
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// A parsed migration file name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationName {
    /// The number, read as an integer. `0001` and `1` give the same value.
    pub number: u64,
    /// The `<name>` part.
    pub name: String,
}

/// Parse a migration file name of the form `<digits>_<name>.sql`.
///
/// The number must be at least 1 and must have at least `min_number_digits`
/// digits. The name must use lowercase letters, digits and `_`.
///
/// # Errors
///
/// Returns [`MigrationProblem::BadName`] when the shape is wrong. Returns
/// [`MigrationProblem::ShortNumber`] when the number has too few digits.
pub fn parse_migration_name(
    file_name: &str,
    origin: &str,
    min_number_digits: usize,
) -> Result<MigrationName, MigrationProblem> {
    let Some(stem) = file_name.strip_suffix(".sql") else {
        return Err(MigrationProblem::BadName {
            origin: origin.to_string(),
        });
    };
    parse_migration_stem(stem, origin, min_number_digits)
}

fn parse_migration_stem(
    stem: &str,
    origin: &str,
    min_number_digits: usize,
) -> Result<MigrationName, MigrationProblem> {
    let Some((digits, name)) = stem.split_once('_') else {
        return Err(MigrationProblem::BadName {
            origin: origin.to_string(),
        });
    };
    if digits.is_empty()
        || !digits.bytes().all(|b| b.is_ascii_digit())
        || name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err(MigrationProblem::BadName {
            origin: origin.to_string(),
        });
    }
    if digits.len() < min_number_digits {
        return Err(MigrationProblem::ShortNumber {
            origin: origin.to_string(),
            digits: digits.len(),
        });
    }
    let number = digits
        .parse::<u64>()
        .map_err(|_| MigrationProblem::BadName {
            origin: origin.to_string(),
        })?;
    if number == 0 {
        return Err(MigrationProblem::BadName {
            origin: origin.to_string(),
        });
    }
    Ok(MigrationName {
        number,
        name: name.to_string(),
    })
}

fn valid_group_name(group: &str) -> bool {
    !group.is_empty()
        && group.len() <= 64
        && group
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

pub(crate) fn join_path(root: &str, name: &str) -> String {
    if root.is_empty() {
        name.to_string()
    } else if root.ends_with('/') {
        format!("{root}{name}")
    } else {
        format!("{root}/{name}")
    }
}

impl MigrationRunner {
    /// A runner whose first group is `set`.
    #[must_use]
    pub fn from_static(set: &'static MigrationSet) -> Self {
        Self {
            groups: vec![Arc::new(set.clone())],
            config: MigrationConfig::default(),
        }
    }

    /// A runner whose first group is the directory `path`.
    ///
    /// The group name is the last path segment. `plan` reads the files through
    /// the file system that it is given, so this constructor reads nothing.
    #[must_use]
    pub fn from_dir(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self {
            groups: vec![Arc::new(directory_set(&path))],
            config: MigrationConfig::default(),
        }
    }

    /// A new runner: this runner's groups, then `set`. `self` does not change.
    #[must_use]
    pub fn add_from_static(&self, set: &'static MigrationSet) -> Self {
        let mut groups = self.groups.clone();
        groups.push(Arc::new(set.clone()));
        Self {
            groups,
            config: self.config.clone(),
        }
    }

    /// A new runner: this runner's groups, then the directory `path`.
    #[must_use]
    pub fn add_from_dir(&self, path: impl Into<PathBuf>) -> Self {
        let mut groups = self.groups.clone();
        groups.push(Arc::new(directory_set(&path.into())));
        Self {
            groups,
            config: self.config.clone(),
        }
    }

    /// Set the rules and limits of this runner.
    #[must_use]
    pub fn with_config(mut self, config: MigrationConfig) -> Self {
        self.config = config;
        self
    }

    /// The group names, in apply order.
    #[must_use]
    pub fn groups(&self) -> Vec<&str> {
        self.groups.iter().map(|set| set.group.as_ref()).collect()
    }

    pub(crate) fn from_set(set: MigrationSet, config: MigrationConfig) -> Self {
        Self {
            groups: vec![Arc::new(set)],
            config,
        }
    }

    pub(crate) fn from_sets(sets: Vec<Arc<MigrationSet>>, config: MigrationConfig) -> Self {
        Self {
            groups: sets,
            config,
        }
    }

    /// Read every group through `files`, check the plan rules, and return the
    /// migrations in apply order. This method reads no database.
    ///
    /// # Errors
    ///
    /// Returns one [`MigrationError`] that holds every broken rule.
    pub async fn plan<F: AsyncVfsFileSystem>(
        &self,
        files: &F,
    ) -> Result<Vec<PlannedMigration>, MigrationError> {
        let mut problems = Vec::new();
        let mut seen_groups = BTreeSet::new();
        let mut planned = Vec::new();

        for set in &self.groups {
            let group = set.group.as_ref();
            if !valid_group_name(group) {
                problems.push(MigrationProblem::BadGroupName {
                    group: group.to_string(),
                });
            }
            if !seen_groups.insert(group.to_string()) {
                problems.push(MigrationProblem::DuplicateGroup {
                    group: group.to_string(),
                });
            }
            match self.load_group(files, set).await {
                Ok(mut group_plan) => {
                    problems.append(&mut check_group_numbers(group, &group_plan));
                    planned.append(&mut group_plan);
                }
                Err(mut group_problems) => problems.append(&mut group_problems),
            }
        }

        if problems.is_empty() {
            Ok(planned)
        } else {
            Err(MigrationError { problems })
        }
    }

    async fn load_group<F: AsyncVfsFileSystem>(
        &self,
        files: &F,
        set: &MigrationSet,
    ) -> Result<Vec<PlannedMigration>, Vec<MigrationProblem>> {
        let group = set.group.as_ref();
        let (mut planned, problems) = match &set.source {
            MigrationSource::Static(migrations) => plan_static(group, migrations),
            MigrationSource::Embedded(migrations) => plan_embedded(group, migrations),
            MigrationSource::Directory(path) => self.load_directory(files, group, path).await,
        };
        if !problems.is_empty() {
            return Err(problems);
        }
        planned.sort_by_key(|migration| migration.number);
        Ok(planned)
    }

    async fn load_directory<F: AsyncVfsFileSystem>(
        &self,
        files: &F,
        group: &str,
        path: &Path,
    ) -> (Vec<PlannedMigration>, Vec<MigrationProblem>) {
        let root = path.to_string_lossy().into_owned();
        let mut problems = Vec::new();
        let directory = match files.open_directory_async(root.clone()).await {
            Ok(directory) => directory,
            Err(error) => {
                problems.push(MigrationProblem::Io {
                    origin: root,
                    message: error.to_string(),
                });
                return (Vec::new(), problems);
            }
        };
        let entries = match directory.list_async().await {
            Ok(entries) => entries,
            Err(error) => {
                problems.push(MigrationProblem::Io {
                    origin: root,
                    message: error.to_string(),
                });
                return (Vec::new(), problems);
            }
        };

        let mut planned = Vec::new();
        for entry in entries {
            let origin = join_path(&root, &entry.name);
            if entry.file_type == VfsFileType::Directory {
                problems.push(MigrationProblem::NestedFolder { origin });
                continue;
            }
            let parsed =
                match parse_migration_name(&entry.name, &origin, self.config.min_number_digits) {
                    Ok(parsed) => parsed,
                    Err(problem) => {
                        problems.push(problem);
                        continue;
                    }
                };
            let metadata = match files.stat_async(origin.clone()).await {
                Ok(metadata) => metadata,
                Err(error) => {
                    problems.push(MigrationProblem::Io {
                        origin: origin.clone(),
                        message: error.to_string(),
                    });
                    continue;
                }
            };
            if metadata.size > self.config.max_file_bytes {
                problems.push(MigrationProblem::TooLarge {
                    origin,
                    bytes: metadata.size,
                });
                continue;
            }
            let bytes = match files.read_file_async(origin.clone()).await {
                Ok(bytes) => bytes,
                Err(error) => {
                    problems.push(MigrationProblem::Io {
                        origin: origin.clone(),
                        message: error.to_string(),
                    });
                    continue;
                }
            };
            let Ok(sql) = String::from_utf8(bytes) else {
                problems.push(MigrationProblem::NotUtf8 { origin });
                continue;
            };
            planned.push(PlannedMigration {
                group: group.to_string(),
                number: parsed.number,
                name: parsed.name,
                checksum: sha256_hex(sql.as_bytes()),
                sql: Cow::Owned(sql),
                origin,
            });
        }
        (planned, problems)
    }

    /// Apply the missing migrations of the plan. Each migration and its record
    /// row commit in one transaction where the backend supports it.
    ///
    /// # Errors
    ///
    /// Returns one [`MigrationError`] that holds every problem found.
    pub async fn run_async<F: AsyncVfsFileSystem>(
        &self,
        files: &F,
        store: &dyn QueryStore,
    ) -> Result<RunSummary, MigrationError> {
        let plan = self.plan(files).await?;
        self.run_planned(&plan, store).await
    }

    pub(crate) async fn run_planned(
        &self,
        plan: &[PlannedMigration],
        store: &dyn QueryStore,
    ) -> Result<RunSummary, MigrationError> {
        if !store.batch_is_transactional() {
            tracing::warn!(
                table = %self.config.table,
                "the store cannot run a migration and its record row in one transaction; \
                 every migration must be safe to run twice"
            );
        }
        let repaired = self.ensure_record_table(store).await?;
        let applied = self.read_applied(store).await?;
        let runner_groups: BTreeSet<&str> =
            self.groups.iter().map(|set| set.group.as_ref()).collect();

        let mut unknown_groups: Vec<String> = applied
            .keys()
            .filter(|group| !runner_groups.contains(group.as_str()))
            .cloned()
            .collect();
        unknown_groups.sort();

        let mut problems = Vec::new();
        if self.config.strict_groups {
            for group in &unknown_groups {
                problems.push(MigrationProblem::UnknownGroup {
                    group: group.clone(),
                });
            }
        } else {
            for group in &unknown_groups {
                tracing::warn!(group = %group, "the record table has a group that this runner does not");
            }
        }
        self.check_applied(plan, &applied, &mut problems);

        if !problems.is_empty() {
            return Err(MigrationError { problems });
        }

        let mut summary = RunSummary {
            applied: repaired.into_iter().collect(),
            unknown_groups,
        };
        for set in &self.groups {
            let group = set.group.as_ref();
            let mut group_applied: BTreeSet<u64> = applied
                .get(group)
                .map_or_default(|rows| rows.iter().map(|row| row.number).collect());
            let mut group_plan: Vec<&PlannedMigration> = plan
                .iter()
                .filter(|migration| migration.group == group)
                .collect();
            group_plan.sort_by_key(|migration| migration.number);

            for migration in group_plan {
                if group_applied.contains(&migration.number) {
                    continue;
                }
                match self.apply_one(store, migration).await {
                    Ok(()) => {
                        summary.applied.push((group.to_string(), migration.number));
                        group_applied.insert(migration.number);
                    }
                    Err(error) if is_unique_violation(&error) => {
                        // Another process applied this migration first. The
                        // primary key (group_name, number) made its insert and
                        // our insert conflict, so our transaction rolled back.
                        let now = self.read_applied(store).await?;
                        let present = now.get(group).is_some_and(|rows| {
                            rows.iter().any(|row| row.number == migration.number)
                        });
                        if present {
                            group_applied.insert(migration.number);
                        } else {
                            return Err(MigrationError {
                                problems: vec![MigrationProblem::Apply {
                                    group: group.to_string(),
                                    number: migration.number,
                                    message: error.to_string(),
                                }],
                            });
                        }
                    }
                    Err(error) => {
                        return Err(MigrationError {
                            problems: vec![MigrationProblem::Apply {
                                group: group.to_string(),
                                number: migration.number,
                                message: error.to_string(),
                            }],
                        });
                    }
                }
            }
        }
        Ok(summary)
    }

    fn check_applied(
        &self,
        plan: &[PlannedMigration],
        applied: &BTreeMap<String, Vec<AppliedRow>>,
        problems: &mut Vec<MigrationProblem>,
    ) {
        for set in &self.groups {
            let group = set.group.as_ref();
            let Some(rows) = applied.get(group) else {
                continue;
            };
            let numbers: BTreeSet<u64> = rows.iter().map(|row| row.number).collect();
            let last = numbers.iter().next_back().copied().unwrap_or(0);
            let missing: Vec<u64> = (1..=last)
                .filter(|number| !numbers.contains(number))
                .collect();
            if !missing.is_empty() {
                problems.push(MigrationProblem::AppliedNotPrefix {
                    group: group.to_string(),
                    missing,
                });
            }

            let group_plan: BTreeMap<u64, &PlannedMigration> = plan
                .iter()
                .filter(|migration| migration.group == group)
                .map(|migration| (migration.number, migration))
                .collect();
            for row in rows {
                let Some(migration) = group_plan.get(&row.number) else {
                    problems.push(MigrationProblem::AppliedButMissing {
                        group: group.to_string(),
                        number: row.number,
                    });
                    continue;
                };
                if row.name != migration.name {
                    problems.push(MigrationProblem::NameChanged {
                        group: group.to_string(),
                        number: row.number,
                        applied: row.name.clone(),
                        planned: migration.name.clone(),
                    });
                }
                if self.config.verify_checksums
                    && !row.checksum.is_empty()
                    && row.checksum != migration.checksum
                {
                    problems.push(MigrationProblem::ChecksumChanged {
                        group: group.to_string(),
                        number: row.number,
                        stored: row.checksum.clone(),
                        now: migration.checksum.clone(),
                    });
                }
            }
        }
    }

    async fn apply_one(
        &self,
        store: &dyn QueryStore,
        migration: &PlannedMigration,
    ) -> StorageResult<()> {
        let applied_at = unix_seconds(self.config.clock.wall_now());
        let insert = format!(
            "INSERT INTO {} (group_name, number, name, checksum, applied_at) VALUES (?, ?, ?, ?, ?)",
            self.table_identifier()?
        );
        let params = vec![
            DataValue::Text(migration.group.clone()),
            DataValue::Integer(number_to_i64(migration.number)?),
            DataValue::Text(migration.name.clone()),
            DataValue::Text(migration.checksum.clone()),
            DataValue::Integer(applied_at),
        ];

        if !store.batch_is_transactional() {
            store.execute_batch(&migration.sql).await?;
            store.execute(&insert, &params).await?;
            return Ok(());
        }

        let mut statements: Vec<(String, Vec<DataValue>)> = split_sql(&migration.sql)
            .into_iter()
            .map(|statement| (statement, Vec::new()))
            .collect();
        statements.push((insert, params));
        store.execute_batch_statements(&statements).await
    }

    /// Fill an empty checksum in the record table from the current Structured
    /// Query Language (SQL). An operator runs this after checking that the
    /// files did not change.
    ///
    /// # Errors
    ///
    /// Returns one [`MigrationError`] when the plan or the record table fails.
    pub async fn record_checksums<F: AsyncVfsFileSystem>(
        &self,
        files: &F,
        store: &dyn QueryStore,
    ) -> Result<usize, MigrationError> {
        let plan = self.plan(files).await?;
        self.ensure_record_table(store).await?;
        let applied = self.read_applied(store).await?;
        let table = self.table_identifier()?;
        let mut updated = 0usize;
        for migration in &plan {
            let Some(rows) = applied.get(&migration.group) else {
                continue;
            };
            let Some(row) = rows.iter().find(|row| row.number == migration.number) else {
                continue;
            };
            if !row.checksum.is_empty() {
                continue;
            }
            store
                .execute(
                    &format!("UPDATE {table} SET checksum = ? WHERE group_name = ? AND number = ?"),
                    &[
                        DataValue::Text(migration.checksum.clone()),
                        DataValue::Text(migration.group.clone()),
                        DataValue::Integer(number_to_i64(migration.number)?),
                    ],
                )
                .await?;
            updated += 1;
        }
        Ok(updated)
    }

    /// Create or upgrade the record table. Returns the `(group, number)` of
    /// the 023 repair when this call applied it.
    async fn ensure_record_table(
        &self,
        store: &dyn QueryStore,
    ) -> Result<Option<(String, u64)>, MigrationError> {
        match self.table_shape(store).await? {
            TableShape::Missing => {
                store
                    .execute_batch(&create_table_sql(&self.table_identifier()?))
                    .await?;
            }
            TableShape::Current { has_checksum } => {
                if !has_checksum {
                    store
                        .execute_batch(&format!(
                            "ALTER TABLE {} ADD COLUMN checksum TEXT NOT NULL DEFAULT ''",
                            self.table_identifier()?
                        ))
                        .await?;
                }
            }
            TableShape::Legacy => return self.upgrade_record_table(store).await,
        }
        Ok(None)
    }

    async fn table_shape(&self, store: &dyn QueryStore) -> StorageResult<TableShape> {
        let table = self.table_identifier()?;
        let mut rows = store
            .query(&format!("PRAGMA table_info('{table}')"), &[])
            .await?;
        let mut columns = Vec::new();
        while let Some(row) = rows.next().await {
            let row = row?;
            columns.push(row.get::<String>(1)?);
        }
        drop(rows);
        if columns.is_empty() {
            return Ok(TableShape::Missing);
        }
        if columns.iter().any(|column| column == "group_name") {
            return Ok(TableShape::Current {
                has_checksum: columns.iter().any(|column| column == "checksum"),
            });
        }
        if columns.iter().any(|column| column == "id") {
            return Ok(TableShape::Legacy);
        }
        Err(StorageError::Migration(format!(
            "table {table} has no group_name column and no id column"
        )))
    }

    async fn upgrade_record_table(
        &self,
        store: &dyn QueryStore,
    ) -> Result<Option<(String, u64)>, MigrationError> {
        let table = self.table_identifier()?;
        let next = format!("{table}_next");
        let mut rows = store.query(&format!("SELECT id FROM {table}"), &[]).await?;
        let mut old_ids = Vec::new();
        while let Some(row) = rows.next().await {
            let row = row?;
            old_ids.push(row.get::<String>(0)?);
        }
        drop(rows);

        let has_core = old_ids.iter().any(|id| id.starts_with("001_"));
        let has_023 = old_ids.iter().any(|id| id.starts_with("023_"));
        let repair_023 = has_core && !has_023;

        let mut statements: Vec<(String, Vec<DataValue>)> = vec![
            (format!("DROP TABLE IF EXISTS {next}"), Vec::new()),
            (create_table_sql(&next), Vec::new()),
            (
                format!(
                    "INSERT INTO {next} (group_name, number, name, checksum, applied_at) \
                     SELECT '{DB_CORE_GROUP}', CAST(substr(id, 1, instr(id, '_') - 1) AS INTEGER), \
                     name, '', applied_at FROM {table}"
                ),
                Vec::new(),
            ),
            (format!("DROP TABLE {table}"), Vec::new()),
            (format!("ALTER TABLE {next} RENAME TO {table}"), Vec::new()),
        ];

        if repair_023 {
            let migration = MIGRATIONS
                .iter()
                .find(|migration| migration.id.starts_with("023_"))
                .ok_or_else(|| {
                    StorageError::Migration(
                        "the 023 repair needs 023_create_vectors.sql".to_string(),
                    )
                })?;
            for statement in split_sql(migration.sql) {
                statements.push((statement, Vec::new()));
            }
            statements.push((
                format!(
                    "INSERT INTO {table} (group_name, number, name, checksum, applied_at) \
                     VALUES (?, ?, ?, ?, ?)"
                ),
                vec![
                    DataValue::Text(DB_CORE_GROUP.to_string()),
                    DataValue::Integer(23),
                    DataValue::Text(migration.name.to_string()),
                    DataValue::Text(sha256_hex(migration.sql.as_bytes())),
                    DataValue::Integer(unix_seconds(self.config.clock.wall_now())),
                ],
            ));
        }

        if let Err(error) = store.execute_batch_statements(&statements).await {
            // Two processes can upgrade at the same time. If the other process
            // finished, the table now has the new shape.
            if matches!(self.table_shape(store).await?, TableShape::Current { .. }) {
                return Ok(None);
            }
            return Err(error.into());
        }
        Ok(repair_023.then(|| (DB_CORE_GROUP.to_string(), 23)))
    }

    async fn read_applied(
        &self,
        store: &dyn QueryStore,
    ) -> Result<BTreeMap<String, Vec<AppliedRow>>, MigrationError> {
        let table = self.table_identifier()?;
        let mut rows = store
            .query(
                &format!("SELECT group_name, number, name, checksum FROM {table}"),
                &[],
            )
            .await?;
        let mut applied: BTreeMap<String, Vec<AppliedRow>> = BTreeMap::new();
        while let Some(row) = rows.next().await {
            let row = row?;
            let group = row.get::<String>(0)?;
            let number = row.get::<i64>(1)?;
            let name = row.get::<String>(2)?;
            let checksum = row.get::<String>(3)?;
            let number = u64::try_from(number).map_err(|_| {
                StorageError::Migration(format!(
                    "the record table has a negative migration number in group {group:?}"
                ))
            })?;
            applied.entry(group).or_default().push(AppliedRow {
                number,
                name,
                checksum,
            });
        }
        drop(rows);
        for rows in applied.values_mut() {
            rows.sort_by_key(|row| row.number);
        }
        Ok(applied)
    }

    fn table_identifier(&self) -> StorageResult<String> {
        let table = self.config.table.as_str();
        let mut bytes = table.bytes();
        let first = bytes.next();
        let valid_first = first.is_some_and(|b| b.is_ascii_alphabetic() || b == b'_');
        if !valid_first || !bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err(StorageError::Migration(format!(
                "the record table name {table:?} is not a plain identifier"
            )));
        }
        Ok(table.to_string())
    }
}

#[derive(Debug)]
enum TableShape {
    Missing,
    Legacy,
    Current { has_checksum: bool },
}

#[derive(Debug)]
struct AppliedRow {
    number: u64,
    name: String,
    checksum: String,
}

fn create_table_sql(table: &str) -> String {
    format!(
        "CREATE TABLE IF NOT EXISTS {table} (
            group_name TEXT    NOT NULL,
            number     INTEGER NOT NULL,
            name       TEXT    NOT NULL,
            checksum   TEXT    NOT NULL,
            applied_at INTEGER NOT NULL,
            PRIMARY KEY (group_name, number)
        )"
    )
}

fn directory_set(path: &Path) -> MigrationSet {
    let group = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    MigrationSet {
        group: Cow::Owned(group),
        source: MigrationSource::Directory(path.to_path_buf()),
    }
}

fn plan_static(
    group: &str,
    migrations: &'static [Migration],
) -> (Vec<PlannedMigration>, Vec<MigrationProblem>) {
    let mut planned = Vec::new();
    let mut problems = Vec::new();
    for migration in migrations {
        let origin = format!("{group}[{}]", migration.id);
        // A `Static` migration may use fewer than four digits, as the
        // three-digit core identifiers do.
        match parse_migration_stem(migration.id, &origin, 0) {
            Ok(parsed) => planned.push(PlannedMigration {
                group: group.to_string(),
                number: parsed.number,
                name: migration.name.to_string(),
                checksum: sha256_hex(migration.sql.as_bytes()),
                sql: Cow::Borrowed(migration.sql),
                origin,
            }),
            Err(problem) => problems.push(problem),
        }
    }
    (planned, problems)
}

fn plan_embedded(
    group: &str,
    migrations: &'static [EmbeddedMigration],
) -> (Vec<PlannedMigration>, Vec<MigrationProblem>) {
    let mut planned = Vec::new();
    let mut problems = Vec::new();
    for migration in migrations {
        let origin = format!("{group}/{:04}_{}.sql", migration.number, migration.name);
        if migration.number == 0
            || migration.name.is_empty()
            || !migration
                .name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            problems.push(MigrationProblem::BadName { origin });
            continue;
        }
        planned.push(PlannedMigration {
            group: group.to_string(),
            number: migration.number,
            name: migration.name.to_string(),
            checksum: migration.sha256.to_string(),
            sql: Cow::Borrowed(migration.sql),
            origin,
        });
    }
    (planned, problems)
}

fn check_group_numbers(group: &str, planned: &[PlannedMigration]) -> Vec<MigrationProblem> {
    let mut problems = Vec::new();
    let mut by_number: BTreeMap<u64, Vec<String>> = BTreeMap::new();
    for migration in planned {
        by_number
            .entry(migration.number)
            .or_default()
            .push(migration.origin.clone());
    }
    for (number, origins) in &by_number {
        if origins.len() > 1 {
            problems.push(MigrationProblem::DuplicateNumber {
                group: group.to_string(),
                number: *number,
                origins: origins.clone(),
            });
        }
    }
    let mut expected = 1u64;
    for number in by_number.keys() {
        while expected < *number {
            problems.push(MigrationProblem::Gap {
                group: group.to_string(),
                missing: expected,
            });
            expected += 1;
        }
        expected = number + 1;
    }
    problems
}

fn number_to_i64(number: u64) -> StorageResult<i64> {
    i64::try_from(number).map_err(|_| {
        StorageError::Migration(format!(
            "migration number {number} is too large for the database"
        ))
    })
}

/// Whether a storage error is a unique or primary-key conflict.
fn is_unique_violation(error: &StorageError) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("unique constraint") || message.contains("primary key")
}

/// Split Structured Query Language (SQL) text into single statements.
///
/// The split keeps a semicolon inside a quoted string or a comment as data. A
/// `CREATE TRIGGER` body with an inner semicolon is not supported.
fn split_sql(sql: &str) -> Vec<String> {
    enum State {
        Normal,
        Single,
        Double,
        Backtick,
        LineComment,
        BlockComment,
    }

    let bytes = sql.as_bytes();
    let mut state = State::Normal;
    let mut start = 0usize;
    let mut out = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        match state {
            State::Normal => match byte {
                b'\'' => state = State::Single,
                b'"' => state = State::Double,
                b'`' => state = State::Backtick,
                b'-' if bytes.get(index + 1) == Some(&b'-') => {
                    state = State::LineComment;
                    index += 1;
                }
                b'/' if bytes.get(index + 1) == Some(&b'*') => {
                    state = State::BlockComment;
                    index += 1;
                }
                b';' => {
                    let statement = sql[start..index].trim();
                    if !statement.is_empty() {
                        out.push(statement.to_string());
                    }
                    start = index + 1;
                }
                _ => {}
            },
            State::Single => {
                if byte == b'\'' {
                    if bytes.get(index + 1) == Some(&b'\'') {
                        index += 1;
                    } else {
                        state = State::Normal;
                    }
                }
            }
            State::Double => {
                if byte == b'"' {
                    if bytes.get(index + 1) == Some(&b'"') {
                        index += 1;
                    } else {
                        state = State::Normal;
                    }
                }
            }
            State::Backtick => {
                if byte == b'`' {
                    if bytes.get(index + 1) == Some(&b'`') {
                        index += 1;
                    } else {
                        state = State::Normal;
                    }
                }
            }
            State::LineComment => {
                if byte == b'\n' {
                    state = State::Normal;
                }
            }
            State::BlockComment => {
                if byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    state = State::Normal;
                    index += 1;
                }
            }
        }
        index += 1;
    }
    let statement = sql[start..].trim();
    if !statement.is_empty() {
        out.push(statement.to_string());
    }
    out
}
