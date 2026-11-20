use crate::{vocab::VOCABULARY_VERSION, MountSurface, Platform, WireLimits};

/// Platform, mount surface and vocabulary version that a client declares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientCaps {
    pub platform: Platform,
    pub mount: MountSurface,
    pub vocabulary_version: u32,
}

impl ClientCaps {
    /// Builds the current capabilities for a web mount.
    #[must_use]
    pub fn web_current() -> Self {
        Self {
            platform: Platform::Web,
            mount: MountSurface::Web,
            vocabulary_version: VOCABULARY_VERSION,
        }
    }

    /// Builds the current capabilities for a native mount.
    #[must_use]
    pub fn native_current(platform: Platform) -> Self {
        Self {
            platform,
            mount: MountSurface::Native,
            vocabulary_version: VOCABULARY_VERSION,
        }
    }
}

/// Settings for frame sizes and table rows.
///
/// The defaults use the standard wire limits.
/// Each table frame has at most 8192 rows, the `DataFusion` default batch size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriterConfig {
    table_rows: usize,
    wire_limits: WireLimits,
}

impl Default for WriterConfig {
    fn default() -> Self {
        Self {
            table_rows: 8192,
            wire_limits: WireLimits::default(),
        }
    }
}

impl WriterConfig {
    /// Sets the row limit of each table frame. A zero limit becomes one.
    #[must_use]
    pub fn max_table_rows(mut self, rows: usize) -> Self {
        self.table_rows = rows.max(1);
        self
    }

    #[must_use]
    pub fn limits(mut self, limits: WireLimits) -> Self {
        self.wire_limits = limits;
        self
    }

    #[must_use]
    pub fn table_rows(&self) -> usize {
        self.table_rows
    }

    #[must_use]
    pub fn wire_limits(&self) -> &WireLimits {
        &self.wire_limits
    }
}
