use crate::task::{DownloadTask, TaskEvent, TaskId, TaskPriority, TaskProgress, TaskStatus};
use crate::Result;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Events broadcast from the queue
pub type QueueEvent = TaskEvent;

/// Thread-safe download queue
#[derive(Clone)]
pub struct DownloadQueue {
    tasks: Arc<DashMap<TaskId, DownloadTask>>,
    event_tx: broadcast::Sender<QueueEvent>,
    /// Channel for workers to receive new work
    work_tx: async_channel::Sender<TaskId>,
    work_rx: async_channel::Receiver<TaskId>,
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
        }
    }

    /// Subscribe to queue events
    pub fn subscribe(&self) -> broadcast::Receiver<QueueEvent> {
        self.event_tx.subscribe()
    }

    /// Subscribe to work items (for download workers)
    pub fn work_receiver(&self) -> async_channel::Receiver<TaskId> {
        self.work_rx.clone()
    }

    /// Add a task to the queue
    pub async fn enqueue(&self, task: DownloadTask) -> Result<TaskId> {
        let id = task.id;
        info!(task_id = %id, url = %task.url, "Enqueuing task");

        let event = QueueEvent::Added {
            task: Box::new(task.clone()),
        };
        self.tasks.insert(id, task);
        let _ = self.event_tx.send(event);

        // Send to work channel
        if let Err(e) = self.work_tx.send(id).await {
            warn!("Work channel full, task {id} will be picked up on next cycle: {e}");
        }

        Ok(id)
    }

    /// Update the status of a task
    pub fn update_status(&self, id: TaskId, status: TaskStatus) -> Result<()> {
        if let Some(mut task) = self.tasks.get_mut(&id) {
            task.status = status.clone();
            task.updated_at = chrono::Utc::now();

            if status.is_terminal() {
                task.finished_at = Some(chrono::Utc::now());
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

    /// Update progress of a task
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

    /// Get a task by ID
    pub fn get(&self, id: &TaskId) -> Option<DownloadTask> {
        self.tasks.get(id).map(|t| t.clone())
    }

    /// Get all tasks, optionally filtered by status
    pub fn list(&self) -> Vec<DownloadTask> {
        let mut tasks: Vec<DownloadTask> = self.tasks.iter().map(|t| t.clone()).collect();
        tasks.sort_by(|a, b| {
            b.priority.cmp(&a.priority)
                .then(a.created_at.cmp(&b.created_at))
        });
        tasks
    }

    /// Pause a task
    pub fn pause(&self, id: TaskId) -> Result<()> {
        self.update_status(id, TaskStatus::Paused)
    }

    /// Cancel a task
    pub fn cancel(&self, id: TaskId) -> Result<()> {
        self.update_status(id, TaskStatus::Cancelled)
    }

    /// Remove a completed/failed/cancelled task
    pub fn remove(&self, id: TaskId) -> Result<()> {
        if let Some((_, task)) = self.tasks.remove(&id) {
            if !task.status.is_terminal() {
                warn!("Removing non-terminal task {id}");
            }
            let _ = self.event_tx.send(QueueEvent::Removed { id });
            Ok(())
        } else {
            Err(crate::NexDLError::TaskNotFound(id))
        }
    }

    /// Get queue statistics
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
