import { Router } from "express";

const router = Router();

interface Settings {
  outputDir: string;
  maxConcurrent: number;
  autoStart: boolean;
  theme: "dark" | "light" | "system";
  defaultAddon: string | null;
  proxyUrl: string | null;
  userAgent: string;
  maxRetries: number;
  speedLimitKbps: number | null;
}

const settings: Settings = {
  outputDir: "~/Downloads/NexDL",
  maxConcurrent: 4,
  autoStart: true,
  theme: "dark",
  defaultAddon: null,
  proxyUrl: null,
  userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
  maxRetries: 3,
  speedLimitKbps: null,
};

router.get("/settings", (_req, res) => {
  res.json(settings);
});

router.put("/settings", (req, res) => {
  const body = req.body ?? {};
  const allowed: (keyof Settings)[] = [
    "outputDir", "maxConcurrent", "autoStart", "theme",
    "defaultAddon", "proxyUrl", "userAgent", "maxRetries", "speedLimitKbps",
  ];
  for (const key of allowed) {
    if (key in body) (settings as Record<string, unknown>)[key] = body[key];
  }
  res.json(settings);
});

export default router;
