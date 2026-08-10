import { Router } from "express";

const router = Router();

interface Addon {
  name: string;
  version: string;
  description: string;
  author: string;
  supportedSites: string[];
  enabled: boolean;
  path: string;
}

const addons = new Map<string, Addon>([
  ["youtube", {
    name: "youtube", version: "1.2.0",
    description: "Downloads videos, playlists, and shorts from YouTube using yt-dlp. Supports quality selection, subtitles, and metadata.",
    author: "NexDL Team", supportedSites: ["youtube.com", "youtu.be"],
    enabled: true, path: "~/.local/share/nexdl/addons/youtube",
  }],
  ["instagram", {
    name: "instagram", version: "1.1.0",
    description: "Downloads posts, reels, stories, and carousels from Instagram. Supports logged-in account cookies for private content.",
    author: "NexDL Team", supportedSites: ["instagram.com"],
    enabled: true, path: "~/.local/share/nexdl/addons/instagram",
  }],
  ["facebook", {
    name: "facebook", version: "1.0.3",
    description: "Downloads reels, videos, photos, and albums from Facebook pages and profiles.",
    author: "NexDL Team", supportedSites: ["facebook.com", "fb.watch"],
    enabled: true, path: "~/.local/share/nexdl/addons/facebook",
  }],
  ["generic_http", {
    name: "generic_http", version: "0.9.0",
    description: "Fallback downloader for direct file links (mp4, mp3, pdf, zip, etc.). No extraction, direct download only.",
    author: "NexDL Team", supportedSites: ["*"],
    enabled: true, path: "~/.local/share/nexdl/addons/generic_http",
  }],
  ["telegram_bot", {
    name: "telegram_bot", version: "0.5.0",
    description: "Telegram bot addon — accepts URLs sent to the bot and enqueues them automatically. Requires BOT_TOKEN in settings.",
    author: "NexDL Team", supportedSites: ["t.me"],
    enabled: false, path: "~/.local/share/nexdl/addons/telegram_bot",
  }],
  ["reup_example", {
    name: "reup_example", version: "0.3.0",
    description: "Example post-processor addon. Re-uploads completed downloads to S3, FTP, or rclone remotes after download finishes.",
    author: "NexDL Team", supportedSites: [],
    enabled: false, path: "~/.local/share/nexdl/addons/reup_example",
  }],
]);

router.get("/addons", (_req, res) => {
  res.json([...addons.values()]);
});

router.post("/addons/:name/enable", (req, res) => {
  const addon = addons.get(req.params.name);
  if (!addon) return void res.status(404).json({ error: "addon not found" });
  addon.enabled = true;
  res.json(addon);
});

router.post("/addons/:name/disable", (req, res) => {
  const addon = addons.get(req.params.name);
  if (!addon) return void res.status(404).json({ error: "addon not found" });
  addon.enabled = false;
  res.json(addon);
});

router.post("/addons/:name/reload", (req, res) => {
  const addon = addons.get(req.params.name);
  if (!addon) return void res.status(404).json({ error: "addon not found" });
  // Simulate reload by bumping a patch version digit
  const parts = addon.version.split(".");
  parts[2] = String(Number(parts[2] ?? 0) + 1);
  addon.version = parts.join(".");
  res.json(addon);
});

export default router;
