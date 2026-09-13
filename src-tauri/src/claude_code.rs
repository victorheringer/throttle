use chrono::{DateTime, Datelike, Local, Utc};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Deserialize)]
struct TranscriptUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
}

#[derive(Deserialize)]
struct TranscriptMessage {
    id: Option<String>,
    usage: Option<TranscriptUsage>,
}

#[derive(Deserialize)]
struct TranscriptLine {
    timestamp: Option<String>,
    message: Option<TranscriptMessage>,
}

#[derive(Deserialize)]
struct RateLimitWindow {
    used_percentage: Option<f64>,
    resets_at: Option<i64>,
}

#[derive(Deserialize)]
struct RateLimitCache {
    five_hour: Option<RateLimitWindow>,
    seven_day: Option<RateLimitWindow>,
}

/// Zeroes out a percentage past its reset time, since the cache only
/// refreshes while Claude Code is running and can otherwise stay stale.
fn effective_pct(window: &RateLimitWindow, now: i64) -> Option<f64> {
    let pct = window.used_percentage?;
    match window.resets_at {
        Some(resets_at) if now >= resets_at => Some(0.0),
        _ => Some(pct),
    }
}

pub struct ClaudeCodeUsage {
    pub week_used_pct: f64,
    pub week_label: String,
    pub session_used_pct: f64,
    pub session_label: String,
}

fn projects_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let home = app.path().home_dir().map_err(|e| e.to_string())?;
    Ok(home.join(".claude").join("projects"))
}

fn rate_limit_cache_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?;
    Ok(dir.join("claude-code-rate-limits.json"))
}

fn read_rate_limit_cache(app: &AppHandle) -> Option<(RateLimitWindow, RateLimitWindow)> {
    let path = rate_limit_cache_path(app).ok()?;
    let raw = fs::read_to_string(path).ok()?;
    let cache: RateLimitCache = serde_json::from_str(&raw).ok()?;
    let five_hour = cache.five_hour?;
    let seven_day = cache.seven_day?;
    five_hour.used_percentage?;
    seven_day.used_percentage?;
    Some((five_hour, seven_day))
}

fn collect_jsonl_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            out.push(path);
        }
    }
}

fn compute_token_totals(app: &AppHandle) -> Result<(u64, u64, u64), String> {
    let dir = projects_dir(app)?;
    if !dir.exists() {
        return Err("~/.claude/projects folder not found".into());
    }

    let mut files = Vec::new();
    collect_jsonl_files(&dir, &mut files);

    let now = Local::now();
    let today = now.date_naive();
    let week_start = today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64);
    let session_start = Utc::now() - chrono::Duration::hours(5);

    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut session_tokens: u64 = 0;
    let mut today_tokens: u64 = 0;
    let mut week_tokens: u64 = 0;

    for file in files {
        let Ok(content) = fs::read_to_string(&file) else {
            continue;
        };

        for line in content.lines() {
            if !line.contains("\"usage\"") {
                continue;
            }

            let Ok(entry) = serde_json::from_str::<TranscriptLine>(line) else {
                continue;
            };
            let Some(message) = entry.message else {
                continue;
            };
            let Some(usage) = message.usage else {
                continue;
            };
            let Some(id) = message.id else {
                continue;
            };
            let Some(ts) = entry.timestamp else {
                continue;
            };

            if !seen_ids.insert(id) {
                continue;
            }

            let Ok(parsed_ts) = DateTime::parse_from_rfc3339(&ts) else {
                continue;
            };
            let local_date = parsed_ts.with_timezone(&Local).date_naive();
            let utc_ts = parsed_ts.with_timezone(&Utc);

            let total = usage.input_tokens.unwrap_or(0)
                + usage.output_tokens.unwrap_or(0)
                + usage.cache_creation_input_tokens.unwrap_or(0)
                + usage.cache_read_input_tokens.unwrap_or(0);

            if local_date >= week_start {
                week_tokens += total;
            }
            if local_date == today {
                today_tokens += total;
            }
            if utc_ts >= session_start {
                session_tokens += total;
            }
        }
    }

    Ok((today_tokens, week_tokens, session_tokens))
}

pub fn compute_usage(app: &AppHandle) -> Result<ClaudeCodeUsage, String> {
    if let Some((five_hour, seven_day)) = read_rate_limit_cache(app) {
        let now = Utc::now().timestamp();
        if let (Some(five_hour_pct), Some(seven_day_pct)) =
            (effective_pct(&five_hour, now), effective_pct(&seven_day, now))
        {
            return Ok(ClaudeCodeUsage {
                week_used_pct: seven_day_pct,
                week_label: "This week".into(),
                session_used_pct: five_hour_pct,
                session_label: "Current session (5h)".into(),
            });
        }
    }

    let (today_tokens, week_tokens, session_tokens) = compute_token_totals(app)?;

    let week_used_pct = if week_tokens > 0 {
        (today_tokens as f64 / week_tokens as f64) * 100.0
    } else {
        0.0
    };
    let session_used_pct = if today_tokens > 0 {
        (session_tokens as f64 / today_tokens as f64) * 100.0
    } else {
        0.0
    };

    Ok(ClaudeCodeUsage {
        week_used_pct,
        week_label: "Week (approx.)".into(),
        session_used_pct,
        session_label: "Last 5h (approx.)".into(),
    })
}
