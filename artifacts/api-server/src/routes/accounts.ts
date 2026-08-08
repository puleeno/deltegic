import { Router } from "express";
import { randomUUID } from "crypto";

const router = Router();

interface Account {
  id: string;
  label: string;
  site: string;
  username: string | null;
  cookieCount: number;
  createdAt: string;
}

const accounts = new Map<string, Account>([
  [randomUUID(), {
    id: randomUUID(), label: "YouTube Personal",
    site: "youtube.com", username: "user@gmail.com",
    cookieCount: 12, createdAt: new Date(Date.now() - 86400000 * 7).toISOString(),
  }],
  [randomUUID(), {
    id: randomUUID(), label: "Instagram Main",
    site: "instagram.com", username: "@my_instagram",
    cookieCount: 8, createdAt: new Date(Date.now() - 86400000 * 3).toISOString(),
  }],
  [randomUUID(), {
    id: randomUUID(), label: "Facebook Work",
    site: "facebook.com", username: null,
    cookieCount: 5, createdAt: new Date(Date.now() - 86400000).toISOString(),
  }],
]);

// Fix: seed map with correct keys
const accountsFixed = new Map<string, Account>();
for (const acc of accounts.values()) {
  accountsFixed.set(acc.id, acc);
}
accounts.clear();
for (const [k, v] of accountsFixed) {
  accounts.set(k, v);
}

router.get("/accounts", (_req, res) => {
  res.json([...accounts.values()]);
});

router.get("/accounts/:id", (req, res) => {
  const acc = accounts.get(req.params.id);
  if (!acc) return void res.status(404).json({ error: "not found" });
  res.json(acc);
});

router.post("/accounts", (req, res) => {
  const { label, site, username, cookies } = req.body ?? {};
  if (!label || !site) return void res.status(400).json({ error: "label and site required" });
  const cookieCount = cookies
    ? (cookies.match(/\S+/g) ?? []).filter((_: string, i: number) => i % 2 === 0).length
    : 0;
  const acc: Account = {
    id: randomUUID(), label, site,
    username: username ?? null, cookieCount,
    createdAt: new Date().toISOString(),
  };
  accounts.set(acc.id, acc);
  res.status(201).json(acc);
});

router.patch("/accounts/:id", (req, res) => {
  const acc = accounts.get(req.params.id);
  if (!acc) return void res.status(404).json({ error: "not found" });
  const { label, username, cookies } = req.body ?? {};
  if (label !== undefined) acc.label = label;
  if (username !== undefined) acc.username = username;
  if (cookies !== undefined) {
    acc.cookieCount = (cookies.match(/\S+/g) ?? []).filter((_: string, i: number) => i % 2 === 0).length;
  }
  res.json(acc);
});

router.delete("/accounts/:id", (req, res) => {
  if (!accounts.delete(req.params.id)) return void res.status(404).json({ error: "not found" });
  res.status(204).send();
});

export default router;
