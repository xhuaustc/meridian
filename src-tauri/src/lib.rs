mod acme_client;
mod cert_manager;
mod commands;
mod config_engine;
mod dns_provider;
mod error;
mod hosts_manager;
mod metrics;
mod nginx_manager;
mod store;
mod validators;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tracing::info;
use tracing_subscriber::EnvFilter;

use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, WebviewWindow, WindowEvent};

use crate::error::AppError;

/// Set macOS dock icon visibility by changing the activation policy.
#[cfg(target_os = "macos")]
fn set_dock_visible(visible: bool) {
    use objc2_app_kit::NSApplicationActivationPolicy;

    let mtm = unsafe { objc2_foundation::MainThreadMarker::new_unchecked() };
    let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
    let policy = if visible {
        NSApplicationActivationPolicy::Regular
    } else {
        NSApplicationActivationPolicy::Accessory
    };
    app.setActivationPolicy(policy);
}

/// Show a window and make the dock icon visible.
fn show_window(window: &WebviewWindow) {
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
    #[cfg(target_os = "macos")]
    set_dock_visible(true);
}

/// Hide a window and remove the dock icon.
fn hide_window(window: &tauri::Window) {
    let _ = window.hide();
    #[cfg(target_os = "macos")]
    set_dock_visible(false);
}

#[derive(Clone)]
/// Tray menu items that need updating when language or engine status changes.
pub struct TrayMenuItems {
    status: MenuItem<tauri::Wry>,
    show: MenuItem<tauri::Wry>,
    start: MenuItem<tauri::Wry>,
    stop: MenuItem<tauri::Wry>,
    add_rule: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

/// Detect OS language: returns "zh" if any system locale starts with "zh", otherwise "en".
#[cfg(not(windows))]
fn detect_os_language() -> String {
    std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .map(|v| if v.to_lowercase().starts_with("zh") { "zh".to_string() } else { "en".to_string() })
        .unwrap_or_else(|_| "en".to_string())
}

#[cfg(windows)]
fn detect_os_language() -> String {
    // Try LANG env var first (set by some tools like Git Bash)
    if let Ok(v) = std::env::var("LANG") {
        if v.to_lowercase().starts_with("zh") {
            return "zh".to_string();
        }
    }
    // Use Windows GetUserDefaultLocaleName via PowerShell
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-Command", "[System.Globalization.CultureInfo]::CurrentUICulture.Name"]);
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    if let Ok(output) = cmd.output() {
        let locale = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
        if locale.starts_with("zh") {
            return "zh".to_string();
        }
    }
    "en".to_string()
}

/// Read the language setting from the database, defaulting to OS locale.
fn get_language(state: &AppState) -> String {
    state
        .get_conn()
        .ok()
        .and_then(|db| store::settings_repo::get(&db, "language").ok().flatten())
        .unwrap_or_else(detect_os_language)
}

/// Update all tray menu items to reflect current engine status and language.
fn format_uptime(seconds: u64) -> String {
    if seconds < 60 {
        return format!("{}s", seconds);
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{}m", minutes);
    }
    let hours = minutes / 60;
    let remaining_m = minutes % 60;
    if hours < 24 {
        return format!("{}h {}m", hours, remaining_m);
    }
    let days = hours / 24;
    let remaining_h = hours % 24;
    format!("{}d {}h", days, remaining_h)
}

fn sync_tray_menu(data_dir: &Path, items: &TrayMenuItems, lang: &str) {
    let status_info = nginx_manager::status(data_dir);
    let running = status_info.status == "running";
    let zh = lang == "zh";

    let status_text = if running {
        let uptime_str = status_info
            .uptime_seconds
            .map(|s| format!(" · {}", format_uptime(s)))
            .unwrap_or_default();
        if zh {
            format!("● 运行中{}", uptime_str)
        } else {
            format!("● Running{}", uptime_str)
        }
    } else if zh {
        "● 已停止".to_string()
    } else {
        "● Stopped".to_string()
    };
    let _ = items.status.set_text(&status_text);
    let _ = items.show.set_text(if zh { "显示窗口" } else { "Show Window" });
    let _ = items.start.set_text(if zh { "启动" } else { "Start" });
    let _ = items.stop.set_text(if zh { "停止" } else { "Stop" });
    let _ = items.add_rule.set_text(if zh { "添加代理" } else { "Add Proxy" });
    let _ = items.quit.set_text(if zh { "退出" } else { "Quit" });

    let _ = items.start.set_enabled(!running);
    let _ = items.stop.set_enabled(running);
}

pub struct AppState {
    pub pool: store::DbPool,
    pub data_dir: PathBuf,
    pub tray_items: Mutex<Option<TrayMenuItems>>,
}

impl AppState {
    /// Get a connection from the pool.
    pub fn get_conn(&self) -> Result<store::PooledConn, AppError> {
        self.pool.get().map_err(|e| {
            AppError::Database(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
                Some(format!("Pool error: {}", e)),
            ))
        })
    }
}

