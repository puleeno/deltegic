use crate::task::{DownloadTask, TaskEvent, TaskId, TaskProgress, TaskStatus};
use crate::Result;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::Mutex;
use tokio::sync::broadcast;
use tracing::{info, warn};

/// Events broadcast from the queue
pub type QueueEvent = TaskEvent;

/// Thread-safe download queue
#[derive(Clone)]
pub struct DownloadQueue {
    tasks: Arc<DashMap<TaskId, DownloadTask>>,
    event_tx: broadcast::Sender<QueueEvent>,
    work_tx: async_channel::Sender<TaskId>,
    work_rx: async_channel::Receiver<TaskId>,
    cancel_tokens: Arc<Mutex<HashMap<TaskId, Arc<tokio_util::sync::CancellationToken>>>>,
}

impl DownloadQueue {
    pub fn new(concurrency: usize) -> Self {
        let (event_tx, _) = broadcast::channel(1024);
        let (work_tx, work_rx) = async_channel::bounded(concurrency * 4);
        Self {
            tasks: Arc::new(DashMap::new()),
            event_tx,
            work_tx,
            work_rx,
            cancel_tokens: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<QueueEvent> {
        self.event_tx.subscribe()
    }

    pub fn work_receiver(&self) -> async_channel::Receiver<TaskId> {
        self.work_rx.clone()
    }

    pub async fn enqueue(&self, task: DownloadTask) -> Result<TaskId> {
        let id = task.id;
        info!(task_id = %id, url = %task.url, "Enqueuing task");

        let event = QueueEvent::Added {
            task: Box::new(task.clone()),
        };
        self.tasks.insert(id, task);
        let _ = self.event_tx.send(event);

        if let Err(e) = self.work_tx.send(id).await {
            warn!("Work channel full, task {id} will be picked up on next cycle: {e}");
        }

        Ok(id)
    }

    pub fn update_status(&self, id: TaskId, status: TaskStatus) -> Result<()> {
        if let Some(mut task) = self.tasks.get_mut(&id) {
            task.status = status.clone();
            task.updated_at = chrono::Utc::now();

            if status.is_terminal() {
                task.finished_at = Some(chrono::Utc::now());
                self.cancel_tokens.lock().remove(&id);
            }
            if matches!(status, TaskStatus::Downloading | TaskStatus::Processing) && task.started_at.is_none() {
                task.started_at = Some(chrono::Utc::now());
            }

            let _ = self.event_tx.send(QueueEvent::StatusChanged { id, status });
            Ok(())
        } else {
            Err(crate::NexDLError::TaskNotFound(id))
        }
    }

    pub fn update_progress(&self, id: TaskId, progress: TaskProgress) -> Result<()> {
        if let Some(mut task) = self.tasks.get_mut(&id) {
            task.progress = progress.clone();
            task.updated_at = chrono::Utc::now();
            let _ = self.event_tx.send(QueueEvent::ProgressUpdated { id, progress });
            Ok(())
        } else {
            Err(crate::NexDLError::TaskNotFound(id))
        }
    }

    pub fn get(&self, id: &TaskId) -> Option<DownloadTask> {
        self.tasks.get(id).map(|t| t.clone())
    }

    pub fn list(&self) -> Vec<DownloadTask> {
        let mut tasks: Vec<DownloadTask> = self.tasks.iter().map(|t| t.clone()).collect();
        tasks.sort_by(|a, b| {
            b.priority.cmp(&a.priority)
                .then(a.created_at.cmp(&b.created_at))
        });
        tasks
    }

    /// Pause a task — also triggers cancellation token
    pub fn pause(&self, id: TaskId) -> Result<()> {
        if let Some(token) = self.cancel_tokens.lock().get(&id) {
            token.cancel();
        }
        self.update_status(id, TaskStatus::Paused)
    }

    /// Resume a paused task — re-enqueues it
    pub fn resume(&self, id: TaskId) -> Result<()> {
        let task = self.get(&id)
            .ok_or(crate::NexDLError::TaskNotFound(id))?;

        if !matches!(task.status, TaskStatus::Paused) {
            warn!("Cannot resume task {id} — not paused");
            return Ok(());
        }

        // Create new cancel token
        let token = Arc::new(tokio_util::sync::CancellationToken::new());
        self.cancel_tokens.lock().insert(id, token);

        // Reset status to Queued
        self.update_status(id, TaskStatus::Queued)?;

        // Re-send to work channel
        let work_tx = self.work_tx.clone();
        tokio::spawn(async move {
            let _ = work_tx.send(id).await;
        });

        info!(task_id = %id, "Task resumed");
        Ok(())
    }

    /// Cancel a task — triggers cancellation token
    pub fn cancel(&self, id: TaskId) -> Result<()> {
        if let Some(token) = self.cancel_tokens.lock().get(&id) {
            token.cancel();
        }
        self.update_status(id, TaskStatus::Cancelled)
    }

    pub fn remove(&self, id: TaskId) -> Result<()> {
        if let Some((_, task)) = self.tasks.remove(&id) {
            if !task.status.is_terminal() {
                warn!("Removing non-terminal task {id}");
            }
            self.cancel_tokens.lock().remove(&id);
            let _ = self.event_tx.send(QueueEvent::Removed { id });
            Ok(())
        } else {
            Err(crate::NexDLError::TaskNotFound(id))
        }
    }

    /// Get or create a cancellation token for a task
    pub fn get_cancel_token(&self, id: TaskId) -> Arc<tokio_util::sync::CancellationToken> {
        let mut tokens = self.cancel_tokens.lock();
        tokens.entry(id)
            .or_insert_with(|| Arc::new(tokio_util::sync::CancellationToken::new()))
            .clone()
    }

    /// Check if a task's cancellation token has been triggered
    pub fn is_cancelled(&self, id: &TaskId) -> bool {
        self.cancel_tokens.lock()
            .get(id)
            .map(|t| t.is_cancelled())
            .unwrap_or(false)
    }

    /// Get total download speed across all active tasks
    pub fn total_speed(&self) -> u64 {
        self.tasks.iter()
            .filter(|t| matches!(t.status, TaskStatus::Downloading))
            .map(|t| t.progress.speed_bps)
            .sum()
    }

    pub fn stats(&self) -> QueueStats {
        let mut stats = QueueStats::default();
        for task in self.tasks.iter() {
            match &task.status {
                TaskStatus::Queued => stats.queued += 1,
                TaskStatus::Processing | TaskStatus::Downloading | TaskStatus::PostProcessing => {
                    stats.active += 1;
                }
                TaskStatus::Completed { .. } => stats.completed += 1,
                TaskStatus::Failed { .. } => stats.failed += 1,
                TaskStatus::Paused => stats.paused += 1,
                TaskStatus::Cancelled => stats.cancelled += 1,
                TaskStatus::CaptchaRequired { .. } => stats.captcha_required += 1,
            }
        }
        stats.total = self.tasks.len();
        stats
    }
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct QueueStats {
    pub total: usize,
    pub queued: usize,
    pub active: usize,
    pub completed: usize,
    pub failed: usize,
    pub paused: usize,
    pub cancelled: usize,
    pub captcha_required: usize,
}
