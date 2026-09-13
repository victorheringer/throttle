# AGENTS.md

Reference notes for AI coding agents working on this repo.

## What this is

Throttle — a Windows tray app (Tauri 2: Rust backend + vanilla HTML/CSS/TS frontend, no framework) that tracks usage/credits for OpenRouter, ElevenLabs, and Claude Code.

- `src-tauri/` — Rust backend (commands, tray, config I/O, provider fetchers)
- `src/` — frontend (`index.html`, `main.ts`, `styles.css`)
- `scripts/` — `claude-statusline.mjs` (see below) and `collect-installers.mjs`

## Commands

```bash
npm install
npm run tauri dev          # dev mode, hot reload
npm run build:installer    # release build + copies installers/*.exe|msi to installers/
npx tsc --noEmit           # typecheck frontend
cargo check                # (from src-tauri/) typecheck backend
```

Don't run `npm run tauri build` (or `build:installer`) while `npm run tauri dev` is running — it can silently kill the dev watcher, leaving an orphaned Vite process that still hot-reloads CSS/JS but never rebuilds Rust again. Stop dev mode first.

## Config

User-editable provider config lives outside the repo, at `%APPDATA%\com.victorheringer.usagetracker\config.json` — a JSON array of `{ provider, key, enable }`. A provider with no key is always treated as disabled regardless of `enable`, except `claude-code`, which needs no key.

The `identifier` in `tauri.conf.json` (`com.victorheringer.usagetracker`) is intentionally different from the product name ("Throttle") and must not be changed casually — it determines this config path and the Claude Code rate-limit cache path (below). Renaming it would orphan existing user config.

## Claude Code usage tracking

No public API exists for an individual account's usage %. Two data sources, in priority order:

1. **Real data**: the Claude Code CLI's statusline feature exposes `rate_limits.five_hour` / `rate_limits.seven_day` (`used_percentage`, `resets_at`) via stdin. `scripts/claude-statusline.mjs` must be wired up as the user's statusline command in `~/.claude/settings.json` (outside this repo, a manual one-time step — see README). It writes a cache to `%APPDATA%\com.victorheringer.usagetracker\claude-code-rate-limits.json`, which `src-tauri/src/claude_code.rs` reads.
   - The cache only updates while Claude Code is actively running. `claude_code.rs::effective_pct` zeroes out a percentage once its `resets_at` has passed, so a stale pre-reset value (e.g. near 100%) doesn't linger on-screen after the window actually reset server-side.
2. **Fallback**: if no cache exists, `compute_token_totals` parses `~/.claude/projects/**/*.jsonl` transcripts directly and computes a proportional estimate (today/week, last-5h/today). This is NOT the real quota — each transcript line for a given API turn repeats the same `message.id` multiple times (one per tool-call block), so token totals must be deduplicated by `message.id` or they'll be wildly overcounted.

## Known platform limits

- **Windows only.** Porting to Linux would need rework: `tray-icon`'s own docs state click events aren't emitted on Linux (only the right-click context menu works), so the whole "click tray icon to toggle a popup near it" interaction wouldn't carry over as-is.
- `window.hide()` called from the frontend needs the explicit `core:window:allow-hide` permission in `capabilities/default.json` — it's not part of `core:default`.
- A CSS rule that sets `display` on an element — even a low-specificity class selector — beats the browser's default `[hidden] { display: none }` rule, because author-origin styles always win over user-agent-origin styles regardless of specificity. Any element toggled via `el.hidden` in JS needs its visible-state CSS scoped as `selector:not([hidden])` (or `selector[hidden] { display: none }`), never a bare `selector { display: ... }`. Bit twice in this codebase already (settings view, disabled-providers list).

## Style

- No framework on the frontend; keep it that way unless asked.
- Comments: one line max, only for a genuinely non-obvious WHY. Default to none — this codebase had a pass specifically to trim over-commenting.
- The `--accent` color is a deliberate choice (deep emerald, not the generic AI-purple/indigo default) — don't casually change it. Note `--accent`/`--danger` are theme-adaptive (bright in dark mode) for use as pop colors, while `--accent-solid`/`--danger-solid` stay fixed-dark in both themes for anything with white text or a white icon on top of a solid fill.
