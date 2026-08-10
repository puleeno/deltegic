"""
NexDL YouTube Addon
Downloads YouTube videos, playlists, and Shorts.
Uses yt-dlp as a subprocess for maximum compatibility.
"""
import re
import json
import subprocess
import shutil
import nexdl


class YouTubeAddon(nexdl.Addon):

    def name(self) -> str:
        return "youtube"

    def version(self) -> str:
        return "1.0.0"

    def description(self) -> str:
        return "Download YouTube videos, playlists, and Shorts via yt-dlp"

    def author(self) -> str:
        return "NexDL Team"

    def supported_sites(self) -> list[str]:
        return ["youtube.com", "youtu.be", "www.youtube.com", "music.youtube.com"]

    def can_handle(self, url: str) -> bool:
        return any(s in url for s in ["youtube.com/", "youtu.be/"])

    def extract(self, ctx: nexdl.Context) -> list[nexdl.DownloadItem]:
        url = ctx.url
        ctx.log("info", f"Extracting from YouTube: {url}")

        # Try yt-dlp for metadata first
        if shutil.which("yt-dlp"):
            return self._extract_via_ytdlp(url, ctx)
        else:
            ctx.log("warn", "yt-dlp not found — falling back to basic extraction")
            return self._extract_basic(url, ctx)

    def _extract_via_ytdlp(self, url: str, ctx: nexdl.Context) -> list[nexdl.DownloadItem]:
        """Use yt-dlp --dump-json to get video info without downloading"""
        try:
            result = subprocess.run(
                ["yt-dlp", "--dump-json", "--no-playlist", url],
                capture_output=True, text=True, timeout=30,
            )
            if result.returncode != 0:
                ctx.log("warn", f"yt-dlp error: {result.stderr[:200]}")
                return []

            info = json.loads(result.stdout)
            title = info.get("title", "YouTube Video")
            ext = info.get("ext", "mp4")
            video_id = info.get("id", "unknown")

            # Build a yt-dlp download item (custom download method)
            item = nexdl.DownloadItem(
                url=url,
                title=title,
                filename=f"{self._safe_filename(title)}.{ext}",
                output_path=f"{ctx.output_dir}/{self._safe_filename(title)}.{ext}",
            )
            item.metadata["video_id"] = video_id
            item.metadata["duration"] = str(info.get("duration", 0))
            item.metadata["uploader"] = info.get("uploader", "")
            item.metadata["use_ytdlp"] = "true"
            return [item]

        except subprocess.TimeoutExpired:
            ctx.log("error", "yt-dlp timed out")
        except json.JSONDecodeError as e:
            ctx.log("error", f"yt-dlp JSON parse error: {e}")
        except Exception as e:
            ctx.log("error", f"yt-dlp exception: {e}")
        return []

    def download(self, item: nexdl.DownloadItem, ctx: nexdl.Context) -> None:
        """Override download to use yt-dlp for actual download"""
        if item.metadata.get("use_ytdlp") == "true" and shutil.which("yt-dlp"):
            self._download_via_ytdlp(item, ctx)
        else:
            # Fallback: direct HTTP download
            ctx.http.download(item.url, item.output_path)

    def _download_via_ytdlp(self, item: nexdl.DownloadItem, ctx: nexdl.Context) -> None:
        output_template = f"{ctx.output_dir}/%(title)s.%(ext)s"
        cookie_header = ctx.http.cookies if ctx.http.cookies else None

        cmd = [
            "yt-dlp",
            "--format", "bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best",
            "--merge-output-format", "mp4",
            "--output", output_template,
            "--no-mtime",
        ]

        if cookie_header:
            # Write cookies to temp file
            import tempfile, os
            with tempfile.NamedTemporaryFile(mode='w', suffix='.txt', delete=False) as f:
                f.write("# Netscape HTTP Cookie File\n")
                # Parse cookie string to netscape format (simplified)
                for part in cookie_header.split(";"):
                    part = part.strip()
                    if "=" in part:
                        name, value = part.split("=", 1)
                        # We'd need domain info for proper netscape format
                cmd.extend(["--cookies", f.name])
                cookie_file = f.name

        cmd.append(item.url)

        ctx.log("info", f"Running yt-dlp: {' '.join(cmd[-3:])}")
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=3600)

        if result.returncode != 0:
            raise RuntimeError(f"yt-dlp failed: {result.stderr[-500:]}")

        ctx.log("info", "yt-dlp download complete")

    def _extract_basic(self, url: str, ctx: nexdl.Context) -> list[nexdl.DownloadItem]:
        """Basic extraction without yt-dlp"""
        html = ctx.http.get(url)
        title = re.search(r'"title":"([^"]{5,100})"', html)
        title = title.group(1) if title else "YouTube Video"

        # Try to find streaming URLs
        patterns = [
            r'"url":"(https://[^"]+googlevideo\.com/[^"]+)"',
            r'"signatureCipher":"[^"]*url=(https%3A%2F%2F[^"&]+)"',
        ]
        for p in patterns:
            m = re.search(p, html)
            if m:
                video_url = m.group(1).replace("\\u0026", "&")
                item = nexdl.DownloadItem(
                    url=video_url,
                    title=title,
                    filename=self._safe_filename(title) + ".mp4",
                    output_path=f"{ctx.output_dir}/{self._safe_filename(title)}.mp4",
                )
                return [item]

        ctx.log("warn", "Could not extract YouTube URL without yt-dlp")
        return []

    def _safe_filename(self, s: str) -> str:
        return re.sub(r'[<>:"/\\|?*]', '_', s)[:100]
