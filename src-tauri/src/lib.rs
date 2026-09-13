mod claude_code;
mod config;
mod providers;

use serde::Serialize;
use std::sync::Mutex;
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Rect, WindowEvent};

const TRAY_ID: &str = "main-tray";

struct TrayRectState(Mutex<Option<Rect>>);

#[derive(Debug, Serialize, Clone, Default)]
pub struct ProviderStatus {
    pub provider: String,
    pub has_key: bool,
    pub config_enabled: bool,
    pub active: bool,
    pub used: Option<f64>,
    pub limit: Option<f64>,
    pub unit: String,
    pub error: Option<String>,
    pub secondary_used: Option<f64>,
    pub secondary_limit: Option<f64>,
    pub secondary_label: Option<String>,
    pub primary_label: Option<String>,
}

#[tauri::command]
async fn fetch_all_usage(app: AppHandle) -> Result<Vec<ProviderStatus>, String> {
    let configs = config::load_config(&app)?;
    let mut results = Vec::with_capacity(configs.len());

    for cfg in configs {
        if cfg.provider == "claude-code" {
            if !cfg.enable {
                results.push(ProviderStatus {
                    provider: "claude-code".into(),
                    has_key: true,
                    config_enabled: false,
                    active: false,
                    ..Default::default()
                });
                continue;
            }

            match claude_code::compute_usage(&app) {
                Ok(usage) => {
                    results.push(ProviderStatus {
                        provider: "claude-code".into(),
                        has_key: true,
                        config_enabled: true,
                        active: true,
                        used: Some(usage.week_used_pct),
                        limit: Some(100.0),
                        unit: "%".into(),
                        primary_label: Some(usage.week_label),
                        secondary_used: Some(usage.session_used_pct),
                        secondary_limit: Some(100.0),
                        secondary_label: Some(usage.session_label),
                        ..Default::default()
                    });
                }
                Err(e) => {
                    results.push(ProviderStatus {
                        provider: "claude-code".into(),
                        has_key: true,
                        config_enabled: true,
                        active: true,
                        error: Some(e),
                        ..Default::default()
                    });
                }
            }
            continue;
        }

        let has_key = !cfg.key.trim().is_empty();
        let active = has_key && cfg.enable;

        if active {
            let usage = providers::fetch_usage(&cfg.provider, &cfg.key).await;
            results.push(ProviderStatus {
                provider: cfg.provider,
                has_key,
                config_enabled: cfg.enable,
                active,
                used: usage.used,
                limit: usage.limit,
                unit: usage.unit,
                error: usage.error,
                ..Default::default()
            });
        } else {
            results.push(ProviderStatus {
                provider: cfg.provider,
                has_key,
                config_enabled: cfg.enable,
                active,
                ..Default::default()
            });
        }
    }

    Ok(results)
}

#[tauri::command]
fn update_tray_status(app: AppHandle, tooltip: String, lines: Vec<String>) -> Result<(), String> {
    let tray = app
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| "tray icon not found".to_string())?;

    tray.set_tooltip(Some(tooltip)).map_err(|e| e.to_string())?;

    let display_lines = if lines.is_empty() {
        vec!["No provider enabled".to_string()]
    } else {
        lines
    };

    let info_items: Vec<MenuItem<tauri::Wry>> = display_lines
        .iter()
        .enumerate()
        .map(|(i, line)| MenuItem::with_id(&app, format!("info-{i}"), line, false, None::<&str>))
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    let separator = PredefinedMenuItem::separator(&app).map_err(|e| e.to_string())?;
    let toggle_item =
        MenuItem::with_id(&app, "toggle", "Show/Hide", true, None::<&str>)
            .map_err(|e| e.to_string())?;
    let refresh_item =
        MenuItem::with_id(&app, "refresh", "Refresh now", true, None::<&str>)
            .map_err(|e| e.to_string())?;
    let quit_item =
        MenuItem::with_id(&app, "quit", "Quit", true, None::<&str>).map_err(|e| e.to_string())?;

    let mut refs: Vec<&dyn IsMenuItem<tauri::Wry>> = Vec::new();
    for item in &info_items {
        refs.push(item);
    }
    refs.push(&separator);
    refs.push(&toggle_item);
    refs.push(&refresh_item);
    refs.push(&quit_item);

    let menu = Menu::with_items(&app, &refs).map_err(|e| e.to_string())?;
    tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
fn get_config_path(app: AppHandle) -> Result<String, String> {
    // ensure the file exists before exposing its path
    config::load_config(&app)?;
    Ok(config::config_path(&app).to_string_lossy().to_string())
}

#[tauri::command]
fn get_config(app: AppHandle) -> Result<Vec<config::ProviderConfig>, String> {
    config::load_config(&app)
}

#[tauri::command]
fn save_config(app: AppHandle, configs: Vec<config::ProviderConfig>) -> Result<(), String> {
    if configs.is_empty() {
        return Err("empty configuration rejected (nothing was saved)".into());
    }
    config::save_config(&app, &configs)
}

#[tauri::command]
fn open_config_file(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    config::load_config(&app)?;
    let path = config::config_path(&app);
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<String>)
        .map_err(|e| e.to_string())
}

fn toggle_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
            return;
        }

        if let Some(state) = app.try_state::<TrayRectState>() {
            let rect = state.0.lock().unwrap().clone();
            if let Some(rect) = rect {
                position_near_tray(&window, &rect);
            }
        }

        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn position_near_tray(window: &tauri::WebviewWindow, rect: &Rect) {
    let scale_factor = window.scale_factor().unwrap_or(1.0);
    let icon_pos = rect.position.to_physical::<f64>(scale_factor);
    let icon_size = rect.size.to_physical::<f64>(scale_factor);

    if let Ok(win_size) = window.outer_size() {
        let x = icon_pos.x + (icon_size.width / 2.0) - (win_size.width as f64 / 2.0);
        let y = icon_pos.y - win_size.height as f64 - 8.0;
        let _ = window.set_position(PhysicalPosition::new(x.max(0.0), y.max(0.0)));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            fetch_all_usage,
            get_config_path,
            get_config,
            save_config,
            open_config_file,
            update_tray_status
        ])
        .setup(|app| {
            app.manage(TrayRectState(Mutex::new(None)));

            let toggle_item =
                MenuItem::with_id(app, "toggle", "Show/Hide", true, None::<&str>)?;
            let refresh_item =
                MenuItem::with_id(app, "refresh", "Refresh now", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&toggle_item, &refresh_item, &quit_item])?;

            let _tray = TrayIconBuilder::with_id(TRAY_ID)
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Throttle")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "toggle" => toggle_main_window(app),
                    "refresh" => {
                        let _ = app.emit("refresh-requested", ());
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    let app = tray.app_handle();

                    let rect = match &event {
                        TrayIconEvent::Click { rect, .. }
                        | TrayIconEvent::DoubleClick { rect, .. }
                        | TrayIconEvent::Enter { rect, .. }
                        | TrayIconEvent::Move { rect, .. }
                        | TrayIconEvent::Leave { rect, .. } => Some(rect.clone()),
                        _ => None,
                    };
                    if let Some(rect) = rect {
                        if let Some(state) = app.try_state::<TrayRectState>() {
                            *state.0.lock().unwrap() = Some(rect);
                        }
                    }

                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_main_window(app);
                    }
                })
                .build(app)?;

            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                window.on_window_event(move |event| match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = window_clone.hide();
                    }
                    WindowEvent::Focused(false) => {
                        let _ = window_clone.hide();
                    }
                    _ => {}
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
