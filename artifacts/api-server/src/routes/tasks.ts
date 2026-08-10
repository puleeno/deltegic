import { Router } from "express";
import { randomUUID } from "crypto";

const router = Router();

// ── In-memory queue (simulates Rust nexdl-core on Replit) ──────────────────
export interface Task {
  id: string;
  url: string;
  title: string;
  status: "queued" | "processing" | "downloading" | "post_processing" | "completed" | "paused" | "cancelled" | "failed";
  progress: number;
  downloaded: number;
  totalSize: number | null;
  speed: string;
  eta: string;
  addonName: string;
  outputDir: string;
  outputPath: string | null;
  error: string | null;
  priority: number;
  subtasksDone: number;
  subtasksTotal: number;
  createdAt: string;
  updatedAt: string;
}

const tasks = new Map<string, Task>();

// Seed a few demo tasks so the UI isn't empty on first load
function seedDemoTasks() {
  const demos: Task[] = [
    {
      id: randomUUID(), url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
      title: "Rick Astley - Never Gonna Give You Up (Official Video)",
      status: "completed", progress: 1.0, downloaded: 45234567, totalSize: 45234567,
      speed: "0 B/s", eta: "done", addonName: "youtube", outputDir: "~/Downloads/NexDL",
      outputPath: "~/Downloads/NexDL/rickroll.mp4", error: null, priority: 0,
      subtasksDone: 1, subtasksTotal: 1,
      createdAt: new Date(Date.now() - 3600000).toISOString(),
      updatedAt: new Date(Date.now() - 3500000).toISOString(),
    },
    {
      id: randomUUID(), url: "https://www.instagram.com/p/ABC123/",
      title: "Instagram Post — @travel.vibes",
      status: "downloading", progress: 0.63, downloaded: 12800000, totalSize: 20300000,
      speed: "2.4 MB/s", eta: "3s", addonName: "instagram", outputDir: "~/Downloads/NexDL",
      outputPath: null, error: null, priority: 1,
      subtasksDone: 0, subtasksTotal: 3,
      createdAt: new Date(Date.now() - 120000).toISOString(),
      updatedAt: new Date().toISOString(),
    },
    {
      id: randomUUID(), url: "https://www.facebook.com/reel/987654321",
      title: "Facebook Reel — Cooking Tutorial",
      status: "queued", progress: 0, downloaded: 0, totalSize: null,
      speed: "—", eta: "—", addonName: "facebook", outputDir: "~/Downloads/NexDL",
      outputPath: null, error: null, priority: 0,
      subtasksDone: 0, subtasksTotal: 0,
      createdAt: new Date(Date.now() - 30000).toISOString(),
      updatedAt: new Date(Date.now() - 30000).toISOString(),
    },
    {
      id: randomUUID(), url: "https://example.com/album/summer-photos",
      title: "Summer Photos Album (15 items)",
      status: "failed", progress: 0.2, downloaded: 3100000, totalSize: 15500000,
      speed: "0 B/s", eta: "—", addonName: "generic_http", outputDir: "~/Downloads/NexDL",
      outputPath: null, error: "HTTP 403 Forbidden — authentication required",
      priority: 0, subtasksDone: 3, subtasksTotal: 15,
      createdAt: new Date(Date.now() - 600000).toISOString(),
      updatedAt: new Date(Date.now() - 580000).toISOString(),
    },
  ];
  demos.forEach(t => tasks.set(t.id, t));
}
seedDemoTasks();

// Simulate download progress for "downloading" tasks
setInterval(() => {
  for (const task of tasks.values()) {
    if (task.status !== "downloading") continue;
    const inc = Math.random() * 0.04;
    task.progress = Math.min(1.0, task.progress + inc);
    task.downloaded = Math.round(task.progress * (task.totalSize ?? 20_000_000));
    task.speed = `${(Math.random() * 3 + 1).toFixed(1)} MB/s`;
    const remaining = task.totalSize ? task.totalSize - task.downloaded : 0;
    const eta = remaining / ((Math.random() * 3 + 1) * 1024 * 1024);
    task.eta = eta < 1 ? "<1s" : `${Math.ceil(eta)}s`;
    task.updatedAt = new Date().toISOString();
    if (task.progress >= 1.0) {
      task.status = "completed";
      task.outputPath = `${task.outputDir}/${task.title.slice(0, 40)}.mp4`;
      task.speed = "0 B/s"; task.eta = "done";
    }
  }
}, 1000);

