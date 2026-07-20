use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};
use uuid::Uuid;

pub type ScheduleId = Uuid;

/// When a scheduled task should trigger
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum TriggerSpec {
    /// Run once at a specific time
    Once { at: DateTime<Utc> },
    /// Run at a fixed interval
    Interval { seconds: u64 },
    /// Simple cron expression (minute hour day month weekday)
    Cron { expression: String },
    /// Run on every app startup
    OnStartup,
}

/// What action a scheduled item performs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ScheduledAction {
    /// Download a URL via the addon system
    Download {
        url: String,
        output_dir: String,
        account_id: Option<Uuid>,
        options: serde_json::Value,
    },
    /// Run a Python addon directly
    RunAddon {
        addon_name: String,
        method: String,
        args: serde_json::Value,
    },
    /// Reup: re-upload downloaded files somewhere
    Reup {
        source_dir: String,
        destination: String,
        addon_name: String,
    },
    /// Cleanup old files
    Cleanup {
        directory: String,
        older_than_days: u32,
        pattern: String,
    },
}

/// A scheduled item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleEntry {
    pub id: ScheduleId,
    pub name: String,
    pub description: String,
    pub trigger: TriggerSpec,
    pub action: ScheduledAction,
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: Option<DateTime<Utc>>,
    pub run_count: u64,
    pub created_at: DateTime<Utc>,
}

impl ScheduleEntry {
    pub fn new(name: impl Into<String>, trigger: TriggerSpec, action: ScheduledAction) -> Self {
        let now = Utc::now();
        let next_run = Self::compute_next(&trigger, None);
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            description: String::new(),
            trigger,
            action,
            enabled: true,
            last_run: None,
            next_run,
            run_count: 0,
            created_at: now,
        }
    }

    fn compute_next(trigger: &TriggerSpec, last: Option<DateTime<Utc>>) -> Option<DateTime<Utc>> {
        let now = Utc::now();
        match trigger {
            TriggerSpec::Once { at } => {
                if *at > now { Some(*at) } else { None }
            }
            TriggerSpec::Interval { seconds } => {
                let base = last.unwrap_or(now);
                Some(base + Duration::seconds(*seconds as i64))
            }
            TriggerSpec::Cron { expression } => {
                // Simple cron: next_minute for now, full cron needs a library
                Some(now + Duration::minutes(1))
            }
            TriggerSpec::OnStartup => None,
        }
    }

    pub fn advance(&mut self) {
        let now = Utc::now();
        self.last_run = Some(now);
        self.run_count += 1;
        self.next_run = Self::compute_next(&self.trigger, Some(now));
    }

    pub fn is_due(&self) -> bool {
        if !self.enabled { return false; }
        self.next_run.map(|t| t <= Utc::now()).unwrap_or(false)
    }
}

/// Event when a scheduled item fires
#[derive(Debug, Clone)]
pub struct SchedulerEvent {
    pub entry: ScheduleEntry,
}

/// The scheduler — checks entries and fires them
pub struct Scheduler {
    entries: Arc<DashMap<ScheduleId, ScheduleEntry>>,
    event_tx: broadcast::Sender<SchedulerEvent>,
}

impl Scheduler {
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self {
            entries: Arc::new(DashMap::new()),
            event_tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SchedulerEvent> {
        self.event_tx.subscribe()
    }

    pub fn add(&self, entry: ScheduleEntry) -> ScheduleId {
        let id = entry.id;
        self.entries.insert(id, entry);
        id
    }

    pub fn remove(&self, id: &ScheduleId) -> Option<ScheduleEntry> {
        self.entries.remove(id).map(|(_, e)| e)
    }

    pub fn list(&self) -> Vec<ScheduleEntry> {
        self.entries.iter().map(|e| e.clone()).collect()
    }

    pub fn enable(&self, id: &ScheduleId, enabled: bool) {
        if let Some(mut entry) = self.entries.get_mut(id) {
            entry.enabled = enabled;
        }
    }

    /// Run the scheduler loop — call this in a background task
    pub async fn run(self: Arc<Self>) {
        info!("Scheduler started");
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            self.tick().await;
        }
    }

    async fn tick(&self) {
        let due_ids: Vec<ScheduleId> = self.entries
            .iter()
            .filter(|e| e.is_due())
            .map(|e| e.id)
            .collect();

        for id in due_ids {
            if let Some(mut entry) = self.entries.get_mut(&id) {
                info!(name = %entry.name, "Scheduler firing task");
                entry.advance();
                let _ = self.event_tx.send(SchedulerEvent {
                    entry: entry.clone(),
                });
            }
        }
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}