/// Command to sync tray menu items (called from frontend when language changes).
#[tauri::command]
fn sync_tray(state: tauri::State<'_, AppState>) {
    let lang = get_language(&state);
    let guard = state.tray_items.lock().unwrap();
    if let Some(ref items) = *guard {
        sync_tray_menu(&state.data_dir, items, &lang);
    }
}

#[tauri::command]
fn get_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to resolve app data directory");

            std::fs::create_dir_all(&app_data_dir)
                .expect("Failed to create app data directory");

            // Ensure nginx subdirectories exist
            let nginx_dir = app_data_dir.join("nginx");
            std::fs::create_dir_all(nginx_dir.join("conf.d"))
                .expect("Failed to create conf.d directory");
            std::fs::create_dir_all(nginx_dir.join("stream.d"))
                .expect("Failed to create stream.d directory");
            std::fs::create_dir_all(nginx_dir.join("logs"))
                .expect("Failed to create logs directory");
            std::fs::create_dir_all(nginx_dir.join("temp"))
                .expect("Failed to create temp directory");
            std::fs::create_dir_all(nginx_dir.join("certs"))
                .expect("Failed to create certs directory");

            // Initialize database pool
            let db_path = app_data_dir.join("meridian.db");
            let pool = store::init_pool(&db_path)
                .expect("Failed to initialize database pool");

            let state = AppState {
                pool: pool.clone(),
                data_dir: app_data_dir.clone(),
                tray_items: Mutex::new(None),
            };

            app.manage(state);

            // Build tray menu (placeholder text, will be set by sync_tray_menu)
            let status_i = MenuItemBuilder::with_id("status", "-")
                .enabled(false)
                .build(app)?;
            let sep0 = tauri::menu::PredefinedMenuItem::separator(app)?;
            let show_i = MenuItemBuilder::with_id("show", "-").build(app)?;
            let start_i = MenuItemBuilder::with_id("start", "-").build(app)?;
            let stop_i = MenuItemBuilder::with_id("stop", "-").build(app)?;
            let sep1 = tauri::menu::PredefinedMenuItem::separator(app)?;
            let add_i = MenuItemBuilder::with_id("add_rule", "-").build(app)?;
            let quit_i = MenuItemBuilder::with_id("quit", "-").build(app)?;

            let tray_menu = MenuBuilder::new(app)
                .item(&status_i)
                .item(&sep0)
                .item(&show_i)
                .item(&start_i)
                .item(&stop_i)
                .item(&sep1)
                .item(&add_i)
                .item(&quit_i)
                .build()?;

            let items = TrayMenuItems {
                status: status_i,
                show: show_i,
                start: start_i,
                stop: stop_i,
                add_rule: add_i,
                quit: quit_i,
            };

            // Set initial tray menu state
            let app_state = app.state::<AppState>();
            let lang = get_language(&app_state);
            sync_tray_menu(&app_data_dir, &items, &lang);

            // Store tray items in AppState for sync_tray command
            {
                let mut guard = app_state.tray_items.lock().unwrap();
                *guard = Some(items.clone());
            }

            let me_items = items.clone();
            let te_items = items.clone();

            let tray_icon = {
                let bytes = include_bytes!("../icons/tray-icon@2x.png");
                let img = image::load_from_memory(bytes).expect("Failed to load tray icon");
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                tauri::image::Image::new_owned(rgba.into_raw(), w, h)
            };

            TrayIconBuilder::new()
                .icon(tray_icon)
                .icon_as_template(true)
                .tooltip(if lang == "zh" { "轻渡 · Meridian" } else { "Meridian" })
                .show_menu_on_left_click(false)
                .menu(&tray_menu)
                .on_menu_event(move |app, event| {
                    let id = event.id().as_ref();
                    let state = app.state::<AppState>();
                    match id {
                        "show" => {
                            if let Some(w) = app.get_webview_window("main") {
                                show_window(&w);
                            }
                        }
                        "start" => {
                            let _ = nginx_manager::start(&state.data_dir);
                            let lang = get_language(&state);
                            sync_tray_menu(&state.data_dir, &me_items, &lang);
                        }
                        "stop" => {
                            let _ = nginx_manager::stop(&state.data_dir);
                            let lang = get_language(&state);
                            sync_tray_menu(&state.data_dir, &me_items, &lang);
                        }
                        "add_rule" => {
                            if let Some(w) = app.get_webview_window("main") {
                                show_window(&w);
                                let _ = w.eval("window.__navigate && window.__navigate('/proxy/new')");
                            }
                        }
                        "quit" => {
                            let _ = nginx_manager::stop(&state.data_dir);
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(move |tray, event| {
                    match event {
                        tauri::tray::TrayIconEvent::Click {
                            button: tauri::tray::MouseButton::Left,
                            ..
                        } => {
                            // Left click: show window only, no menu
                            if let Some(w) = tray.app_handle().get_webview_window("main") {
                                show_window(&w);
                            }
                        }
                        tauri::tray::TrayIconEvent::Click {
                            button: tauri::tray::MouseButton::Right,
                            ..
                        } => {
                            // Right click: sync menu state before it shows
                            let state = tray.app_handle().state::<AppState>();
                            let lang = get_language(&state);
                            sync_tray_menu(&state.data_dir, &te_items, &lang);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // Clean up any stale nginx process from a previous session
            nginx_manager::cleanup_stale_process(&app_data_dir);

            // Always generate nginx configs on startup so nginx.conf exists
            // for status checks even when auto-start is disabled.
            {
                let app_state = app.state::<AppState>();
                if let Ok(db) = app_state.get_conn() {
                    let rules = store::proxy_repo::list_enabled(&db).unwrap_or_default();
                    let certs = store::cert_repo::list_all(&db).unwrap_or_default();
                    let access_lists_raw = store::access_repo::list_all_lists(&db).unwrap_or_default();
                    let worker_processes = store::settings_repo::get(&db, "worker_processes")
                        .ok().flatten().unwrap_or_else(|| "2".to_string());
                    let mut access_lists = Vec::new();
                    for al in &access_lists_raw {
                        if let Ok(rules) = store::access_repo::list_rules_by_list(&db, &al.id) {
                            access_lists.push((al.clone(), rules));
                        }
                    }
                    drop(db);
                    let _ = config_engine::generate_all_configs_with_settings(
                        &app_data_dir, &rules, &certs, &access_lists, &worker_processes,
                    );
                }
            }

            // Auto-start engine if setting is enabled
            let auto_start = {
                let app_state = app.state::<AppState>();
                app_state.get_conn().ok()
                    .and_then(|db| store::settings_repo::get(&db, "auto_start_engine").ok().flatten())
                    .map_or(false, |v| v == "true")
            };
            if auto_start {
                info!("Auto-starting engine");
                let _ = nginx_manager::start(&app_data_dir);
                // Refresh tray status
                let app_state = app.state::<AppState>();
                let lang = get_language(&app_state);
                sync_tray_menu(&app_data_dir, &items, &lang);
            }

            // Log retention cleanup
            {
                let app_state = app.state::<AppState>();
                let retention_days = app_state.get_conn().ok()
                    .and_then(|db| store::settings_repo::get(&db, "log_retention_days").ok().flatten())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(7);
                let logs_dir = app_data_dir.join("nginx/logs");
                commands::logs::cleanup_old_logs(&logs_dir, retention_days);
            }

            // Spawn auto-renewal background task
            acme_client::renewal::spawn_renewal_task(
                pool.clone(),
                app_data_dir.clone(),
            );

            // Spawn nginx health check
            nginx_manager::spawn_health_check(
                app_data_dir.clone(),
                app.handle().clone(),
            );

            // Spawn scheduled log cleanup
            commands::logs::spawn_log_cleanup_task(
                pool.clone(),
                app_data_dir.clone(),
            );

            info!("Meridian initialized. Data dir: {:?}", app_data_dir);

            // Enforce minimum window size programmatically (config values may not
            // be applied on all platforms).
            if let Some(window) = app.get_webview_window("main") {
                use tauri::LogicalSize;
                let _ = window.set_min_size(Some(LogicalSize::new(900.0, 600.0)));
            }

            #[cfg(target_os = "macos")]
            if let Some(window) = app.get_webview_window("main") {
                use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
                let _ = apply_vibrancy(
                    &window,
                    NSVisualEffectMaterial::Sidebar,
                    None,
                    None,
                );
                info!("Applied macOS sidebar vibrancy effect");
            }

            #[cfg(target_os = "windows")]
            if let Some(window) = app.get_webview_window("main") {
                use window_vibrancy::apply_mica;
                if apply_mica(&window, None).is_ok() {
                    let _ = window.eval("document.documentElement.setAttribute('data-mica', '')");
                    info!("Applied Windows Mica material effect");
                } else {
                    info!("Mica not available (pre-Win11), using solid sidebar fallback");
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Hide window instead of closing — app stays in tray
                api.prevent_close();
                hide_window(window);
            }
        })
        .invoke_handler(tauri::generate_handler![
            // Proxy commands
            commands::proxy::list_proxies,
            commands::proxy::get_proxy,
            commands::proxy::create_proxy,
            commands::proxy::update_proxy,
            commands::proxy::delete_proxy,
            commands::proxy::toggle_proxy,
            commands::proxy::batch_toggle_proxies,
            commands::proxy::batch_delete_proxies,
            // Certificate commands
            commands::cert::list_certificates,
            commands::cert::get_certificate,
            commands::cert::generate_self_signed_cert,
            commands::cert::import_certificate,
            commands::cert::delete_certificate,
            commands::cert::export_certificate,
            commands::cert::check_expiring_certs,
            // DNS credential commands
            commands::dns_credential::list_dns_credentials,
            commands::dns_credential::create_dns_credential,
            commands::dns_credential::update_dns_credential,
            commands::dns_credential::delete_dns_credential,
            commands::dns_credential::test_dns_credential,
            // ACME commands
            commands::acme::request_acme_cert,
            commands::acme::get_acme_renewal_status,
            // Access list commands
            commands::access::list_access_lists,
            commands::access::get_access_list,
            commands::access::create_access_list,
            commands::access::update_access_list,
            commands::access::delete_access_list,
            commands::access::create_access_rule,
            commands::access::delete_access_rule,
            commands::access::reorder_access_rules,
            // Host management commands
            commands::hosts::list_hosts,
            commands::hosts::create_host,
            commands::hosts::update_host,
            commands::hosts::delete_host,
            commands::hosts::toggle_host,
            commands::hosts::check_hostname_exists,
            commands::hosts::sync_hosts_file,
            // Engine commands
            commands::engine::get_engine_status,
            commands::engine::start_engine,
            commands::engine::stop_engine,
            commands::engine::reload_engine,
            commands::engine::restart_engine,
            commands::engine::apply_config,
            commands::engine::test_nginx_config,
            commands::engine::detect_conflicts,
            commands::engine::check_port_conflict,
            // Log commands
            commands::logs::read_access_log,
            commands::logs::read_error_log,
            commands::logs::clear_logs,
            // Metrics commands
            commands::metrics::get_proxy_metrics,
            // Settings commands
            commands::settings::get_setting,
            commands::settings::set_setting,
            commands::settings::list_settings,
            commands::settings::export_data,
            commands::settings::import_data,
            commands::settings::backup_database,
            sync_tray,
            get_platform,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
