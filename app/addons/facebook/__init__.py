"""
NexDL Facebook Addon
Downloads Facebook albums, Reels, videos, and photo sets.
Requires a logged-in Facebook account with cookies set via Account Manager.
"""
import re
import json
import nexdl


class FacebookAddon(nexdl.Addon):

    def name(self) -> str:
        return "facebook"

    def version(self) -> str:
        return "1.0.0"

    def description(self) -> str:
        return "Download Facebook videos, Reels, albums, and photo sets"

    def author(self) -> str:
        return "NexDL Team"

    def supported_sites(self) -> list[str]:
        return ["facebook.com", "fb.watch", "fb.com", "m.facebook.com"]

    def can_handle(self, url: str) -> bool:
        patterns = [
            r"facebook\.com/",
            r"fb\.watch/",
            r"fb\.com/",
        ]
        return any(re.search(p, url) for p in patterns)

    def extract(self, ctx: nexdl.Context) -> list[nexdl.DownloadItem]:
        url = ctx.url
        ctx.log("info", f"Extracting from Facebook: {url}")

        # Determine content type
        if "/reel/" in url or "/reels/" in url:
            return self._extract_reel(url, ctx)
        elif "/videos/" in url or "/video/" in url:
            return self._extract_video(url, ctx)
        elif "/media/set/" in url or "/album/" in url or "set=" in url:
            return self._extract_album(url, ctx)
        elif "/photos/" in url or "/photo/" in url:
            return self._extract_photo(url, ctx)
        else:
            # Try generic video extraction
            return self._extract_video(url, ctx)

    def _extract_reel(self, url: str, ctx: nexdl.Context) -> list[nexdl.DownloadItem]:
        """Extract a Facebook Reel"""
        ctx.log("info", f"Extracting Reel: {url}")
        html = ctx.http.get(url)

        # Look for video source in page data
        video_url = self._find_video_in_html(html)
        title = self._extract_title(html) or "Facebook Reel"

        if not video_url:
            ctx.log("warn", "Could not find direct video URL in Reel page")
            # Try mobile URL
            mobile_url = url.replace("www.facebook.com", "m.facebook.com")
            html = ctx.http.get(mobile_url)
            video_url = self._find_video_in_html(html)

        if video_url:
            item = nexdl.DownloadItem(
                url=video_url,
                title=title,
                filename=self._safe_filename(title) + ".mp4",
                output_path=f"{ctx.output_dir}/{self._safe_filename(title)}.mp4",
            )
            item.referer = url
            item.metadata["type"] = "reel"
            return [item]

        ctx.log("error", "Failed to extract Reel video URL")
        return []

    def _extract_video(self, url: str, ctx: nexdl.Context) -> list[nexdl.DownloadItem]:
        """Extract a Facebook video"""
        ctx.log("info", f"Extracting video: {url}")
        html = ctx.http.get(url)
        video_url = self._find_video_in_html(html)
        title = self._extract_title(html) or "Facebook Video"

        if video_url:
            item = nexdl.DownloadItem(
                url=video_url,
                title=title,
                filename=self._safe_filename(title) + ".mp4",
                output_path=f"{ctx.output_dir}/{self._safe_filename(title)}.mp4",
            )
            item.referer = url
            return [item]
        return []

    def _extract_album(self, url: str, ctx: nexdl.Context) -> list[nexdl.DownloadItem]:
        """Extract a Facebook photo album"""
        ctx.log("info", f"Extracting album: {url}")
        html = ctx.http.get(url)

        items = []
        # Find all photo URLs in the album
        photo_urls = re.findall(r'href="(/photo/[^"]+)"', html)
        photo_urls = list(set(photo_urls))  # deduplicate

        ctx.log("info", f"Found {len(photo_urls)} photos in album")

        for i, photo_path in enumerate(photo_urls[:200]):  # limit to 200
            full_url = f"https://www.facebook.com{photo_path}"
            photo_html = ctx.http.get(full_url)
            img_url = self._find_image_in_html(photo_html)

            if img_url:
                ext = self._get_extension(img_url)
                filename = f"photo_{i+1:04d}{ext}"
                item = nexdl.DownloadItem(
                    url=img_url,
                    title=f"Photo {i+1}",
                    filename=filename,
                    output_path=f"{ctx.output_dir}/{filename}",
                )
                item.index = i
                item.total = len(photo_urls)
                item.referer = full_url
                item.metadata["type"] = "album_photo"
                items.append(item)
                ctx.report_progress(i + 1, len(photo_urls), f"Extracted photo {i+1}/{len(photo_urls)}")

        return items

    def _extract_photo(self, url: str, ctx: nexdl.Context) -> list[nexdl.DownloadItem]:
        """Extract a single Facebook photo"""
        html = ctx.http.get(url)
        img_url = self._find_image_in_html(html)
        if img_url:
            ext = self._get_extension(img_url)
            item = nexdl.DownloadItem(
                url=img_url,
                title="Facebook Photo",
                filename=f"photo{ext}",
                output_path=f"{ctx.output_dir}/photo{ext}",
            )
            item.referer = url
            return [item]
        return []

    def _find_video_in_html(self, html: str) -> str | None:
        """Find highest-quality video URL in page HTML"""
        patterns = [
            # HD video
            r'"hd_src":"([^"]+)"',
            r'"playable_url_quality_hd":"([^"]+)"',
            # SD video
            r'"sd_src":"([^"]+)"',
            r'"playable_url":"([^"]+)"',
            # Generic mp4
            r'"(https?://[^"]+\.mp4[^"]*)"',
            # video tag src
            r'<source[^>]+src="([^"]+\.mp4[^"]*)"',
        ]
        for pattern in patterns:
            match = re.search(pattern, html)
            if match:
                url = match.group(1)
                # Unescape unicode escapes
                url = url.replace("\\u0025", "%").replace("\\/", "/")
                return url
        return None

    def _find_image_in_html(self, html: str) -> str | None:
        """Find highest-quality image URL in page HTML"""
        patterns = [
            r'"image":\{"uri":"([^"]+)"',
            r'"uri":"(https://[^"]+scontent[^"]+\.jpe?g[^"]*)"',
            r'<img[^>]+src="(https://[^"]+scontent[^"]+\.jpe?g[^"]*)"',
        ]
        for pattern in patterns:
            match = re.search(pattern, html)
            if match:
                url = match.group(1).replace("\\/", "/")
                return url
        return None

    def _extract_title(self, html: str) -> str | None:
        patterns = [
            r'<title>([^<]+)</title>',
            r'"title":"([^"]{5,100})"',
        ]
        for p in patterns:
            m = re.search(p, html)
            if m:
                return m.group(1).strip()
        return None

    def _safe_filename(self, s: str) -> str:
        return re.sub(r'[<>:"/\\|?*]', '_', s)[:100]

    def _get_extension(self, url: str) -> str:
        for ext in ['.jpg', '.jpeg', '.png', '.webp']:
            if ext in url.lower():
                return ext
        return '.jpg'
