"""
Deltegic Facebook Addon
Downloads Facebook albums, Reels, videos, and photo sets.
Requires a logged-in Facebook account with cookies set via Account Manager.
"""
import re
import json
import deltegic


class FacebookAddon(deltegic.Addon):

    def name(self) -> str:
        return "facebook"

    def version(self) -> str:
        return "2.0.0"

    def description(self) -> str:
        return "Download Facebook videos, Reels, albums, and photo sets"

    def author(self) -> str:
        return "Deltegic Team"

    def supported_sites(self) -> list[str]:
        return ["facebook.com", "fb.watch", "fb.com", "m.facebook.com"]

    def can_handle(self, url: str) -> bool:
        patterns = [
            r"(?:www\.|m\.)?facebook\.com/",
            r"fb\.watch/",
            r"fb\.com/",
        ]
        return any(re.search(p, url) for p in patterns)

    def extract(self, ctx: deltegic.Context) -> list[deltegic.DownloadItem]:
        url = ctx.url
        ctx.log("info", f"Extracting from Facebook: {url}")

        # Clean tracking params
        clean_url = re.sub(r'\?.*$', '', url)

        # Check if we have cookies
        has_cookies = bool(ctx.http.cookies)
        if not has_cookies:
            ctx.log("warn", "No Facebook cookies found — content may be limited. Add a Facebook account in Accounts tab.")

        # Determine content type from URL
        if "/reel/" in url or "/reels/" in url:
            return self._extract_reel(clean_url, ctx)
        elif "/videos/" in url or "/video/" in url or "/watch/" in url:
            return self._extract_video(clean_url, ctx)
        elif "/media/set/" in url or "/album/" in url or "set=" in url:
            return self._extract_album(clean_url, ctx)
        elif "/photos/" in url or "/photo/" in url:
            return self._extract_photo(clean_url, ctx)
        elif "/pages/" in url:
            return self._extract_page(clean_url, ctx)
        elif "/story.php" in url or "/posts/" in url or "/permalink/" in url:
            return self._extract_post(clean_url, ctx)
        else:
            # Try multiple strategies
            return self._extract_generic(clean_url, ctx)

    def _extract_reel(self, url: str, ctx: deltegic.Context) -> list[deltegic.DownloadItem]:
        """Extract a Facebook Reel"""
        ctx.log("info", f"Extracting Reel: {url}")

        # Try main page first
        html = self._fetch_page(url, ctx)
        if html:
            video_url = self._find_video_in_html(html)
            title = self._extract_title(html) or "Facebook Reel"

            if video_url:
                return [self._make_video_item(video_url, title, url, "reel")]

        # Try mobile URL
        mobile_url = url.replace("www.facebook.com", "m.facebook.com")
        ctx.log("info", f"Trying mobile URL: {mobile_url}")
        html = self._fetch_page(mobile_url, ctx)
        if html:
            video_url = self._find_video_in_html(html)
            title = self._extract_title(html) or "Facebook Reel"

            if video_url:
                return [self._make_video_item(video_url, title, url, "reel")]

        ctx.log("error", "Failed to extract Reel — no video URL found")
        return []

    def _extract_video(self, url: str, ctx: deltegic.Context) -> list[deltegic.DownloadItem]:
        """Extract a Facebook video"""
        ctx.log("info", f"Extracting video: {url}")

        html = self._fetch_page(url, ctx)
        if not html:
            return []

        video_url = self._find_video_in_html(html)
        title = self._extract_title(html) or "Facebook Video"

        if video_url:
            return [self._make_video_item(video_url, title, url, "video")]

        # Try embed URL
        embed_url = self._get_embed_url(url)
        if embed_url:
            ctx.log("info", f"Trying embed URL: {embed_url}")
            html = self._fetch_page(embed_url, ctx)
            if html:
                video_url = self._find_video_in_html(html)
                if video_url:
                    return [self._make_video_item(video_url, title, url, "video")]

        ctx.log("error", "Failed to extract video — no video URL found")
        return []

    def _extract_album(self, url: str, ctx: deltegic.Context) -> list[deltegic.DownloadItem]:
        """Extract a Facebook photo album"""
        ctx.log("info", f"Extracting album: {url}")

        html = self._fetch_page(url, ctx)
        if not html:
            return []

        items = []

        # Strategy 1: Find photo links
        photo_urls = re.findall(r'href="(/photo/[^"]+)"', html)
        photo_urls = list(set(photo_urls))

        # Strategy 2: Find photo IDs from data
        if not photo_urls:
            photo_ids = re.findall(r'"photo_id":"(\d+)"', html)
            photo_urls = [f"/photo/?fbid={pid}" for pid in photo_ids]

        # Strategy 3: Find image URIs directly
        if not photo_urls:
            img_urls = re.findall(r'"uri":"(https://[^"]+(?:\.jpg|\.jpeg|\.png|\.webp)[^"]*)"', html)
            for i, img_url in enumerate(img_urls[:200]):
                img_url = img_url.replace("\\/", "/")
                ext = self._get_extension(img_url)
                filename = f"photo_{i+1:04d}{ext}"
                items.append(self._make_image_item(img_url, f"Photo {i+1}", filename, url))

            if items:
                ctx.log("info", f"Found {len(items)} images directly in HTML")
                return items

        ctx.log("info", f"Found {len(photo_urls)} photo links in album")

        for i, photo_path in enumerate(photo_urls[:200]):
            full_url = f"https://www.facebook.com{photo_path}" if photo_path.startswith("/") else photo_path
            photo_html = self._fetch_page(full_url, ctx)
            if photo_html:
                img_url = self._find_image_in_html(photo_html)
                if img_url:
                    ext = self._get_extension(img_url)
                    filename = f"photo_{i+1:04d}{ext}"
                    items.append(self._make_image_item(img_url, f"Photo {i+1}", filename, full_url))
                    ctx.report_progress(i + 1, len(photo_urls), f"Extracted photo {i+1}/{len(photo_urls)}")

        ctx.log("info", f"Extracted {len(items)} photos from album")
        return items

    def _extract_photo(self, url: str, ctx: deltegic.Context) -> list[deltegic.DownloadItem]:
        """Extract a single Facebook photo"""
        html = self._fetch_page(url, ctx)
        if not html:
            return []

        img_url = self._find_image_in_html(html)
        if img_url:
            return [self._make_image_item(img_url, "Facebook Photo", "photo.jpg", url)]

        ctx.log("error", "Failed to extract photo")
        return []

    def _extract_page(self, url: str, ctx: deltegic.Context) -> list[deltegic.DownloadItem]:
        """Extract content from a Facebook Page"""
        ctx.log("info", f"Extracting Facebook Page: {url}")

        html = self._fetch_page(url, ctx)
        if not html:
            return []

        items = []

        # Pages often have videos
        video_urls = re.findall(r'"(https?://[^"]+(?:\.mp4|/video)[^"]*)"', html)
        seen = set()
        for video_url in video_urls:
            video_url = video_url.replace("\\u0025", "%").replace("\\/", "/")
            if video_url not in seen and "facebook.com" not in video_url:
                seen.add(video_url)
                title = f"Page Video {len(items) + 1}"
                items.append(self._make_video_item(video_url, title, url, "page_video"))

        # Pages also have images
        img_urls = re.findall(r'"uri":"(https://[^"]+(?:\.jpg|\.jpeg|\.png|\.webp)[^"]*)"', html)
        for img_url in img_urls[:50]:
            img_url = img_url.replace("\\/", "/")
            ext = self._get_extension(img_url)
            filename = f"page_photo_{len(items)+1:04d}{ext}"
            items.append(self._make_image_item(img_url, f"Page Photo {len(items)+1}", filename, url))

        ctx.log("info", f"Found {len(items)} items from Page")
        return items

    def _extract_post(self, url: str, ctx: deltegic.Context) -> list[deltegic.DownloadItem]:
        """Extract a Facebook post — supports multiple images and videos"""
        ctx.log("info", f"Extracting post: {url}")

        html = self._fetch_page(url, ctx)
        if not html:
            return []

        # Try video first
        video_url = self._find_video_in_html(html)
        if video_url:
            title = self._extract_title(html) or "Facebook Post Video"
            return [self._make_video_item(video_url, title, url, "post_video")]

        # Find ALL images in the post
        items = []

        # Strategy 1: All high-res image URIs from JSON data (scontent/fbcdn)
        img_urls = re.findall(r'"uri"\s*:\s*"(https?://[^"]*(?:scontent[^"]*|fbcdn[^"]*)\.(?:jpg|jpeg|png|webp)[^"]*)"', html)
        img_urls = list(dict.fromkeys(img_urls))  # dedupe preserving order

        # Strategy 2: Escaped URLs in JSON (common in __bbox data)
        if not img_urls:
            img_urls = re.findall(r'\\?"(?:uri|url)\\?"\s*:\s*\\?"(https?:\\/\\/[^"]*(?:scontent|fbcdn)[^"]*\.(?:jpg|jpeg|png|webp)[^"]*)', html)
            img_urls = [u.replace("\\/", "/") for u in img_urls]
            img_urls = list(dict.fromkeys(img_urls))

        # Strategy 3: All image src tags
        if not img_urls:
            img_urls = re.findall(r'<img[^>]+src="(https://[^"]+(?:scontent|fbcdn)[^"]+(?:\.jpg|\.jpeg|\.png|\.webp)[^"]*)"', html)
            img_urls = list(dict.fromkeys(img_urls))

        # Strategy 4: data-src
        if not img_urls:
            img_urls = re.findall(r'data-src="(https://[^"]+(?:scontent|fbcdn)[^"]+(?:\.jpg|\.jpeg|\.png|\.webp)[^"]*)"', html)
            img_urls = list(dict.fromkeys(img_urls))

        # Strategy 5: og:image (fallback, single)
        if not img_urls:
            og = re.search(r'<meta[^>]+property="og:image"[^>]+content="([^"]+)"', html)
            if og:
                img_urls = [og.group(1)]

        # Strategy 6: Any scontent URL in the page
        if not img_urls:
            img_urls = re.findall(r'(https?://[^"\'\\>\s]+scontent[^"\'\\>\s]+\.(?:jpg|jpeg|png|webp))', html)
            img_urls = list(dict.fromkeys(img_urls))

        ctx.log("info", f"Found {len(img_urls)} image URLs in post HTML")

        for i, img_url in enumerate(img_urls[:100]):
            img_url = img_url.replace("\\/", "/").replace("\\u0025", "%").replace("\\u0026", "&")
            if not img_url.startswith("http"):
                continue
            ext = self._get_extension(img_url)
            filename = f"photo_{i+1:04d}{ext}"
            items.append(self._make_image_item(img_url, f"Photo {i+1}", filename, url))

        if items:
            ctx.log("info", f"Extracted {len(items)} images from post")
        else:
            ctx.log("error", "No images found in post — you may need to log in via Accounts tab")

        return items

    def _extract_generic(self, url: str, ctx: deltegic.Context) -> list[deltegic.DownloadItem]:
        """Generic extraction — try everything"""
        ctx.log("info", f"Trying generic extraction: {url}")

        html = self._fetch_page(url, ctx)
        if not html:
            return []

        # Try video
        video_url = self._find_video_in_html(html)
        if video_url:
            title = self._extract_title(html) or "Facebook Video"
            return [self._make_video_item(video_url, title, url, "video")]

        # Try image
        img_url = self._find_image_in_html(html)
        if img_url:
            return [self._make_image_item(img_url, "Facebook Image", "image.jpg", url)]

        ctx.log("error", "Could not extract any downloadable content from this URL")
        ctx.log("info", "Tip: Make sure you are logged into Facebook (add account in Accounts tab)")
        return []

    # ── Helpers ───────────────────────────────────────────────────────────────

    def _fetch_page(self, url: str, ctx: deltegic.Context) -> str | None:
        """Fetch a page with error handling"""
        try:
            html = ctx.http.get(url)
            if not html:
                ctx.log("warn", f"Empty response from {url}")
                return None

            # Check for login wall (actual login form, not just nav links)
            lower = html.lower()
            if ('id="login_form"' in lower or 'id="loginbutton"' in lower
                    or 'name="login"' in lower or 'data-testid="royal_login_button"' in lower):
                ctx.log("warn", "Facebook returned login page — add cookies via Accounts tab")
                return None

            # Check for error page
            if "content not found" in html.lower() or "page isn't available" in html.lower():
                ctx.log("warn", "Facebook says content is not available")
                return None

            return html
        except Exception as e:
            ctx.log("error", f"Failed to fetch {url}: {e}")
            return None

    def _find_video_in_html(self, html: str) -> str | None:
        """Find highest-quality video URL in page HTML"""
        patterns = [
            # HD video
            r'"hd_src":"([^"]+)"',
            r'"hd_src_no_ratelimit":"([^"]+)"',
            r'"playable_url_quality_hd":"([^"]+)"',
            # SD video
            r'"sd_src":"([^"]+)"',
            r'"sd_src_no_ratelimit":"([^"]+)"',
            r'"playable_url":"([^"]+)"',
            # DASH video
            r'"dash_manifest":.*?"base_url":"([^"]+)"',
            # Embedded player
            r'"video_url":"([^"]+)"',
            # Generic mp4 in JSON
            r'"(https?://[^"]+\.mp4[^"]*)"',
            # Video tag src
            r'<source[^>]+src="([^"]+\.mp4[^"]*)"',
            # data-src
            r'data-src="(https?://[^"]+\.mp4[^"]*)"',
        ]
        for pattern in patterns:
            match = re.search(pattern, html)
            if match:
                url = match.group(1)
                url = url.replace("\\u0025", "%").replace("\\/", "/").replace("\\u0026", "&")
                # Validate it looks like a real URL
                if url.startswith("http") and ("facebook" in url or "fbcdn" in url or "akamai" in url):
                    return url
        return None

    def _find_image_in_html(self, html: str) -> str | None:
        """Find highest-quality image URL in page HTML"""
        patterns = [
            # High-res image URIs
            r'"uri":"(https://[^"]+(?:scontent[^"]+|fbcdn[^"]+)(?:\.jpg|\.jpeg|\.png|\.webp)[^"]*)"',
            # og:image
            r'<meta[^>]+property="og:image"[^>]+content="([^"]+)"',
            # Regular image src
            r'<img[^>]+src="(https://[^"]+(?:scontent|fbcdn)[^"]+\.jpe?g[^"]*)"',
            # image data in JSON
            r'"image":\{"uri":"([^"]+)"',
            # Full-size image link
            r'href="([^"]+/full/\d+/[^"]+)"',
        ]
        for pattern in patterns:
            match = re.search(pattern, html)
            if match:
                url = match.group(1).replace("\\/", "/")
                if url.startswith("http"):
                    return url
        return None

    def _extract_title(self, html: str) -> str | None:
        patterns = [
            r'<title>([^<]+)</title>',
            r'"title":"([^"]{5,100})"',
            r'property="og:title"[^>]+content="([^"]+)"',
        ]
        for p in patterns:
            m = re.search(p, html)
            if m:
                title = m.group(1).strip()
                # Clean Facebook title suffixes
                title = re.sub(r'\s*\|\s*Facebook.*$', '', title)
                title = re.sub(r'\s*-\s*Facebook.*$', '', title)
                return title if title else None
        return None

    def _get_embed_url(self, url: str) -> str | None:
        """Get embed URL for a video"""
        match = re.search(r'/videos/(\d+)', url)
        if match:
            return f"https://www.facebook.com/plugins/video.php?href=https://www.facebook.com/video/{match.group(1)}"
        match = re.search(r'/watch/?\?v=(\d+)', url)
        if match:
            return f"https://www.facebook.com/plugins/video.php?href=https://www.facebook.com/video/{match.group(1)}"
        return None

    def _make_video_item(self, url: str, title: str, referer: str, video_type: str) -> deltegic.DownloadItem:
        item = deltegic.DownloadItem(
            url=url,
            title=title,
            filename=self._safe_filename(title) + ".mp4",
            output_path=f"{self._safe_filename(title)}.mp4",
        )
        item.referer = referer
        item.metadata["type"] = video_type
        return item

    def _make_image_item(self, url: str, title: str, filename: str, referer: str) -> deltegic.DownloadItem:
        item = deltegic.DownloadItem(
            url=url,
            title=title,
            filename=filename,
            output_path=filename,
        )
        item.referer = referer
        item.metadata["type"] = "image"
        return item

    def _safe_filename(self, s: str) -> str:
        return re.sub(r'[<>:"/\\|?*]', '_', s)[:100]

    def _get_extension(self, url: str) -> str:
        for ext in ['.jpg', '.jpeg', '.png', '.webp']:
            if ext in url.lower():
                return ext
        return '.jpg'
