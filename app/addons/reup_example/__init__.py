"""
NexDL Reup Example Addon
Example post-processing addon: after download, re-upload to another platform.

This shows how to write an addon that acts as a post-processor.
The addon doesn't download anything itself — it hooks into post_process()
to reupload what was downloaded.
"""
import re
import os
import json
import nexdl


class ReupExampleAddon(nexdl.Addon):
    """
    Example reup addon — copies finished downloads to a destination.
    In practice, replace _reup() with actual upload logic (S3, FTP, etc.).
    """

    def name(self) -> str:
        return "reup-example"

    def version(self) -> str:
        return "1.0.0"

    def description(self) -> str:
        return "Example: re-upload downloaded files to another destination"

    def author(self) -> str:
        return "NexDL Team"

    def supported_sites(self) -> list[str]:
        return []  # Post-processor only

    def can_handle(self, url: str) -> bool:
        return False  # Not a URL handler

    def post_process(self, item: nexdl.DownloadItem, ctx: nexdl.Context) -> None:
        """Called after each item is downloaded."""
        enabled = ctx.get_option("reup_enabled", "false")
        if enabled.lower() != "true":
            return

        destination = ctx.get_option("reup_destination", "")
        if not destination:
            ctx.log("warn", "reup_destination not set, skipping reup")
            return

        ctx.log("info", f"Reupping {item.output_path} -> {destination}")
        self._reup(item.output_path, destination, ctx)

    def _reup(self, source_path: str, destination: str, ctx: nexdl.Context) -> None:
        """
        Replace this with actual upload logic:
        - S3: use boto3
        - FTP: use ftplib
        - Telegram: send via Bot API
        - Google Drive: use google-auth + drive API
        - Rclone: subprocess call
        """
        if destination.startswith("s3://"):
            self._reup_s3(source_path, destination, ctx)
        elif destination.startswith("ftp://"):
            self._reup_ftp(source_path, destination, ctx)
        elif destination.startswith("rclone:"):
            self._reup_rclone(source_path, destination, ctx)
        else:
            # Default: copy to local path
            import shutil
            os.makedirs(destination, exist_ok=True)
            dest_file = os.path.join(destination, os.path.basename(source_path))
            shutil.copy2(source_path, dest_file)
            ctx.log("info", f"Copied to {dest_file}")

    def _reup_s3(self, path: str, s3_url: str, ctx: nexdl.Context) -> None:
        import subprocess
        result = subprocess.run(
            ["aws", "s3", "cp", path, s3_url],
            capture_output=True, text=True, timeout=300
        )
        if result.returncode != 0:
            raise RuntimeError(f"S3 upload failed: {result.stderr}")
        ctx.log("info", f"Uploaded to S3: {s3_url}")

    def _reup_rclone(self, path: str, destination: str, ctx: nexdl.Context) -> None:
        import subprocess
        # Format: rclone:remote:path
        remote = destination[len("rclone:"):]
        result = subprocess.run(
            ["rclone", "copy", path, remote],
            capture_output=True, text=True, timeout=300
        )
        if result.returncode != 0:
            raise RuntimeError(f"rclone failed: {result.stderr}")
        ctx.log("info", f"Uploaded via rclone to {remote}")

    def _reup_ftp(self, path: str, ftp_url: str, ctx: nexdl.Context) -> None:
        from ftplib import FTP
        from urllib.parse import urlparse
        parsed = urlparse(ftp_url)
        with FTP(parsed.hostname) as ftp:
            if parsed.username:
                ftp.login(parsed.username, parsed.password or "")
            with open(path, "rb") as f:
                filename = os.path.basename(path)
                ftp.storbinary(f"STOR {filename}", f)
        ctx.log("info", f"Uploaded via FTP: {ftp_url}")