function getStats() {
  const arr = [...tasks.values()];
  return {
    total: arr.length,
    queued: arr.filter(t => t.status === "queued").length,
    processing: arr.filter(t => t.status === "processing").length,
    downloading: arr.filter(t => t.status === "downloading").length,
    completed: arr.filter(t => t.status === "completed").length,
    failed: arr.filter(t => t.status === "failed").length,
    paused: arr.filter(t => t.status === "paused").length,
    cancelled: arr.filter(t => t.status === "cancelled").length,
    totalDownloaded: arr.reduce((s, t) => s + t.downloaded, 0),
    activeSpeed: arr
      .filter(t => t.status === "downloading")
      .map(t => t.speed)
      .join(", ") || "0 B/s",
  };
}

function inferAddon(url: string): string {
  if (/youtube\.com|youtu\.be/i.test(url)) return "youtube";
  if (/instagram\.com/i.test(url)) return "instagram";
  if (/facebook\.com|fb\.watch/i.test(url)) return "facebook";
  if (/twitter\.com|x\.com/i.test(url)) return "twitter";
  if (/tiktok\.com/i.test(url)) return "tiktok";
  if (/t\.me/i.test(url)) return "telegram";
  return "generic_http";
}

// GET /api/tasks
router.get("/tasks", (_req, res) => {
  const arr = [...tasks.values()].sort(
    (a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime(),
  );
  const status = _req.query.status as string | undefined;
  const filtered = status ? arr.filter(t => t.status === status) : arr;
  const limit = Number(_req.query.limit ?? 100);
  const offset = Number(_req.query.offset ?? 0);
  res.json({ tasks: filtered.slice(offset, offset + limit), total: filtered.length });
});

// GET /api/tasks/stats
router.get("/tasks/stats", (_req, res) => {
  res.json(getStats());
});

// GET /api/tasks/:id
router.get("/tasks/:id", (req, res) => {
  const task = tasks.get(req.params.id);
  if (!task) return void res.status(404).json({ error: "not found" });
  res.json(task);
});

// POST /api/tasks
router.post("/tasks", (req, res) => {
  const { url, outputDir, priority, addonHint } = req.body ?? {};
  if (!url) return void res.status(400).json({ error: "url required" });
  const task: Task = {
    id: randomUUID(), url,
    title: decodeURIComponent(url.split("/").filter(Boolean).pop() ?? url).slice(0, 80),
    status: "queued", progress: 0, downloaded: 0, totalSize: null,
    speed: "—", eta: "—",
    addonName: addonHint ?? inferAddon(url),
    outputDir: outputDir ?? "~/Downloads/NexDL",
    outputPath: null, error: null,
    priority: priority ?? 0, subtasksDone: 0, subtasksTotal: 0,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  };
  tasks.set(task.id, task);
  // Auto-start after 1 second (simulate processing → downloading)
  setTimeout(() => {
    const t = tasks.get(task.id);
    if (t && t.status === "queued") {
      t.status = "processing"; t.updatedAt = new Date().toISOString();
      setTimeout(() => {
        const t2 = tasks.get(task.id);
        if (t2 && t2.status === "processing") {
          t2.status = "downloading"; t2.totalSize = Math.floor(Math.random() * 50_000_000) + 5_000_000;
          t2.updatedAt = new Date().toISOString();
        }
      }, 1500);
    }
  }, 1000);
  res.status(201).json(task);
});

// DELETE /api/tasks/:id
router.delete("/tasks/:id", (req, res) => {
  if (!tasks.delete(req.params.id)) return void res.status(404).json({ error: "not found" });
  res.status(204).send();
});

// POST /api/tasks/:id/pause
router.post("/tasks/:id/pause", (req, res) => {
  const t = tasks.get(req.params.id);
  if (!t) return void res.status(404).json({ error: "not found" });
  if (t.status === "downloading" || t.status === "queued") {
    t.status = "paused"; t.speed = "0 B/s"; t.eta = "—"; t.updatedAt = new Date().toISOString();
  }
  res.json(t);
});

// POST /api/tasks/:id/resume
router.post("/tasks/:id/resume", (req, res) => {
  const t = tasks.get(req.params.id);
  if (!t) return void res.status(404).json({ error: "not found" });
  if (t.status === "paused") { t.status = "downloading"; t.updatedAt = new Date().toISOString(); }
  res.json(t);
});

// POST /api/tasks/:id/cancel
router.post("/tasks/:id/cancel", (req, res) => {
  const t = tasks.get(req.params.id);
  if (!t) return void res.status(404).json({ error: "not found" });
  t.status = "cancelled"; t.speed = "0 B/s"; t.eta = "—"; t.updatedAt = new Date().toISOString();
  res.json(t);
});

// POST /api/tasks/:id/retry
router.post("/tasks/:id/retry", (req, res) => {
  const t = tasks.get(req.params.id);
  if (!t) return void res.status(404).json({ error: "not found" });
  t.status = "queued"; t.progress = 0; t.downloaded = 0; t.error = null;
  t.speed = "—"; t.eta = "—"; t.updatedAt = new Date().toISOString();
  res.json(t);
});

export default router;
export { tasks };
