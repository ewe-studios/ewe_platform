//! Error definitions for the foundation_ai project

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

/// A comprehensive enums representing all errors supported by package.
#[derive(From, Debug)]
pub enum FoundationAIErrors {
    ModelErrors(ModelErrors),
    GenerationErrors(GenerationError),
}

pub type FoundationAIResult<T> = std::result::Result<T, FoundationAIErrors>;
