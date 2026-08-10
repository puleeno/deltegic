"""
NexDL Telegram Bot Addon
Provides a Telegram bot interface for NexDL.
Users can send URLs to the bot and receive downloads.
Requires TELEGRAM_BOT_TOKEN set in NexDL settings.

Usage as a scheduled addon (not a URL handler):
    scheduler.add_addon("telegram_bot", method="run_bot", args={})
"""
import re
import os
import nexdl


class TelegramBotAddon(nexdl.Addon):

    def name(self) -> str:
        return "telegram-bot"

    def version(self) -> str:
        return "1.0.0"

    def description(self) -> str:
        return "Telegram Bot interface — send URLs to NexDL via Telegram"

    def author(self) -> str:
        return "NexDL Team"

    def supported_sites(self) -> list[str]:
        return []  # Not a URL downloader; runs as a service

    def can_handle(self, url: str) -> bool:
        return False  # We're a bot addon, not a URL handler

    def run_bot(self, ctx: nexdl.Context) -> None:
        """Start the Telegram bot polling loop.
        Called by the scheduler or manually via RunAddon action.
        """
        token = ctx.get_option("TELEGRAM_BOT_TOKEN", "")
        if not token:
            ctx.log("error", "TELEGRAM_BOT_TOKEN not set in options")
            return

        ctx.log("info", "Starting Telegram bot polling...")
        offset = 0

        while True:
            # Long-poll for updates
            try:
                resp = ctx.http.get_json(
                    f"https://api.telegram.org/bot{token}/getUpdates"
                    f"?timeout=30&offset={offset}"
                )

                updates = resp.get("result", [])
                for update in updates:
                    offset = update["update_id"] + 1
                    self._handle_update(update, token, ctx)

            except Exception as e:
                ctx.log("warn", f"Bot poll error: {e}")
                import time
                time.sleep(5)

    def _handle_update(self, update: dict, token: str, ctx: nexdl.Context) -> None:
        message = update.get("message", {})
        chat_id = message.get("chat", {}).get("id")
        text = message.get("text", "").strip()

        if not chat_id or not text:
            return

        if text.startswith("/start"):
            self._send_message(token, chat_id,
                "🚀 NexDL Bot ready!\nSend me any URL to download.\n\n"
                "Commands:\n/status — show queue status\n/list — list downloads")

        elif text.startswith("/status"):
            self._send_message(token, chat_id, "📊 Queue status: (checking...)")

        elif text.startswith("/list"):
            self._send_message(token, chat_id, "📋 Download list: (checking...)")

        elif text.startswith("http"):
            # URL received — add to download queue
            url = text.split()[0]
            output_dir = ctx.get_option("output_dir", "~/Downloads/NexDL/telegram")
            self._send_message(token, chat_id, f"⬇️ Added to queue:\n{url}")
            # Store URL for the engine to pick up
            storage_key = f"queue/{update['update_id']}.json"
            import json
            ctx.storage.write_json(storage_key, json.dumps({"url": url, "output_dir": output_dir}))

        else:
            self._send_message(token, chat_id,
                "Send me a URL to download, or use /status to check the queue.")

    def _send_message(self, token: str, chat_id: int, text: str) -> None:
        import json
        url = f"https://api.telegram.org/bot{token}/sendMessage"
        body = json.dumps({"chat_id": chat_id, "text": text, "parse_mode": "HTML"})
        # Use requests directly here since we don't have ctx
        try:
            import urllib.request
            req = urllib.request.Request(url, data=body.encode(), method="POST",
                                          headers={"Content-Type": "application/json"})
            urllib.request.urlopen(req, timeout=10)
        except Exception:
            pass
