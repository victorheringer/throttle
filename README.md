# Throttle

Windows tray app to track usage/credits for:

- **OpenRouter** — used/available credits
- **ElevenLabs** — used/available characters
- **Claude Code** — session (5h) and weekly usage %

Click the tray icon to open the panel; click outside (or the ✕ button) to close it.

## Running in development

```bash
npm install
npm run tauri dev
```

## Provider configuration

Done through the UI itself: open the panel → gear icon (⚙) → paste the API key and enable the provider → the toggle saves itself automatically.

Data lives in `%APPDATA%\com.victorheringer.usagetracker\config.json`. You can also edit that file manually (the "Edit config.json manually" link in the settings screen).

## ⚠️ Claude Code needs an extra step

Unlike OpenRouter and ElevenLabs, Claude Code **has no public API** to query the individual plan's usage % (5h session / week). The only way to get the **real** number is through the Claude Code CLI's own **statusline** feature, which exposes `rate_limits.five_hour` and `rate_limits.seven_day` via stdin.

Because of that, enabling Claude Code in the app requires configuring a statusline in `~/.claude/settings.json` (Claude Code's global config file, **outside this project**):

```json
{
  "statusLine": {
    "type": "command",
    "command": "node \"C:\\Projects\\usage\\scripts\\claude-statusline.mjs\""
  }
}
```

The script (`scripts/claude-statusline.mjs`) reads the JSON that Claude Code sends to the statusline command, extracts the `rate_limits.five_hour.used_percentage` and `rate_limits.seven_day.used_percentage` fields, and saves them to a cache file (`%APPDATA%\com.victorheringer.usagetracker\claude-code-rate-limits.json`) that the app reads. It also prints something simple (`5h: 34% · 7d: 11%`) to act as a real statusline in your terminal.

**Side effects of setting this up:**
- A status line will start showing up in your Claude Code terminal (if you already had a statusline configured, this change overwrites it).
- The cache only updates while you're actively using Claude Code (the statusline only runs during a session). If it's been a while since you last opened Claude Code, the number may be stale.

**Without this configuration**, the app automatically falls back to an approximate mode (labeled "(approx.)" on the bars), computed locally from the transcript logs in `~/.claude/projects/**/*.jsonl` (ratio of today's/session tokens over the week/day) — this is not the real number for your limit, just a proportional estimate.

## Stack

- [Tauri 2](https://tauri.app/) (Rust + WebView) — `src-tauri/`
- Frontend: plain HTML/CSS/TS (no framework), Vite — `src/`
