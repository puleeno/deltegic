use crate::{
    python_types::{PyContext, PyDownloadItem},
    registry::AddonRegistry,
    AddonApiError, Result,
};
use deltegic_core::{
    task::{TaskId, TaskProgress, TaskStatus},
    DownloadQueue, HttpDownloader,
};
use deltegic_accounts::AccountStore;
use pyo3::prelude::*;
use std::{path::PathBuf, sync::Arc};
use tracing::{error, info, warn};

pub struct AddonRunner {
    registry: Arc<AddonRegistry>,
    queue: DownloadQueue,
    default_output_dir: PathBuf,
    account_store: Option<Arc<AccountStore>>,
}

impl AddonRunner {
    pub fn new(registry: Arc<AddonRegistry>, queue: DownloadQueue, output_dir: PathBuf) -> Self {
        Self {
            registry,
            queue,
            default_output_dir: output_dir,
            account_store: None,
        }
    }

    pub fn with_account_store(mut self, store: Arc<AccountStore>) -> Self {
        self.account_store = Some(store);
        self
    }

    pub async fn process_task(&self, task_id: TaskId) -> Result<()> {
        let task = match self.queue.get(&task_id) {
            Some(t) => t,
            None => return Err(AddonApiError::Other(format!("Task {} not found", task_id))),
        };

        let token = self.queue.get_cancel_token(task_id);

        if token.is_cancelled() || matches!(task.status, TaskStatus::Cancelled) {
            return Ok(());
        }

        if url::Url::parse(&task.url).is_err() {
            self.queue.update_status(task_id, TaskStatus::Failed {
                error: format!("Invalid URL: {}", task.url),
                retries: 0,
            }).ok();
            return Err(AddonApiError::Execution(format!("Invalid URL: {}", task.url)));
        }

        let addon_name = self.registry.find_for_url(&task.url);

        let (url, output_dir) = (task.url.clone(), task.output_dir.clone());

        let output_dir = if output_dir.is_empty() {
            self.default_output_dir.to_string_lossy().to_string()
        } else {
            output_dir
        };

        let account_cookies = self.account_store.as_ref().and_then(|store| {
            store.find_for_url(&url).map(|acc| {
                let domain = url::Url::parse(&url)
                    .ok()
                    .and_then(|u| u.domain().map(|s| s.to_string()))
                    .unwrap_or_default();
                acc.cookie_header(&domain)
            })
        });

        let addon_name = match addon_name {
            Some(name) => name,
            None => {
                self.queue.update_status(task_id, TaskStatus::Downloading).ok();
                return self.direct_download(task_id, &url, &output_dir, &token).await;
            }
        };

        info!(task_id = %task_id, addon = %addon_name, "Processing with addon");
        self.queue.update_status(task_id, TaskStatus::Processing).ok();

        let max_retries = task.max_retries;
        let mut last_error = String::new();

        for attempt in 0..=max_retries {
            if token.is_cancelled() {
                self.queue.update_status(task_id, TaskStatus::Cancelled).ok();
                return Ok(());
            }

            if attempt > 0 {
                info!(task_id = %task_id, attempt, "Retrying extraction");
                self.queue.update_progress(task_id, TaskProgress {
                    message: format!("Retry {}/{}", attempt, max_retries),
                    ..Default::default()
                }).ok();
                tokio::time::sleep(tokio::time::Duration::from_secs(2u64.pow(attempt - 1))).await;
            }

            match self.extract_and_download(
                task_id, &url, &output_dir, &addon_name, &account_cookies, &token,
            ).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last_error = e.to_string();
                    warn!(task_id = %task_id, attempt, "Failed: {}", last_error);
                }
            }
        }

        error!(task_id = %task_id, "All {} retries exhausted: {}", max_retries, last_error);
        self.queue.update_status(task_id, TaskStatus::Failed {
            error: last_error,
            retries: max_retries,
        }).ok();

        Ok(())
    }

    async fn extract_and_download(
        &self,
        task_id: TaskId,
        url: &str,
        output_dir: &str,
        addon_name: &str,
        account_cookies: &Option<String>,
        token: &tokio_util::sync::CancellationToken,
    ) -> Result<()> {
        let registry = self.registry.clone();
        let url_owned = url.to_string();
        let output_dir_owned = output_dir.to_string();
        let addon_name_owned = addon_name.to_string();
        let cookies_clone = account_cookies.clone();

        // Run Python extraction in blocking thread
        let items: Vec<ExtractedItem> = tokio::task::spawn_blocking(move || {
            Python::with_gil(|py| -> std::result::Result<Vec<ExtractedItem>, AddonApiError> {
                let instance = registry.instantiate(&addon_name_owned)
                    .map_err(|e| AddonApiError::Execution(e.to_string()))?;
                let bound = instance.bind(py);

                let ctx = PyContext::create(url_owned.clone(), output_dir_owned.clone());
                let ctx_obj = Py::new(py, ctx)
                    .map_err(|e| AddonApiError::Python(e.to_string()))?;

                if let Some(ref cookies) = cookies_clone {
                    ctx_obj.borrow_mut(py).set_cookies(cookies);
                }

                let items_py = bound.call_method1("extract", (ctx_obj,))
                    .map_err(|e| AddonApiError::Execution(format!("extract(): {e}")))?;

                let py_items: Vec<PyDownloadItem> = items_py.extract()
                    .map_err(|e| AddonApiError::TypeError(format!("extract() return type: {e}")))?;

                Ok(py_items.into_iter().map(|item| ExtractedItem {
                    url: item.url,
                    title: item.title,
                    output_path: item.output_path,
                    filename: item.filename,
                }).collect())
            })
        }).await.map_err(|e| AddonApiError::Execution(e.to_string()))??;

        if items.is_empty() {
            warn!(task_id = %task_id, "Addon extracted 0 items — task may need login cookies or URL is not supported");
            self.queue.update_status(task_id, TaskStatus::Failed {
                error: "Addon extracted 0 items. Try adding a logged-in account in the Accounts tab.".to_string(),
                retries: 0,
            }).ok();
            return Ok(());
        }

        let total = items.len() as u32;
        self.queue.update_status(task_id, TaskStatus::Downloading).ok();

        for (i, item) in items.iter().enumerate() {
            if token.is_cancelled() {
                self.queue.update_status(task_id, TaskStatus::Cancelled).ok();
                return Ok(());
            }

            info!(task_id = %task_id, "Downloading item {}/{}: {}", i + 1, total, item.title);

            let _ = self.queue.update_progress(task_id, TaskProgress {
                message: format!("Downloading: {}", item.title),
                subtasks_done: i as u32,
                subtasks_total: total,
                ..Default::default()
            });

            let dl = HttpDownloader::default();
            let output_path = if item.output_path.is_empty() {
                std::path::Path::new(output_dir).join(&item.filename)
            } else {
                std::path::PathBuf::from(&item.output_path)
            };

            let queue_ref = self.queue.clone();
            let item_url = item.url.clone();
            let token_clone = token.clone();

            dl.download_to_file(
                &item_url,
                &output_path,
                None,
                move |downloaded, total_bytes| {
                    if token_clone.is_cancelled() {
                        return;
                    }
                    let _ = queue_ref.update_progress(task_id, TaskProgress {
                        downloaded,
                        total: total_bytes,
                        message: format!("Item {}/{}", i + 1, total),
                        subtasks_done: i as u32,
                        subtasks_total: total,
                        ..Default::default()
                    });
                },
            ).await.map_err(|e| AddonApiError::Execution(e.to_string()))?;
        }

        self.queue.update_status(task_id, TaskStatus::PostProcessing).ok();

        // Run post_process
        let registry = self.registry.clone();
        let output_dir_clone = output_dir.to_string();
        let url_clone = url.to_string();
        let addon_name_clone = addon_name.to_string();
        let items_clone: Vec<ExtractedItem> = items.iter().cloned().collect();

        tokio::task::spawn_blocking(move || {
            Python::with_gil(|py| {
                if let Ok(instance) = registry.instantiate(&addon_name_clone) {
                    let bound = instance.bind(py);
                    for item in &items_clone {
                        let ctx = PyContext::create(url_clone.clone(), output_dir_clone.clone());
                        let ctx_py = Py::new(py, ctx).unwrap();
                        let dl_item = PyDownloadItem::new(&item.url, &item.title, &item.output_path, &item.filename);
                        let _ = bound.call_method1("post_process", (dl_item, ctx_py));
                    }
                }
            });
        }).await.ok();

        self.queue.update_status(task_id, TaskStatus::Completed {
            output_path: output_dir.to_string(),
        }).ok();

        info!(task_id = %task_id, "Task completed");
        Ok(())
    }

    async fn direct_download(
        &self,
        task_id: TaskId,
        url: &str,
        output_dir: &str,
        token: &tokio_util::sync::CancellationToken,
    ) -> Result<()> {
        let dl = HttpDownloader::default();
        let filename = {
            let last_segment = url.split('/').last().unwrap_or("download");
            let name = last_segment.split('?').next().unwrap_or(last_segment);
            let sanitized: String = name.chars().map(|c| match c {
                '<' | '>' | '"' | '/' | '\\' | '|' | ':' | '?' | '*' => '_',
                _ => c,
            }).collect();
            if sanitized.is_empty() { "download".to_string() } else { sanitized }
        };
        let out_dir = if output_dir.is_empty() {
            self.default_output_dir.to_string_lossy().to_string()
        } else {
            output_dir.to_string()
        };
        let output_path = std::path::Path::new(&out_dir).join(&filename);
        let queue_ref = self.queue.clone();
        let token_clone = token.clone();

        dl.download_to_file(url, &output_path, None, move |downloaded, total| {
            if token_clone.is_cancelled() {
                return;
            }
            let _ = queue_ref.update_progress(task_id, TaskProgress {
                downloaded, total,
                message: "Downloading...".to_string(),
                ..Default::default()
            });
        }).await.map_err(|e| AddonApiError::Execution(e.to_string()))?;

        if token.is_cancelled() {
            self.queue.update_status(task_id, TaskStatus::Cancelled).ok();
        } else {
            self.queue.update_status(task_id, TaskStatus::Completed {
                output_path: output_path.to_string_lossy().to_string(),
            }).ok();
        }
        Ok(())
    }
}

#[derive(Clone)]
struct ExtractedItem {
    url: String,
    title: String,
    output_path: String,
    filename: String,
}
