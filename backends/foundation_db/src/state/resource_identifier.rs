//! Resource identifier trait for generating unique resource IDs.
//!
//! WHY: Resources need unique IDs for state tracking, but the ID format
//!      varies by API and operation. Some use output fields, some use
//!      input fields, some combine both.
//!
//! WHAT: Trait implemented by output types, with associated input type,
//!       providing a method to generate the resource ID from both.
//!
//! HOW: Generator analyzes OpenAPI spec to determine ID pattern for each
//!      endpoint and generates appropriate trait implementations.

use std::fmt::Debug;

/// Trait for generating unique resource identifiers.
///
/// Implemented by output types to generate resource IDs from input/output pairs.
/// The code generator automatically creates implementations based on OpenAPI specs.
pub trait ResourceIdentifier<Input>: Debug + Send + Sync {
    /// Generate resource ID from input and output.
    ///
    /// # Arguments
    ///
    /// * `input` - The request input that produced this output
    ///
    /// # Returns
    ///
    /// Unique resource identifier (e.g., "gcp::cloudkms::AutoKeyConfig/folders/123")
    fn generate_resource_id(&self, input: &Input) -> String;

    /// Get the resource kind (e.g., "gcp::cloudkms::AutoKeyConfig").
    fn resource_kind(&self) -> &'static str;

    /// Get the provider name (e.g., "gcp").
    fn provider(&self) -> &'static str;
}

/// Helper function to compute resource info using ResourceIdentifier trait.
///
/// Used by StoreStateTaskWithResourceIdentifier to extract all resource info
/// from the output and input in one call.
pub fn compute_resource_info<Input, Output>(
    output: &Output,
    input: &Input,
) -> (String, &'static str, &'static str)
where
    Output: ResourceIdentifier<Input>,
{
    (
        output.generate_resource_id(input),
        output.resource_kind(),
        output.provider(),
    )
}
