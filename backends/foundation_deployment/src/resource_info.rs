//! Resource info trait for computing resource identifiers from input types.
//!
//! WHY: Provider methods need to compute resource IDs, kinds, and providers
//!      from input types without needing an output instance.
//!
//! WHAT: Trait with static methods implemented by generated helper structs
//!       for each endpoint.
//!
//! HOW: Code generator analyzes `OpenAPI` specs to determine resource ID patterns
//!      and generates implementations for each endpoint.

/// Trait for computing resource information from input types.
///
/// Implemented by generated helper structs for each API endpoint.
/// The code generator creates these helpers based on `OpenAPI` specs.
///
/// # Type Parameters
///
/// * `Input` - The input/args type for this endpoint
pub trait ResourceInfo<Input>: Send + Sync {
    /// Compute resource ID from input.
    ///
    /// # Arguments
    ///
    /// * `input` - The request input
    ///
    /// # Returns
    ///
    /// Unique resource identifier (e.g., "`gcp::cloudkms::AutoKeyConfig/folders/123`")
    fn compute_resource_id(input: &Input) -> String;

    /// Get the resource kind (e.g., "`gcp::cloudkms::AutoKeyConfig`").
    fn resource_kind() -> &'static str;

    /// Get the provider name (e.g., "gcp").
    fn provider() -> &'static str;
}

/// Helper function to compute resource info using `ResourceInfo` trait.
///
/// Used by provider methods to extract all resource info from input in one call.
pub fn compute_resource_info<Input, Helper>(input: &Input) -> (String, &'static str, &'static str)
where
    Helper: ResourceInfo<Input>,
{
    (
        Helper::compute_resource_id(input),
        Helper::resource_kind(),
        Helper::provider(),
    )
}
