# Deltegic

Download manager with an addon system similar to JDownloader, written in Rust with Python addons via PyO3, and a native Slint GUI.

## Run & Operate

### Build the app
```bash
cd app && cargo build --release
```

### Run (requires display server for Slint)
```bash
cd app && cargo run --bin deltegic
```

### Compile-check without building binary
```bash
cd app && cargo check
```

### Run Python type checks on addons
```bash
python3 -m py_compile app/addons/facebook/__init__.py
```

## Stack

- **Language**: Rust (stable 1.88)
- **GUI**: Slint 1.9 (native desktop window)
- **Async**: Tokio multi-thread
- **Python addon API**: PyO3 0.22 (embeds Python 3.12 in Rust)
- **HTTP**: reqwest with rustls-tls
- **Crypto**: AES-256-GCM (account/cookie encryption)
- **Scheduling**: Custom Tokio-based scheduler

## Architecture

```
app/
├── Cargo.toml                # Workspace
├── crates/
│   ├── nexdl-core/           # Download queue, task types, HTTP downloader
│   ├── nexdl-addon-api/      # PyO3 bridge — Python addon API (nexdl module)
│   ├── nexdl-accounts/       # Account + cookie store (AES-256-GCM encrypted)
│   ├── nexdl-scheduler/      # Cron/interval task scheduler
│   └── nexdl-gui/            # Slint UI (main window + captcha window)
└── addons/                   # Built-in Python addons
    ├── facebook/             # Facebook: albums, reels, videos, photos
    ├── instagram/            # Instagram: posts, reels, stories, carousels
    ├── youtube/              # YouTube: via yt-dlp subprocess
    ├── generic_http/         # Direct file URLs (fallback)
    ├── telegram_bot/         # Telegram bot interface
    └── reup_example/         # Re-upload post-processor example
```

## Writing Addons

See `app/addons/ADDON_GUIDE.md` for the full addon API reference.

Quick example:
```python
import nexdl

class MyAddon(nexdl.Addon):
    def name(self): return "my-addon"
    def can_handle(self, url): return "my-site.com" in url
    def extract(self, ctx) -> list[nexdl.DownloadItem]:
        html = ctx.http.get(ctx.url)
        return [nexdl.DownloadItem(url="...", title="...", filename="file.mp4", output_path=f"{ctx.output_dir}/file.mp4")]
```

Drop the folder in `~/.local/share/nexdl/addons/` — NexDL loads it on startup or via **Reload All** in the Addons tab.

## Addon API (nexdl Python module)

| Class | Purpose |
|-------|---------|
| `nexdl.Addon` | Base class for all addons |
| `nexdl.Context` | Execution context (url, output_dir, http, storage, options) |
| `nexdl.DownloadItem` | Represents one file to download |
| `nexdl.HttpClient` | HTTP GET/POST/download with cookie support |
| `nexdl.Storage` | Read/write files from the addon's storage dir |

## Account & Cookie Management

Accounts are stored encrypted (AES-256-GCM) in `~/.local/share/nexdl/accounts.json`.
Import cookies via the Accounts tab — supports Netscape cookie format and browser JSON export (EditThisCookie).

## Captcha Handling

When an addon raises a captcha challenge (`TaskStatus::CaptchaRequired`), a separate Slint window opens notifying the user. The user clicks "Open Browser" to interact with the headless browser, completes the captcha, and the task continues automatically.

## User preferences

_Populate as you build._
