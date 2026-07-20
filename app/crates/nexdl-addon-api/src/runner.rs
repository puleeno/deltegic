use crate::{
    python_types::{PyContext, PyDownloadItem},
    registry::AddonRegistry,
    AddonApiError, Result,
};
use nexdl_core::{
    task::{TaskId, TaskProgress, TaskStatus},
    DownloadQueue, HttpDownloader,
};
use pyo3::prelude::*;
use std::{path::PathBuf, sync::Arc};
use tracing::{error, info, warn};

/// Runs Python addons against download tasks
pub struct AddonRunner {
    registry: Arc<AddonRegistry>,
    queue: DownloadQueue,
    default_output_dir: PathBuf,
}

impl AddonRunner {
    pub fn new(registry: Arc<AddonRegistry>, queue: DownloadQueue, output_dir: PathBuf) -> Self {
        Self { registry, queue, default_output_dir: output_dir }
    }

    pub async fn process_task(&self, task_id: TaskId) -> Result<()> {
        let task = self.queue.get(&task_id)
            .ok_or_else(|| AddonApiError::Other(format!("Task {} not found", task_id)))?;

        let addon_name = match self.registry.find_for_url(&task.url) {
            Some(name) => name,
            None => {
                self.queue.update_status(task_id, TaskStatus::Downloading).ok();
                return self.direct_download(task_id, &task.url, &task.output_dir).await;
            }
        };

        info!(task_id = %task_id, addon = %addon_name, "Processing with addon");
        self.queue.update_status(task_id, TaskStatus::Processing).ok();

        let output_dir = if task.output_dir.is_empty() {
            self.default_output_dir.to_string_lossy().to_string()
        } else {
            task.output_dir.clone()
        };

        let registry = self.registry.clone();
        let url = task.url.clone();
        let output_dir_clone = output_dir.clone();
        let addon_name_clone = addon_name.clone();

        // Run Python extraction in blocking thread
        let items: Vec<ExtractedItem> = tokio::task::spawn_blocking(move || {
            Python::with_gil(|py| {
                let instance = registry.instantiate(&addon_name_clone)
                    .map_err(|e| AddonApiError::Execution(e.to_string()))?;
                let bound = instance.bind(py);

                let ctx = PyContext::create(url.clone(), output_dir_clone.clone());
                let ctx_py = Py::new(py, ctx)
                    .map_err(|e| AddonApiError::Python(e.to_string()))?;

                let items_py = bound.call_method1("extract", (ctx_py,))
                    .map_err(|e| AddonApiError::Execution(format!("extract(): {e}")))?;

                let py_items: Vec<PyDownloadItem> = items_py.extract()
                    .map_err(|e| AddonApiError::TypeError(format!("extract() return type: {e}")))?;

                Ok::<Vec<ExtractedItem>, AddonApiError>(
                    py_items.into_iter().enumerate().map(|(i, item)| ExtractedItem {
                        url: item.url,
                        title: item.title,
                        output_path: item.output_path,
                        filename: item.filename,
                        index: i,
                    }).collect()
                )
            })
        }).await.map_err(|e| AddonApiError::Execution(e.to_string()))??;

        if items.is_empty() {
            warn!(task_id = %task_id, "Addon extracted 0 items");
        }

        let total = items.len() as u32;
        self.queue.update_status(task_id, TaskStatus::Downloading).ok();

        for (i, item) in items.iter().enumerate() {
            info!(task_id = %task_id, "Downloading item {}/{}: {}", i + 1, total, item.title);

            let _ = self.queue.update_progress(task_id, TaskProgress {
                message: format!("Downloading: {}", item.title),
                subtasks_done: i as u32,
                subtasks_total: total,
                ..Default::default()
            });

            let dl = HttpDownloader::default();
            let output_path = if item.output_path.is_empty() {
                std::path::Path::new(&output_dir).join(&item.filename)
            } else {
                std::path::PathBuf::from(&item.output_path)
            };

            let queue_ref = self.queue.clone();
            let item_url = item.url.clone();

            dl.download_to_file(
                &item_url,
                &output_path,
                None,
                move |downloaded, total_bytes| {
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

        // Run post_process for each item
        let registry = self.registry.clone();
        let output_dir_clone2 = output_dir.clone();
        let url_clone = task.url.clone();
        let addon_name_clone2 = addon_name.clone();
        let items_clone: Vec<ExtractedItem> = items.iter().cloned().collect();

        tokio::task::spawn_blocking(move || {
            Python::with_gil(|py| {
                if let Ok(instance) = registry.instantiate(&addon_name_clone2) {
                    let bound = instance.bind(py);
                    for item in &items_clone {
                        let ctx = PyContext::create(url_clone.clone(), output_dir_clone2.clone());
                        let ctx_py = Py::new(py, ctx).unwrap();
                        let dl_item = PyDownloadItem::new(&item.url, &item.title, &item.output_path, &item.filename);
                        let _ = bound.call_method1("post_process", (dl_item, ctx_py));
                    }
                }
            });
        }).await.ok();

        self.queue.update_status(task_id, TaskStatus::Completed {
            output_path: output_dir.clone(),
        }).ok();

        info!(task_id = %task_id, "Task completed");
        Ok(())
    }

    async fn direct_download(&self, task_id: TaskId, url: &str, output_dir: &str) -> Result<()> {
        let dl = HttpDownloader::default();
        let filename = url.split('/').last().unwrap_or("download").to_string();
        let out_dir = if output_dir.is_empty() {
            self.default_output_dir.to_string_lossy().to_string()
        } else {
            output_dir.to_string()
        };
        let output_path = std::path::Path::new(&out_dir).join(&filename);
        let queue_ref = self.queue.clone();

        dl.download_to_file(url, &output_path, None, move |downloaded, total| {
            let _ = queue_ref.update_progress(task_id, TaskProgress {
                downloaded, total,
                message: "Downloading...".to_string(),
                ..Default::default()
            });
        }).await.map_err(|e| AddonApiError::Execution(e.to_string()))?;

        self.queue.update_status(task_id, TaskStatus::Completed {
            output_path: output_path.to_string_lossy().to_string(),
        }).ok();
        Ok(())
    }
}

#[derive(Clone)]
struct ExtractedItem {
    url: String,
    title: String,
    output_path: String,
    filename: String,
    index: usize,
}
