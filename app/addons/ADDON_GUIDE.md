# Deltegic Addon Guide

## Creating an Addon

Create a folder in `~/.local/share/deltegic/addons/` (or the addon directory configured in Settings):

```
my_addon/
└── __init__.py
```

Your `__init__.py` must define a class that inherits from `deltegic.Addon`:

```python
import deltegic

class MyAddon(deltegic.Addon):
    def name(self) -> str:
        return "my-addon"

    def version(self) -> str:
        return "1.0.0"

    def description(self) -> str:
        return "Downloads from my-site.com"

    def supported_sites(self) -> list[str]:
        return ["my-site.com"]

    def can_handle(self, url: str) -> bool:
        return "my-site.com" in url

    def extract(self, ctx: deltegic.Context) -> list[deltegic.DownloadItem]:
        # Fetch the page and find download URLs
        html = ctx.http.get(url)
        # ... parse html, find video/image URLs ...
        item = deltegic.DownloadItem(
            url="https://my-site.com/video.mp4",
            title="My Video",
            filename="my_video.mp4",
            output_path=f"{ctx.output_dir}/my_video.mp4",
        )
        return [item]
```

## API Reference

### `deltegic.Addon` (base class)

Override these methods:

| Method | Required | Description |
|--------|----------|-------------|
| `name() -> str` | Yes | Unique addon identifier |
| `version() -> str` | Yes | Version string |
| `description() -> str` | No | Human-readable description |
| `author() -> str` | No | Author name |
| `supported_sites() -> list[str]` | No | List of domains |
| `can_handle(url: str) -> bool` | Yes | Return True if this addon handles the URL |
| `extract(ctx) -> list[DownloadItem]` | Yes | Return items to download |
| `download(item, ctx)` | No | Override for custom download logic |
| `post_process(item, ctx)` | No | Called after each download (reup, rename, etc.) |
| `resolve_captcha(ctx, captcha_url) -> str` | No | Return cookies/token after captcha solve |

### `deltegic.Context`

```python
ctx.url          # str — the URL being processed
ctx.output_dir   # str — where to save files
ctx.http         # HttpClient — make HTTP requests
ctx.storage      # Storage — read/write files
ctx.options      # dict[str, str] — addon options from task

ctx.report_progress(downloaded: int, total: int, message: str)
ctx.log(level: str, message: str)  # level: debug, info, warn, error
ctx.get_option(key: str, default: str) -> str
```

### `deltegic.HttpClient`

```python
http = ctx.http

# Requests (synchronous — runs on async executor)
html = http.get(url: str) -> str
data = http.get_json(url: str) -> dict/list
text = http.post_json(url: str, body: str) -> str
bytes_written = http.download(url: str, dest: str) -> int

# Configuration
http.set_cookie(cookie_string: str)
http.set_header(key: str, value: str)
http.cookies  # current cookie string
```

### `deltegic.DownloadItem`

```python
item = deltegic.DownloadItem(url, title="", output_path="", filename="")

item.url          # Download URL
item.title        # Human-readable title
item.output_path  # Full path to save the file
item.filename     # Just the filename part
item.size         # File size in bytes (-1 if unknown)
item.mime_type    # MIME type
item.referer      # Referer header
item.cookies      # Cookie string for this item
item.headers      # dict[str, str] extra headers
item.metadata     # dict[str, str] addon-specific data
item.index        # Index in album/playlist (0-based)
item.total        # Total items in set
```

### `deltegic.Storage`

```python
storage = ctx.storage

storage.read_text(path: str) -> str
storage.write_text(path: str, content: str)
storage.read_json(path: str) -> dict/list
storage.write_json(path: str, json_str: str)
storage.exists(path: str) -> bool
storage.list_dir(path: str) -> list[str]
storage.base_dir  # str — absolute base directory
```

## Cookie Management

Cookies are managed per-account in Deltegic. When an addon is invoked for a task
that has an account assigned, the account's cookies are automatically injected
into `ctx.http`.

To import cookies:
1. Open **Accounts** in Deltegic
2. Click **+ Add Account** → paste cookies (Netscape/JSON format)
3. Assign the account to your download tasks

## Addon Lifecycle

```
URL received
     │
     ▼
registry.find_for_url(url)  ← loops through addons, calls can_handle()
     │
     ▼
addon.extract(ctx)          ← returns list[DownloadItem]
     │
     ▼
for each item:
    addon.download(item, ctx)       ← or default HTTP download
    addon.post_process(item, ctx)   ← optional post-processing
     │
     ▼
Task marked complete
```

## Example: API-Based Addon

```python
import deltegic
import json

class MyApiAddon(deltegic.Addon):
    API_BASE = "https://api.my-site.com/v1"

    def name(self): return "my-api-addon"
    def version(self): return "1.0.0"
    def supported_sites(self): return ["my-site.com"]

    def can_handle(self, url: str) -> bool:
        return "my-site.com/video/" in url

    def extract(self, ctx: deltegic.Context) -> list[deltegic.DownloadItem]:
        # Extract video ID from URL
        import re
        m = re.search(r'/video/(\d+)', ctx.url)
        if not m: return []
        video_id = m.group(1)

        # Call API
        ctx.http.set_header("Authorization", f"Bearer {ctx.get_option('api_token', '')}")
        data = ctx.http.get_json(f"{self.API_BASE}/video/{video_id}")

        return [deltegic.DownloadItem(
            url=data["stream_url"],
            title=data["title"],
            filename=f"{data['title']}.mp4",
            output_path=f"{ctx.output_dir}/{data['title']}.mp4",
        )]
```

## Built-in Addons

| Addon | Sites | Notes |
|-------|-------|-------|
| `facebook` | facebook.com, fb.watch | Albums, Reels, Videos, Photos |
| `instagram` | instagram.com | Posts, Reels, Stories, Carousels |
| `youtube` | youtube.com, youtu.be | Uses yt-dlp if available |
| `generic-http` | any | Direct file URLs (.mp4, .zip, etc.) |
| `telegram-bot` | — | Bot interface (scheduler addon) |
| `reup-example` | — | Post-processor example (S3, FTP, rclone) |
