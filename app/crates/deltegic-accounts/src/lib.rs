pub mod cookie;
pub mod store;

pub use cookie::{CookieJar, CookieEntry};
pub use store::{AccountStore, Account, AccountId};

#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    #[error("Account not found: {0}")]
    NotFound(uuid::Uuid),
    #[error("Encryption error: {0}")]
    Encryption(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AccountError>;
