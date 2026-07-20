"""
NexDL Instagram Addon
Downloads Instagram posts, Reels, Stories, and profile media.
Requires logged-in Instagram session cookies via Account Manager.
"""
import re
import json
import nexdl


class InstagramAddon(nexdl.Addon):

    def name(self) -> str:
        return "instagram"

    def version(self) -> str:
        return "1.0.0"

    def description(self) -> str:
        return "Download Instagram posts, Reels, Stories, and carousels"

    def author(self) -> str:
        return "NexDL Team"

    def supported_sites(self) -> list[str]:
        return ["instagram.com", "www.instagram.com"]

    def can_handle(self, url: str) -> bool:
        return "instagram.com" in url

    def extract(self, ctx: nexdl.Context) -> list[nexdl.DownloadItem]:
        url = ctx.url
        ctx.log("info", f"Extracting from Instagram: {url}")

        # Set Instagram-specific headers
        ctx.http.set_header("X-IG-App-ID", "936619743392459")
        ctx.http.set_header("X-Requested-With", "XMLHttpRequest")

        if "/reel/" in url:
            return self._extract_reel(url, ctx)
        elif "/p/" in url:
            return self._extract_post(url, ctx)
        elif "/stories/" in url:
            return self._extract_stories(url, ctx)
        else:
            return self._extract_post(url, ctx)

    def _get_shortcode(self, url: str) -> str | None:
        m = re.search(r'/(?:p|reel|tv)/([A-Za-z0-9_-]+)', url)
        return m.group(1) if m else None

    def _extract_post(self, url: str, ctx: nexdl.Context) -> list[nexdl.DownloadItem]:
        shortcode = self._get_shortcode(url)
        if not shortcode:
            ctx.log("error", f"Could not extract shortcode from {url}")
            return []

        # Try GraphQL API
        api_url = f"https://www.instagram.com/graphql/query/?query_hash=b3055c01b4b222b8a47dc12b090e4e64&variables=%7B%22shortcode%22%3A%22{shortcode}%22%7D"

        try:
            data = ctx.http.get_json(api_url)
            media = data.get("data", {}).get("shortcode_media", {})
            return self._media_to_items(media, ctx)
        except Exception as e:
            ctx.log("warn", f"GraphQL failed: {e}, trying oEmbed")
            return self._extract_via_oembed(url, ctx)

    def _extract_reel(self, url: str, ctx: nexdl.Context) -> list[nexdl.DownloadItem]:
        return self._extract_post(url, ctx)

    def _extract_stories(self, url: str, ctx: nexdl.Context) -> list[nexdl.DownloadItem]:
        ctx.log("info", "Stories extraction requires authentication")
        # Stories need session — attempt with cookies
        html = ctx.http.get(url)
        items = []

        # Find video sources
        video_urls = re.findall(r'"video_url":"([^"]+)"', html)
        image_urls = re.findall(r'"display_url":"([^"]+)"', html)

        for i, vu in enumerate(video_urls):
            vu = vu.replace("\\/", "/")
            item = nexdl.DownloadItem(
                url=vu,
                title=f"Story {i+1}",
                filename=f"story_{i+1:03d}.mp4",
                output_path=f"{ctx.output_dir}/story_{i+1:03d}.mp4",
            )
            item.index = i
            items.append(item)

        for i, iu in enumerate(image_urls[:len(image_urls)]):
            iu = iu.replace("\\/", "/")
            item = nexdl.DownloadItem(
                url=iu,
                title=f"Story Photo {i+1}",
                filename=f"story_photo_{i+1:03d}.jpg",
                output_path=f"{ctx.output_dir}/story_photo_{i+1:03d}.jpg",
            )
            items.append(item)

        return items

    def _media_to_items(self, media: dict, ctx: nexdl.Context) -> list[nexdl.DownloadItem]:
        items = []
        typename = media.get("__typename", "")

        if typename == "GraphSidecar":
            # Carousel / album
            edges = media.get("edge_sidecar_to_children", {}).get("edges", [])
            for i, edge in enumerate(edges):
                node = edge.get("node", {})
                sub_items = self._media_to_items(node, ctx)
                for item in sub_items:
                    item.index = i
                    item.total = len(edges)
                    items.extend([item])
        elif typename == "GraphVideo":
            video_url = media.get("video_url", "")
            caption = self._get_caption(media)
            if video_url:
                item = nexdl.DownloadItem(
                    url=video_url,
                    title=caption or "Instagram Video",
                    filename=self._safe_filename(caption or "instagram_video") + ".mp4",
                    output_path=f"{ctx.output_dir}/{self._safe_filename(caption or 'instagram_video')}.mp4",
                )
                item.metadata["shortcode"] = media.get("shortcode", "")
                items.append(item)
        else:
            # GraphImage
            display_url = media.get("display_url", "")
            caption = self._get_caption(media)
            if display_url:
                display_url = display_url.replace("\\/", "/")
                item = nexdl.DownloadItem(
                    url=display_url,
                    title=caption or "Instagram Photo",
                    filename=self._safe_filename(caption or "instagram_photo") + ".jpg",
                    output_path=f"{ctx.output_dir}/{self._safe_filename(caption or 'instagram_photo')}.jpg",
                )
                items.append(item)

        return items

    def _extract_via_oembed(self, url: str, ctx: nexdl.Context) -> list[nexdl.DownloadItem]:
        """Fallback: use oEmbed for basic info"""
        oembed_url = f"https://www.instagram.com/oembed/?url={url}"
        try:
            data = ctx.http.get_json(oembed_url)
            thumbnail = data.get("thumbnail_url", "")
            title = data.get("title", "Instagram Post")
            if thumbnail:
                item = nexdl.DownloadItem(
                    url=thumbnail,
                    title=title,
                    filename=self._safe_filename(title) + ".jpg",
                    output_path=f"{ctx.output_dir}/{self._safe_filename(title)}.jpg",
                )
                return [item]
        except Exception as e:
            ctx.log("error", f"oEmbed failed: {e}")
        return []

    def _get_caption(self, media: dict) -> str:
        try:
            edges = media.get("edge_media_to_caption", {}).get("edges", [])
            if edges:
                return edges[0]["node"]["text"][:80]
        except:
            pass
        return ""

    def _safe_filename(self, s: str) -> str:
        return re.sub(r'[<>:"/\\|?*\n\r]', '_', s.strip())[:80]
