mod cert_manager;
mod commands;
mod config_engine;
mod error;
mod nginx_manager;
mod store;
mod validators;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::Connection;
use tracing::info;
use tracing_subscriber::EnvFilter;

use tauri::menu::{MenuBuilder, MenuItem, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, WindowEvent};

use crate::error::AppError;

#[derive(Clone)]
struct TrayMenuItems {
    status: MenuItem<tauri::Wry>,
    show: MenuItem<tauri::Wry>,
    start: MenuItem<tauri::Wry>,
    stop: MenuItem<tauri::Wry>,
    reload: MenuItem<tauri::Wry>,
    add_rule: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

/// Read the language setting from the database, defaulting to "zh".
fn get_language(state: &AppState) -> String {
    state
        .lock_db()
        .ok()
        .and_then(|db| store::settings_repo::get(&db, "language").ok().flatten())
        .unwrap_or_else(|| "zh".to_string())
}

/// Update all tray menu items to reflect current engine status and language.
fn sync_tray_menu(data_dir: &Path, items: &TrayMenuItems, lang: &str) {
    let running = nginx_manager::status(data_dir).status == "running";
    let zh = lang == "zh";

    let _ = items.status.set_text(match (running, zh) {
        (true, true) => "● 运行中",
        (true, false) => "● Running",
        (false, true) => "● 已停止",
        (false, false) => "● Stopped",
    });
    let _ = items.show.set_text(if zh { "显示窗口" } else { "Show Window" });
    let _ = items.start.set_text(if zh { "启动" } else { "Start" });
    let _ = items.stop.set_text(if zh { "停止" } else { "Stop" });
    let _ = items.reload.set_text(if zh { "重载配置" } else { "Reload" });
    let _ = items.add_rule.set_text(if zh { "新建规则" } else { "New Rule" });
    let _ = items.quit.set_text(if zh { "退出" } else { "Quit" });

    let _ = items.start.set_enabled(!running);
    let _ = items.stop.set_enabled(running);
    let _ = items.reload.set_enabled(running);
}

pub struct AppState {
    pub db: Mutex<Connection>,
    pub data_dir: PathBuf,
}

impl AppState {
    /// Helper to lock the database mutex, converting the PoisonError into AppError.
    pub fn lock_db(&self) -> Result<std::sync::MutexGuard<'_, Connection>, AppError> {
        self.db.lock().map_err(|e| {
            AppError::Database(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
                Some(format!("Lock poisoned: {}", e)),
            ))
        })
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
            std::fs::create_dir_all(nginx_dir.join("certs"))
                .expect("Failed to create certs directory");

            // Initialize database
            let db_path = app_data_dir.join("meridian.db");
            let conn = store::init_database(&db_path)
                .expect("Failed to initialize database");

            let state = AppState {
                db: Mutex::new(conn),
                data_dir: app_data_dir.clone(),
            };

            app.manage(state);

            // Build tray menu (placeholder text, will be set by sync_tray_menu)
            let status_i = MenuItemBuilder::with_id("status", "-")
                .enabled(false)
                .build(app)?;
            let sep0 = tauri::menu::PredefinedMenuItem::separator(app)?;
            let show_i = MenuItemBuilder::with_id("show", "-").build(app)?;
            let sep1 = tauri::menu::PredefinedMenuItem::separator(app)?;
            let start_i = MenuItemBuilder::with_id("start", "-").build(app)?;
            let stop_i = MenuItemBuilder::with_id("stop", "-").build(app)?;
            let reload_i = MenuItemBuilder::with_id("reload", "-").build(app)?;
            let sep2 = tauri::menu::PredefinedMenuItem::separator(app)?;
            let add_i = MenuItemBuilder::with_id("add_rule", "-").build(app)?;
            let sep3 = tauri::menu::PredefinedMenuItem::separator(app)?;
            let quit_i = MenuItemBuilder::with_id("quit", "-").build(app)?;

            let tray_menu = MenuBuilder::new(app)
                .item(&status_i)
                .item(&sep0)
                .item(&show_i)
                .item(&sep1)
                .item(&start_i)
                .item(&stop_i)
                .item(&reload_i)
                .item(&sep2)
                .item(&add_i)
                .item(&sep3)
                .item(&quit_i)
                .build()?;

            let items = TrayMenuItems {
                status: status_i,
                show: show_i,
                start: start_i,
                stop: stop_i,
                reload: reload_i,
                add_rule: add_i,
                quit: quit_i,
            };

            // Set initial tray menu state
            let app_state = app.state::<AppState>();
            let lang = get_language(&app_state);
            sync_tray_menu(&app_data_dir, &items, &lang);

            let me_items = items.clone();
            let te_items = items.clone();

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&tray_menu)
                .tooltip("轻渡 · Meridian")
                .on_menu_event(move |app, event| {
                    let id = event.id().as_ref();
                    let state = app.state::<AppState>();
                    match id {
                        "show" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.unminimize();
                                let _ = w.set_focus();
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
                        "reload" => {
                            let _ = nginx_manager::reload(&state.data_dir);
                            let lang = get_language(&state);
                            sync_tray_menu(&state.data_dir, &me_items, &lang);
                        }
                        "add_rule" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.unminimize();
                                let _ = w.set_focus();
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
                            if let Some(w) = tray.app_handle().get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.unminimize();
                                let _ = w.set_focus();
                            }
                        }
                        tauri::tray::TrayIconEvent::Click {
                            button: tauri::tray::MouseButton::Right,
                            ..
                        } => {
                            let state = tray.app_handle().state::<AppState>();
                            let lang = get_language(&state);
                            sync_tray_menu(&state.data_dir, &te_items, &lang);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            info!("Meridian initialized. Data dir: {:?}", app_data_dir);

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Hide window instead of closing — app stays in tray
                api.prevent_close();
                let _ = window.hide();
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
            // Certificate commands
            commands::cert::list_certificates,
            commands::cert::get_certificate,
            commands::cert::generate_self_signed_cert,
            commands::cert::import_certificate,
            commands::cert::delete_certificate,
            commands::cert::check_expiring_certs,
            // Access list commands
            commands::access::list_access_lists,
            commands::access::get_access_list,
            commands::access::create_access_list,
            commands::access::update_access_list,
            commands::access::delete_access_list,
            commands::access::create_access_rule,
            commands::access::delete_access_rule,
            commands::access::reorder_access_rules,
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
            // Settings commands
            commands::settings::get_setting,
            commands::settings::set_setting,
            commands::settings::list_settings,
            commands::settings::export_data,
            commands::settings::import_data,
            commands::settings::backup_database,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
