pub mod python_types;
pub mod registry;
pub mod runner;

pub use python_types::*;
pub use registry::{AddonRegistry, AddonInfo};
pub use runner::AddonRunner;

#[derive(Debug, thiserror::Error)]
pub enum AddonApiError {
    #[error("Python error: {0}")]
    Python(String),
    #[error("Addon not found: {0}")]
    NotFound(String),
    #[error("Addon load error: {0}")]
    Load(String),
    #[error("Execution error: {0}")]
    Execution(String),
    #[error("Type error: {0}")]
    TypeError(String),
    #[error("{0}")]
    Other(String),
}

impl From<pyo3::PyErr> for AddonApiError {
    fn from(e: pyo3::PyErr) -> Self {
        AddonApiError::Python(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AddonApiError>;
