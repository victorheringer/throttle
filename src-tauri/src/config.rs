use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProviderConfig {
    pub provider: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub enable: bool,
}

pub fn config_path(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_config_dir()
        .expect("failed to resolve app config dir");
    dir.join("config.json")
}

fn default_config() -> Vec<ProviderConfig> {
    vec![
        ProviderConfig {
            provider: "openrouter".into(),
            key: String::new(),
            enable: false,
        },
        ProviderConfig {
            provider: "elevenlabs".into(),
            key: String::new(),
            enable: false,
        },
        ProviderConfig {
            provider: "claude-code".into(),
            key: String::new(),
            enable: false,
        },
    ]
}

pub fn load_config(app: &AppHandle) -> Result<Vec<ProviderConfig>, String> {
    let path = config_path(app);
    if !path.exists() {
        let dir = path.parent().ok_or("invalid config path")?;
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let defaults = default_config();
        let json = serde_json::to_string_pretty(&defaults).map_err(|e| e.to_string())?;
        fs::write(&path, json).map_err(|e| e.to_string())?;
        return Ok(defaults);
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let parsed: Vec<ProviderConfig> =
        serde_json::from_str(&raw).map_err(|e| format!("invalid config.json: {e}"))?;
    Ok(parsed)
}

pub fn save_config(app: &AppHandle, configs: &[ProviderConfig]) -> Result<(), String> {
    let path = config_path(app);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(configs).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}
