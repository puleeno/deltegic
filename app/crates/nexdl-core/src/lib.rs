pub mod downloader;
pub mod queue;
pub mod task;

pub use downloader::HttpDownloader;
pub use queue::{DownloadQueue, QueueEvent};
pub use task::{
    DownloadTask, TaskId, TaskStatus, TaskPriority,
    TaskProgress, AddonMeta, TaskEvent,
};

/// Core error type
#[derive(Debug, thiserror::Error)]
pub enum NexDLError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("URL parse error: {0}")]
    Url(#[from] url::ParseError),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Task not found: {0}")]
    TaskNotFound(TaskId),
    #[error("Addon error: {0}")]
    AddonError(String),
    #[error("Account error: {0}")]
    AccountError(String),
    #[error("Captcha required: {0}")]
    CaptchaRequired(String),
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, NexDLError>;
