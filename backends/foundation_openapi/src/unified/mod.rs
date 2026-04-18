//! Unified generator for OpenAPI specs.
//!
//! WHY: Single generator that produces cohesive per-endpoint units.
//!
//! WHAT: For each endpoint, generates:
//! 1. Resource types (response types, Args types)
//! 2. Client functions (builder, task, execute, convenience)
//! 3. ProviderClient impl block with wrapper method
//!
//! HOW: Groups endpoints (10-200 per group), then generates one module per group
//! where each endpoint's artifacts are colocated.

mod analyzer;
mod generator;

pub use analyzer::{analyze_spec, AnalysisOptions, AnalysisResult, ApiGroup};
pub use generator::{UnifiedGenerator, GenError};
