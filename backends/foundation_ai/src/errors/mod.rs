//! Error definitions for the `foundation_ai` project

use derive_more::From;
use foundation_core::extensions::result_ext::BoxedError;

#[derive(From, Debug)]
pub enum GenerationError {
    Failed(BoxedError),
}

pub type GenerationResult<T> = std::result::Result<T, GenerationError>;

#[derive(From, Debug)]
pub enum ModelErrors {
    NotFound(String),
    FailedLoading(BoxedError),
}

pub type ModelResult<T> = std::result::Result<T, ModelErrors>;

#[derive(From, Debug)]
pub enum ModelProviderErrors {
    NotFound(String),
    FailedFetching(BoxedError),
    ModelErrors(ModelErrors),
}

pub type ModelProviderResult<T> = std::result::Result<T, ModelProviderErrors>;

/// A comprehensive types representing all errors supported by package.
#[derive(From, Debug)]
pub enum FoundationAIErrors {
    ModelErrors(ModelErrors),
    GenerationErrors(GenerationError),
    RegistryErrors(ModelProviderErrors),
}

pub type FoundationAIResult<T> = std::result::Result<T, FoundationAIErrors>;
