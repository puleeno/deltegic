use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub type TaskId = Uuid;

/// Priority of a download task
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl Default for TaskPriority {
    fn default() -> Self {
        TaskPriority::Normal
    }
}

/// Status of a download task
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Waiting in queue
    Queued,
    /// Being processed by addon (URL extraction, login, etc.)
    Processing,
    /// Actively downloading
    Downloading,
    /// Post-processing (reup, transcode, etc.)
    PostProcessing,
    /// Waiting for captcha resolution
    CaptchaRequired { url: String },
    /// Paused by user
    Paused,
    /// Completed successfully
    Completed { output_path: String },
    /// Failed with error
    Failed { error: String, retries: u32 },
    /// Cancelled by user
    Cancelled,
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Completed { .. } | TaskStatus::Failed { .. } | TaskStatus::Cancelled
        )
    }

    pub fn is_active(&self) -> bool {
        matches!(
            self,
            TaskStatus::Processing | TaskStatus::Downloading | TaskStatus::PostProcessing
        )
    }
}

/// Progress information for an active download
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskProgress {
    /// Downloaded bytes
    pub downloaded: u64,
    /// Total bytes (0 if unknown)
    pub total: u64,
    /// Download speed in bytes/sec
    pub speed_bps: u64,
    /// Estimated time remaining in seconds (None if unknown)
    pub eta_secs: Option<u64>,
    /// Progress message from addon
    pub message: String,
    /// Sub-tasks (e.g. album items)
    pub subtasks_done: u32,
    pub subtasks_total: u32,
}

impl TaskProgress {
    pub fn percent(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.downloaded as f64 / self.total as f64 * 100.0).min(100.0)
        }
    }
}

/// Metadata about which addon handles this task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddonMeta {
    pub name: String,
    pub version: String,
    pub addon_path: String,
}

/// The main download task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTask {
    pub id: TaskId,
    pub url: String,
    pub title: Option<String>,
    pub output_dir: String,
    pub filename_template: Option<String>,
    pub priority: TaskPriority,
    pub status: TaskStatus,
    pub progress: TaskProgress,
    pub addon: Option<AddonMeta>,
    pub account_id: Option<Uuid>,
    /// Extra addon-specific options (passed as JSON)
    pub options: HashMap<String, serde_json::Value>,
    /// Tags for filtering/grouping
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub max_retries: u32,
    pub retry_count: u32,
}

impl DownloadTask {
    pub fn new(url: impl Into<String>, output_dir: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            url: url.into(),
            title: None,
            output_dir: output_dir.into(),
            filename_template: None,
            priority: TaskPriority::Normal,
            status: TaskStatus::Queued,
            progress: TaskProgress::default(),
            addon: None,
            account_id: None,
            options: HashMap::new(),
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
            started_at: None,
            finished_at: None,
            max_retries: 3,
            retry_count: 0,
        }
    }

    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_account(mut self, account_id: Uuid) -> Self {
        self.account_id = Some(account_id);
        self
    }

    pub fn with_option(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.options.insert(key.into(), value);
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }
}

/// Event emitted when a task changes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum TaskEvent {
    Added { task: Box<DownloadTask> },
    StatusChanged { id: TaskId, status: TaskStatus },
    ProgressUpdated { id: TaskId, progress: TaskProgress },
    Removed { id: TaskId },
}
