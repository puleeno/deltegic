slint::include_modules!();

use nexdl_addon_api::{AddonRegistry, AddonRunner};
use nexdl_accounts::AccountStore;
use nexdl_core::{
    task::{DownloadTask, TaskStatus},
    DownloadQueue,
};
use nexdl_scheduler::Scheduler;
use std::{
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tokio::runtime::Runtime;
use tracing::{error, info, warn};

fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("nexdl=debug".parse()?)
                .add_directive("warn".parse()?),
        )
        .init();

    info!("NexDL starting...");

    // Tokio runtime for async work
    let rt = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()?,
    );

    // Determine data directories
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nexdl");
    let addon_dir = data_dir.join("addons");
    let downloads_dir = dirs::download_dir()
        .unwrap_or_else(|| PathBuf::from("Downloads"))
        .join("NexDL");

    std::fs::create_dir_all(&data_dir)?;
    std::fs::create_dir_all(&addon_dir)?;
    std::fs::create_dir_all(&downloads_dir)?;

    // ── Core components ──────────────────────────────────────────────────────
    let queue = DownloadQueue::new(4);

    let account_store = Arc::new(AccountStore::new(
        data_dir.join("accounts.json"),
        Some("nexdl-passphrase"), // TODO: user-set passphrase from settings
    ));
    rt.block_on(account_store.load()).unwrap_or_else(|e| warn!("Account load: {e}"));

    let registry = Arc::new(AddonRegistry::new(addon_dir.clone()));
    if let Err(e) = registry.init_python() {
        error!("Python init failed: {e}");
    } else {
        let count = registry.load_all().unwrap_or(0);
        info!("Loaded {count} addons from {:?}", addon_dir);
    }

    let runner = Arc::new(AddonRunner::new(
        registry.clone(),
        queue.clone(),
        downloads_dir.clone(),
    ));

    let scheduler = Arc::new(Scheduler::new());
    rt.spawn(scheduler.clone().run());

    // ── Slint UI ─────────────────────────────────────────────────────────────
    let ui = MainWindow::new()?;

    // Populate initial addon list
    {
        let addon_list: Vec<AddonData> = registry.list().iter().map(|a| AddonData {
            name: a.name.clone().into(),
            version: a.version.clone().into(),
            description: a.description.clone().into(),
            author: a.author.clone().into(),
            enabled: a.enabled,
            sites: a.supported_sites.join(", ").into(),
        }).collect();
        ui.set_addons(addon_list.as_slice().into());
    }

    // Populate accounts
    {
        let account_list: Vec<AccountData> = account_store.list().iter().map(|a| AccountData {
            id: a.id.to_string().into(),
            label: a.label.clone().into(),
            site: a.site.clone().into(),
            username: a.username.clone().unwrap_or_default().into(),
            cookie_count: a.cookies.count() as i32,
        }).collect();
        ui.set_accounts(account_list.as_slice().into());
    }

    // Default output dir
    ui.set_output_dir(downloads_dir.to_string_lossy().to_string().into());

    // ── Callbacks ─────────────────────────────────────────────────────────────

    // Add URL
    {
        let queue_ref = queue.clone();
        let runner_ref = runner.clone();
        let rt_ref = rt.clone();
        let ui_handle = ui.as_weak();

        ui.on_add_url(move |url, output_dir| {
            let url = url.to_string();
            let output_dir = output_dir.to_string();

            if url.trim().is_empty() { return; }

            let task = DownloadTask::new(url.clone(), output_dir);
            let task_id = task.id;

            let queue_clone = queue_ref.clone();
            let runner_clone = runner_ref.clone();
            let ui_w = ui_handle.clone();

            rt_ref.spawn(async move {
                match queue_clone.enqueue(task).await {
                    Ok(id) => {
                        info!("Enqueued {id}");
                        // Start processing
                        let runner = runner_clone.clone();
                        tokio::spawn(async move {
                            if let Err(e) = runner.process_task(id).await {
                                error!("Task {id} failed: {e}");
                            }
                        });
                    }
                    Err(e) => error!("Enqueue failed: {e}"),
                }
            });
        });
    }

    // Pause task
    {
        let queue_ref = queue.clone();
        ui.on_pause_task(move |id| {
            if let Ok(uuid) = id.parse() {
                let _ = queue_ref.pause(uuid);
            }
        });
    }

    // Cancel task
    {
        let queue_ref = queue.clone();
        ui.on_cancel_task(move |id| {
            if let Ok(uuid) = id.parse() {
                let _ = queue_ref.cancel(uuid);
            }
        });
    }

    // Remove task
    {
        let queue_ref = queue.clone();
        ui.on_remove_task(move |id| {
            if let Ok(uuid) = id.parse() {
                let _ = queue_ref.remove(uuid);
            }
        });
    }

    // Toggle addon
    {
        let registry_ref = registry.clone();
        ui.on_toggle_addon(move |name, enabled| {
            registry_ref.enable(&name, enabled);
        });
    }

    // Reload addon
    {
        let registry_ref = registry.clone();
        ui.on_reload_addon(move |name| {
            if let Err(e) = registry_ref.reload(&name) {
                error!("Reload addon {name}: {e}");
            }
        });
    }

    // Open captcha window
    {
        ui.on_open_captcha_window(move || {
            if let Ok(captcha_win) = CaptchaWindow::new() {
                captcha_win.set_site_name("Manual Captcha".into());
                captcha_win.set_message("Solve the captcha in the browser, then click Continue.".into());
                captcha_win.on_open_browser(move || {
                    // TODO: spawn headless browser
                    info!("Opening browser for captcha...");
                });
                captcha_win.on_skip(move || {
                    info!("Captcha skipped");
                });
                captcha_win.on_cancel(move || {
                    info!("Captcha cancelled");
                });
                let _ = captcha_win.show();
            }
        });
    }

    // Add account (stub — opens a dialog in full impl)
    {
        let account_store_ref = account_store.clone();
        let ui_handle = ui.as_weak();
        ui.on_add_account(move || {
            info!("Add account triggered (UI dialog TODO)");
        });
    }

    // Remove account
    {
        let account_store_ref = account_store.clone();
        let ui_handle = ui.as_weak();
        ui.on_remove_account(move |id| {
            if let Ok(uuid) = id.parse() {
                account_store_ref.remove(&uuid);
                // Refresh list
                if let Some(ui) = ui_handle.upgrade() {
                    let list: Vec<AccountData> = account_store_ref.list().iter().map(|a| AccountData {
                        id: a.id.to_string().into(),
                        label: a.label.clone().into(),
                        site: a.site.clone().into(),
                        username: a.username.clone().unwrap_or_default().into(),
                        cookie_count: a.cookies.count() as i32,
                    }).collect();
                    ui.set_accounts(list.as_slice().into());
                }
            }
        });
    }

    // ── Queue → UI sync timer ─────────────────────────────────────────────────
    {
        let queue_ref = queue.clone();
        let ui_handle = ui.as_weak();

        slint::Timer::default().start(
            slint::TimerMode::Repeated,
            Duration::from_millis(500),
            move || {
                let Some(ui) = ui_handle.upgrade() else { return };

                let tasks = queue_ref.list();
                let stats = queue_ref.stats();

                let task_data: Vec<TaskData> = tasks.iter().map(|t| {
                    let status_str = match &t.status {
                        TaskStatus::Queued => "queued",
                        TaskStatus::Processing => "processing",
                        TaskStatus::Downloading => "downloading",
                        TaskStatus::PostProcessing => "post_processing",
                        TaskStatus::Paused => "paused",
                        TaskStatus::Completed { .. } => "completed",
                        TaskStatus::Failed { .. } => "failed",
                        TaskStatus::Cancelled => "cancelled",
                        TaskStatus::CaptchaRequired { .. } => "captcha_required",
                    };

                    let speed = format_speed(t.progress.speed_bps);
                    let eta = t.progress.eta_secs
                        .map(format_eta)
                        .unwrap_or_else(|| "--".to_string());

                    TaskData {
                        id: t.id.to_string().into(),
                        title: t.title.clone().unwrap_or_default().into(),
                        url: t.url.clone().into(),
                        status: status_str.into(),
                        progress: t.progress.percent() as f32,
                        speed: speed.into(),
                        eta: eta.into(),
                        addon_name: t.addon.as_ref()
                            .map(|a| a.name.clone())
                            .unwrap_or_default()
                            .into(),
                    }
                }).collect();

                ui.set_tasks(task_data.as_slice().into());
                ui.set_stat_active(stats.active as i32);
                ui.set_stat_queued(stats.queued as i32);
                ui.set_stat_completed(stats.completed as i32);
                ui.set_stat_failed(stats.failed as i32);
            },
        );
    }

    ui.run()?;
    Ok(())
}

fn format_speed(bps: u64) -> String {
    if bps == 0 { return "0 B/s".to_string(); }
    if bps < 1024 { return format!("{} B/s", bps); }
    if bps < 1024 * 1024 { return format!("{:.1} KB/s", bps as f64 / 1024.0); }
    format!("{:.1} MB/s", bps as f64 / (1024.0 * 1024.0))
}

fn format_eta(secs: u64) -> String {
    if secs < 60 { return format!("{}s", secs); }
    if secs < 3600 { return format!("{}m {}s", secs / 60, secs % 60); }
    format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
}
