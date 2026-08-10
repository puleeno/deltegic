slint::include_modules!();

use deltegic_addon_api::{AddonRegistry, AddonRunner};
use deltegic_accounts::AccountStore;
use deltegic_core::{
    task::{DownloadTask, TaskStatus},
    DownloadQueue,
};
use deltegic_scheduler::Scheduler;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tracing::{error, info, warn};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("deltegic=debug".parse()?)
                .add_directive("warn".parse()?),
        )
        .init();

    info!("Deltegic starting...");

    let rt = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()?,
    );

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("deltegic");

    let addon_dir = {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let c_source = manifest_dir.parent().and_then(|p| p.parent()).unwrap_or(Path::new(".")).join("addons");
        let c1 = exe_dir.join("addons");
        let c2 = exe_dir.parent().unwrap_or(&exe_dir).join("addons");
        let c3 = std::env::current_dir().unwrap_or_default().join("addons");
        // Prefer the source addons dir (has actual .py files) over empty dirs
        fn has_addons(p: &Path) -> bool {
            p.exists() && std::fs::read_dir(p).ok().map(|entries| {
                entries.filter_map(|e| e.ok()).any(|e| {
                    let path = e.path();
                    (path.is_dir() && path.join("__init__.py").exists())
                        || (path.is_file() && path.extension().map(|e| e == "py").unwrap_or(false))
                })
            }).unwrap_or(false)
        }
        if has_addons(&c_source) { c_source }
        else if has_addons(&c1) { c1 }
        else if has_addons(&c2) { c2 }
        else if has_addons(&c3) { c3 }
        else { c_source }
    };
    let downloads_dir = dirs::download_dir()
        .unwrap_or_else(|| PathBuf::from("Downloads"))
        .join("Deltegic");

    std::fs::create_dir_all(&data_dir)?;
    std::fs::create_dir_all(&addon_dir)?;
    std::fs::create_dir_all(&downloads_dir)?;

    let queue = DownloadQueue::new(4);

    let account_store = Arc::new(AccountStore::new(
        data_dir.join("accounts.json"),
        Some("deltegic-passphrase"),
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
    ).with_account_store(account_store.clone()));

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

    ui.set_output_dir(downloads_dir.to_string_lossy().to_string().into());

    // ── Callbacks ─────────────────────────────────────────────────────────────

    // Add URL
    {
        let queue_ref = queue.clone();
        let runner_ref = runner.clone();
        let rt_ref = rt.clone();

        ui.on_add_url(move |url, output_dir| {
            let url = url.to_string();
            let output_dir = output_dir.to_string();

            if url.trim().is_empty() { return; }

            let task = DownloadTask::new(url, output_dir);

            let queue_clone = queue_ref.clone();
            let runner_clone = runner_ref.clone();

            rt_ref.spawn(async move {
                match queue_clone.enqueue(task).await {
                    Ok(id) => {
                        info!("Enqueued {id}");
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

    // Resume task
    {
        let queue_ref = queue.clone();
        let runner_ref = runner.clone();
        let rt_ref = rt.clone();
        ui.on_resume_task(move |id| {
            if let Ok(uuid) = id.parse() {
                let q = queue_ref.clone();
                let r = runner_ref.clone();
                let rt = rt_ref.clone();
                if let Err(e) = q.resume(uuid) {
                    error!("Resume failed: {e}");
                    return;
                }
                rt.spawn(async move {
                    if let Err(e) = r.process_task(uuid).await {
                        error!("Task {uuid} failed after resume: {e}");
                    }
                });
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

    // Add account — creates account from inline UI inputs
    {
        let account_store_ref = account_store.clone();
        let ui_handle = ui.as_weak();
        ui.on_add_account(move || {
            let store = account_store_ref.clone();
            let handle = ui_handle.clone();

            // Open a simple dialog window for account creation
            // For now, create a demo account — full dialog requires Slint popup
            let account = deltegic_accounts::Account::new("New Account", "example.com");
            store.add(account);
            info!("Account added (dialog TODO)");

            // Refresh account list in UI
            if let Some(ui) = handle.upgrade() {
                let list: Vec<AccountData> = store.list().iter().map(|a| AccountData {
                    id: a.id.to_string().into(),
                    label: a.label.clone().into(),
                    site: a.site.clone().into(),
                    username: a.username.clone().unwrap_or_default().into(),
                    cookie_count: a.cookies.count() as i32,
                }).collect();
                ui.set_accounts(list.as_slice().into());
            }
        });
    }

    // Remove account
    {
        let account_store_ref = account_store.clone();
        let ui_handle = ui.as_weak();
        ui.on_remove_account(move |id| {
            if let Ok(uuid) = id.parse() {
                account_store_ref.remove(&uuid);
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

    // Toggle scheduler entry
    {
        let scheduler_ref = scheduler.clone();
        let ui_handle = ui.as_weak();
        ui.on_toggle_sched(move |id, enabled| {
            if let Ok(uuid) = id.parse() {
                scheduler_ref.enable(&uuid, enabled);
                if let Some(ui) = ui_handle.upgrade() {
                    let list: Vec<SchedulerData> = scheduler_ref.list().iter().map(|s| SchedulerData {
                        id: s.id.to_string().into(),
                        name: s.name.clone().into(),
                        trigger: format!("{:?}", s.trigger).into(),
                        action: format!("{:?}", s.action).into(),
                        enabled: s.enabled,
                        last_run: s.last_run.map(|t| t.format("%H:%M:%S").to_string()).unwrap_or_else(|| "--".into()).into(),
                        next_run: s.next_run.map(|t| t.format("%H:%M:%S").to_string()).unwrap_or_else(|| "--".into()).into(),
                        run_count: s.run_count as i32,
                    }).collect();
                    ui.set_scheduler_tasks(list.as_slice().into());
                }
            }
        });
    }

    // Remove scheduler entry
    {
        let scheduler_ref = scheduler.clone();
        let ui_handle = ui.as_weak();
        ui.on_remove_sched(move |id| {
            if let Ok(uuid) = id.parse() {
                scheduler_ref.remove(&uuid);
                if let Some(ui) = ui_handle.upgrade() {
                    let list: Vec<SchedulerData> = scheduler_ref.list().iter().map(|s| SchedulerData {
                        id: s.id.to_string().into(),
                        name: s.name.clone().into(),
                        trigger: format!("{:?}", s.trigger).into(),
                        action: format!("{:?}", s.action).into(),
                        enabled: s.enabled,
                        last_run: s.last_run.map(|t| t.format("%H:%M:%S").to_string()).unwrap_or_else(|| "--".into()).into(),
                        next_run: s.next_run.map(|t| t.format("%H:%M:%S").to_string()).unwrap_or_else(|| "--".into()).into(),
                        run_count: s.run_count as i32,
                    }).collect();
                    ui.set_scheduler_tasks(list.as_slice().into());
                }
            }
        });
    }

    // Select task (show log)
    {
        let queue_ref = queue.clone();
        let ui_handle = ui.as_weak();
        ui.on_select_task(move |id| {
            if let Some(ui) = ui_handle.upgrade() {
                let tasks = queue_ref.list();
                if let Some(task) = tasks.iter().find(|t| id == t.id.to_string()) {
                    let mut log_lines = Vec::new();
                    log_lines.push(format!("URL: {}", task.url));
                    if let Some(addon) = &task.addon {
                        log_lines.push(format!("Addon: {} v{}", addon.name, addon.version));
                    }
                    if !task.progress.message.is_empty() {
                        log_lines.push(format!("Info: {}", task.progress.message));
                    }
                    log_lines.push(format!("Downloaded: {} / {}", task.progress.downloaded, task.progress.total));
                    match &task.status {
                        TaskStatus::Downloading => {
                            log_lines.push("Status: Downloading...".to_string());
                        }
                        TaskStatus::Completed { output_path } => {
                            log_lines.push(format!("Status: Completed → {}", output_path));
                        }
                        TaskStatus::Failed { error, retries } => {
                            log_lines.push(format!("Status: FAILED — {} ({} retries)", error, retries));
                        }
                        TaskStatus::Paused => {
                            log_lines.push("Status: Paused".to_string());
                        }
                        TaskStatus::Cancelled => {
                            log_lines.push("Status: Cancelled".to_string());
                        }
                        s => {
                            log_lines.push(format!("Status: {:?}", s));
                        }
                    }
                    let log_text = log_lines.join("\n");
                    ui.set_selected_task_log(log_text.into());
                }
            }
        });
    }

    // ── Queue → UI sync timer ─────────────────────────────────────────────────
    // IMPORTANT: timer must live until after ui.run() or it gets dropped
    let _sync_timer = {
        let queue_ref = queue.clone();
        let ui_handle = ui.as_weak();

        slint::Timer::default().start(
            slint::TimerMode::Repeated,
            Duration::from_millis(500),
            move || {
                let Some(ui) = ui_handle.upgrade() else { return };

                let tasks = queue_ref.list();
                let stats = queue_ref.stats();
                let total_speed = queue_ref.total_speed();

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

                    let message = t.progress.message.clone();

                    let error = match &t.status {
                        TaskStatus::Failed { error, .. } => error.clone(),
                        _ => String::new(),
                    };

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
                        message: message.into(),
                        error: error.into(),
                    }
                }).collect();

                ui.set_tasks(task_data.as_slice().into());
                ui.set_stat_active(stats.active as i32);
                ui.set_stat_queued(stats.queued as i32);
                ui.set_stat_completed(stats.completed as i32);
                ui.set_stat_failed(stats.failed as i32);
                ui.set_total_speed(format_speed(total_speed).into());
            },
        );
    };

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
