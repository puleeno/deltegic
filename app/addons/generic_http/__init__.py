"""
NexDL Generic HTTP Addon
Handles direct file URLs — any URL pointing to a downloadable file.
This is the fallback addon that handles URLs no other addon claims.
"""
import re
import nexdl

DOWNLOADABLE_EXTENSIONS = {
    '.mp4', '.mkv', '.avi', '.mov', '.webm', '.flv', '.wmv',
    '.mp3', '.aac', '.flac', '.wav', '.ogg', '.m4a',
    '.jpg', '.jpeg', '.png', '.gif', '.webp', '.bmp', '.svg',
    '.zip', '.rar', '.7z', '.tar', '.gz',
    '.pdf', '.epub', '.mobi',
    '.apk', '.exe', '.dmg', '.deb', '.rpm',
}


class GenericHttpAddon(nexdl.Addon):

    def name(self) -> str:
        return "generic-http"

    def version(self) -> str:
        return "1.0.0"

    def description(self) -> str:
        return "Downloads direct file URLs (MP4, MP3, ZIP, PDF, images, etc.)"

    def author(self) -> str:
        return "NexDL Team"

    def supported_sites(self) -> list[str]:
        return ["*"]  # handles all sites as fallback

    def can_handle(self, url: str) -> bool:
        """Only claim URLs that have a downloadable file extension"""
        path = url.split("?")[0].lower()
        return any(path.endswith(ext) for ext in DOWNLOADABLE_EXTENSIONS)

    def extract(self, ctx: nexdl.Context) -> list[nexdl.DownloadItem]:
        url = ctx.url
        filename = url.split("/")[-1].split("?")[0] or "download"

        item = nexdl.DownloadItem(
            url=url,
            title=filename,
            filename=filename,
            output_path=f"{ctx.output_dir}/{filename}",
        )
        return [item]
