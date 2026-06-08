use std::fmt;

#[derive(Debug)]
pub enum CedarError {
    PolicyParse(String),
    SchemaParse(String),
    Validation(Vec<String>),
    EntityParse(String),
    RequestBuild(String),
    Authorization(String),
    Storage(String),
}

impl fmt::Display for CedarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PolicyParse(s) => write!(f, "Policy parse error: {s}"),
            Self::SchemaParse(s) => write!(f, "Schema parse error: {s}"),
            Self::Validation(errs) => write!(f, "Validation errors: {}", errs.join("; ")),
            Self::EntityParse(s) => write!(f, "Entity parse error: {s}"),
            Self::RequestBuild(s) => write!(f, "Request build error: {s}"),
            Self::Authorization(s) => write!(f, "Authorization error: {s}"),
            Self::Storage(s) => write!(f, "Storage error: {s}"),
        }
    }
}

impl std::error::Error for CedarError {}
