import { Router } from "express";
import { randomUUID } from "crypto";

const router = Router();

interface ScheduleEntry {
  id: string;
  name: string;
  triggerType: "once" | "interval" | "cron" | "on_startup";
  triggerValue: string | null;
  action: "download" | "run_addon" | "cleanup";
  actionTarget: string | null;
  enabled: boolean;
  lastRun: string | null;
  nextRun: string | null;
}

const entries = new Map<string, ScheduleEntry>();

// Seed demo schedules
const demoEntries: ScheduleEntry[] = [
  {
    id: randomUUID(), name: "Daily cleanup",
    triggerType: "cron", triggerValue: "0 3 * * *",
    action: "cleanup", actionTarget: null,
    enabled: true,
    lastRun: new Date(Date.now() - 86400000).toISOString(),
    nextRun: new Date(Date.now() + 3600000 * 6).toISOString(),
  },
  {
    id: randomUUID(), name: "Download playlist on startup",
    triggerType: "on_startup", triggerValue: null,
    action: "download", actionTarget: "https://www.youtube.com/playlist?list=PLexample",
    enabled: false, lastRun: null, nextRun: null,
  },
  {
    id: randomUUID(), name: "Run Telegram bot",
    triggerType: "on_startup", triggerValue: null,
    action: "run_addon", actionTarget: "telegram_bot",
    enabled: true,
    lastRun: new Date(Date.now() - 7200000).toISOString(),
    nextRun: null,
  },
];
demoEntries.forEach(e => entries.set(e.id, e));

router.get("/scheduler/entries", (_req, res) => {
  res.json([...entries.values()]);
});

router.post("/scheduler/entries", (req, res) => {
  const { name, triggerType, triggerValue, action, actionTarget, enabled } = req.body ?? {};
  if (!name || !triggerType || !action) {
    return void res.status(400).json({ error: "name, triggerType, and action required" });
  }
  const entry: ScheduleEntry = {
    id: randomUUID(), name, triggerType, triggerValue: triggerValue ?? null,
    action, actionTarget: actionTarget ?? null, enabled: enabled ?? true,
    lastRun: null, nextRun: null,
  };
  entries.set(entry.id, entry);
  res.status(201).json(entry);
});

router.patch("/scheduler/entries/:id", (req, res) => {
  const entry = entries.get(req.params.id);
  if (!entry) return void res.status(404).json({ error: "not found" });
  const { name, triggerType, triggerValue, action, actionTarget, enabled } = req.body ?? {};
  if (name !== undefined) entry.name = name;
  if (triggerType !== undefined) entry.triggerType = triggerType;
  if (triggerValue !== undefined) entry.triggerValue = triggerValue;
  if (action !== undefined) entry.action = action;
  if (actionTarget !== undefined) entry.actionTarget = actionTarget;
  if (enabled !== undefined) entry.enabled = enabled;
  res.json(entry);
});

router.delete("/scheduler/entries/:id", (req, res) => {
  if (!entries.delete(req.params.id)) return void res.status(404).json({ error: "not found" });
  res.status(204).send();
});

export default router;
