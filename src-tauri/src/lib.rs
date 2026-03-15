use std::sync::Arc;

mod clipboard_monitor;
mod config;
mod connectors;
mod error;
mod scheduler;
mod server;
mod store;

use config::Config;
use connectors::github::GithubConnector;
use connectors::rss::RssConnector;
use store::{DashItem, DashStore};

// ---- Tauri コマンド ----

#[tauri::command]
async fn refresh_dashboard(
    store: tauri::State<'_, DashStore>,
) -> Result<Vec<DashItem>, error::AppError> {
    Ok(store.all_items().await)
}

#[tauri::command]
async fn get_config(
    config: tauri::State<'_, Config>,
) -> Result<serde_json::Value, error::AppError> {
    Ok(serde_json::to_value(config.inner()).unwrap_or_default())
}

#[tauri::command]
async fn clear_store(store: tauri::State<'_, DashStore>) -> Result<(), error::AppError> {
    store.clear().await;
    Ok(())
}

/// アイテムをストアから削除する
#[tauri::command]
async fn delete_item(
    store: tauri::State<'_, DashStore>,
    id: String,
) -> Result<(), error::AppError> {
    store.remove_item(&id).await;
    Ok(())
}

/// アイテムをローカル JSON ファイルにブックマーク保存する
#[tauri::command]
async fn bookmark_item(
    app: tauri::AppHandle,
    store: tauri::State<'_, DashStore>,
    id: String,
) -> Result<String, error::AppError> {
    use tauri::Manager;

    // ストアから対象アイテムを探す
    let items = store.all_items().await;
    let item = items
        .into_iter()
        .find(|i| i.id == id)
        .ok_or_else(|| error::AppError::Validation("Item not found".to_string()))?;

    // 保存先: {app_config_dir}/bookmarks.json
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| error::AppError::Validation(e.to_string()))?;
    std::fs::create_dir_all(&config_dir)?;
    let bookmarks_path = config_dir.join("bookmarks.json");

    // 既存のブックマークを読み込む
    let mut bookmarks: Vec<DashItem> = if bookmarks_path.exists() {
        let content = std::fs::read_to_string(&bookmarks_path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        vec![]
    };

    // 重複チェック（同じ ID は追加しない）
    if !bookmarks.iter().any(|b| b.id == item.id) {
        bookmarks.push(item);
        let json = serde_json::to_string_pretty(&bookmarks)
            .map_err(|e| error::AppError::Validation(e.to_string()))?;
        std::fs::write(&bookmarks_path, json)?;
    }

    Ok(bookmarks_path.to_string_lossy().to_string())
}

/// 現在のダッシュボードアイテムを JSON ファイルにエクスポートする
#[tauri::command]
async fn export_items(
    app: tauri::AppHandle,
    store: tauri::State<'_, DashStore>,
) -> Result<String, error::AppError> {
    use tauri::Manager;
    use chrono::Local;

    let items = store.all_items().await;
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");

    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| error::AppError::Validation(e.to_string()))?;
    std::fs::create_dir_all(&config_dir)?;
    let export_path = config_dir.join(format!("export_{timestamp}.json"));

    let json = serde_json::to_string_pretty(&items)
        .map_err(|e| error::AppError::Validation(e.to_string()))?;
    std::fs::write(&export_path, json)?;

    Ok(export_path.to_string_lossy().to_string())
}

// ---- エントリポイント ----

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = Config::load_or_default();
    let store = DashStore::new(config.memory.default_capacity);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(config.clone())
        .manage(store.clone())
        .invoke_handler(tauri::generate_handler![
            refresh_dashboard,
            get_config,
            clear_store,
            delete_item,
            bookmark_item,
            export_items,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();

            // RSS コネクタを Config から構築
            let mut all_connectors: Vec<Arc<dyn connectors::Connector>> = config
                .sources
                .rss
                .as_ref()
                .map(|rss| {
                    let source_lookback = rss.lookback_days;
                    rss.feeds
                        .iter()
                        .map(|f| {
                            // フィード個別の lookback_days が設定されていればそちらを優先
                            let lookback = f.lookback_days.unwrap_or(source_lookback);
                            Arc::new(RssConnector::new(f.clone(), lookback))
                                as Arc<dyn connectors::Connector>
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            // GitHub コネクタ
            if let Some(gh) = config.sources.github.clone() {
                all_connectors.push(Arc::new(GithubConnector::new(gh)));
            }

            // Pull スケジューラ起動（RSS と GitHub の間隔は RSS の値を代表値として使用）
            let poll_interval = config
                .sources
                .rss
                .as_ref()
                .map(|r| r.poll_interval_secs)
                .or_else(|| config.sources.github.as_ref().map(|g| g.poll_interval_secs))
                .unwrap_or(300);
            scheduler::start(app_handle.clone(), store.clone(), all_connectors, poll_interval);

            // Push サーバー起動
            server::start(
                app_handle.clone(),
                store.clone(),
                config.server.port,
                config.server.auth_token.clone(),
            );

            // クリップボード監視起動
            let clipboard_cfg = config
                .sources
                .clipboard
                .clone()
                .unwrap_or_default();
            clipboard_monitor::start(app_handle.clone(), store, clipboard_cfg);

            // ---- タスクトレイ設定 ----
            setup_tray(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use tauri::Manager;

    let show_item = MenuItemBuilder::with_id("show", "ウィンドウを表示").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "終了").build(app)?;
    let menu = MenuBuilder::new(app).items(&[&show_item, &quit_item]).build()?;

    let app_handle = app.handle().clone();
    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("DailyFlash")
        .menu(&menu)
        .on_menu_event(move |_app, event| match event.id().as_ref() {
            "quit" => {
                _app.exit(0);
            }
            "show" => {
                if let Some(window) = _app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            _ => {}
        })
        .on_tray_icon_event(move |_tray, event| {
            // 左クリックでウィンドウをトグル
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if let Some(window) = app_handle.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    // ウィンドウの × ボタンで終了せず非表示にする
    if let Some(window) = app.get_webview_window("main") {
        let w = window.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = w.hide();
            }
        });
    }

    Ok(())
}
