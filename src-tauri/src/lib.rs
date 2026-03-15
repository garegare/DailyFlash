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

/// ~ をホームディレクトリに展開する
fn expand_tilde(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs_next::home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(path)
}

/// bookmarks.json のパスを解決する（Config 指定優先、なければ fallback_dir/bookmarks.json）
fn resolve_bookmarks_path(
    storage: &config::StorageConfig,
    fallback_dir: Option<std::path::PathBuf>,
) -> Option<std::path::PathBuf> {
    if let Some(ref configured) = storage.bookmarks_path {
        Some(expand_tilde(configured))
    } else {
        fallback_dir.map(|d| d.join("bookmarks.json"))
    }
}

/// bookmarks.json を読み込んで DashItem のリストを返す
fn load_bookmarks(path: &std::path::Path) -> Vec<DashItem> {
    if !path.exists() {
        return vec![];
    }
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[bookmark] read error: {e}");
            return vec![];
        }
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// アイテムをローカル JSON ファイルにブックマーク保存する
#[tauri::command]
async fn bookmark_item(
    app: tauri::AppHandle,
    config: tauri::State<'_, Config>,
    store: tauri::State<'_, DashStore>,
    id: String,
) -> Result<String, error::AppError> {
    use tauri::{Emitter, Manager};

    // ストアから対象アイテムを探す
    let items = store.all_items().await;
    let mut item = items
        .into_iter()
        .find(|i| i.id == id)
        .ok_or_else(|| error::AppError::Validation("Item not found".to_string()))?;

    // "bookmark" タグを付与（まだなければ）
    if !item.tags.contains(&"bookmark".to_string()) {
        item.tags.push("bookmark".to_string());
    }

    // 保存先パスを解決
    let fallback_dir = app.path().app_config_dir().ok();
    let bookmarks_path = resolve_bookmarks_path(&config.storage, fallback_dir)
        .ok_or_else(|| error::AppError::Validation("bookmarks path unavailable".to_string()))?;
    if let Some(parent) = bookmarks_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // 既存のブックマークを読み込む
    let mut bookmarks: Vec<DashItem> = if bookmarks_path.exists() {
        let content = std::fs::read_to_string(&bookmarks_path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        vec![]
    };

    // 重複チェック（同じ ID は上書き保存）
    bookmarks.retain(|b| b.id != item.id);
    bookmarks.push(item.clone());
    let json = serde_json::to_string_pretty(&bookmarks)
        .map_err(|e| error::AppError::Validation(e.to_string()))?;
    std::fs::write(&bookmarks_path, json)?;

    // ストアの既存アイテムを bookmark タグ付きで置き換えてダッシュボードを更新
    store.remove_item(&item.id).await;
    store.push(item).await;
    let _ = app.emit("dashboard_updated", ());

    Ok(bookmarks_path.to_string_lossy().to_string())
}

/// メモアイテムをストアに追加する
#[tauri::command]
async fn add_note(
    app: tauri::AppHandle,
    store: tauri::State<'_, DashStore>,
    text: String,
) -> Result<(), error::AppError> {
    use tauri::Emitter;
    use chrono::Local;

    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(error::AppError::Validation("テキストが空です".to_string()));
    }

    // 1行目をタイトルに、全文を body に保存（編集時に全文を使う）
    let first_line: String = text.lines().next().unwrap_or("").chars().take(80).collect();
    let title = if first_line.is_empty() {
        "メモ".to_string()
    } else {
        first_line
    };

    let id = format!("note-{}", uuid::Uuid::new_v4());
    let item = DashItem {
        id,
        source_id: "note".to_string(),
        source_name: "メモ".to_string(),
        title,
        body: Some(text),
        url: None,
        image_data: None,
        published_at: Local::now(),
        tags: vec!["note".to_string()],
    };

    store.push(item).await;
    let _ = app.emit("dashboard_updated", ());
    Ok(())
}

/// メモアイテムを編集する（テキストを更新し published_at は保持）
#[tauri::command]
async fn edit_note(
    app: tauri::AppHandle,
    store: tauri::State<'_, DashStore>,
    id: String,
    text: String,
) -> Result<(), error::AppError> {
    use tauri::Emitter;

    let text = text.trim().to_string();
    if text.is_empty() {
        return Err(error::AppError::Validation("テキストが空です".to_string()));
    }

    let items = store.all_items().await;
    let item = items
        .into_iter()
        .find(|i| i.id == id)
        .ok_or_else(|| error::AppError::Validation("Note not found".to_string()))?;

    let first_line: String = text.lines().next().unwrap_or("").chars().take(80).collect();
    let title = if first_line.is_empty() {
        "メモ".to_string()
    } else {
        first_line
    };

    let mut updated = item;
    updated.title = title;
    updated.body = Some(text);

    store.remove_item(&id).await;
    store.push(updated).await;
    let _ = app.emit("dashboard_updated", ());
    Ok(())
}

/// ブックマークを解除する（bookmarks.json から削除し、ストアのタグも外す）
#[tauri::command]
async fn unbookmark_item(
    app: tauri::AppHandle,
    config: tauri::State<'_, Config>,
    store: tauri::State<'_, DashStore>,
    id: String,
) -> Result<(), error::AppError> {
    use tauri::{Emitter, Manager};

    // bookmarks.json から該当アイテムを削除
    let fallback_dir = app.path().app_config_dir().ok();
    if let Some(bm_path) = resolve_bookmarks_path(&config.storage, fallback_dir) {
        if bm_path.exists() {
            let content = std::fs::read_to_string(&bm_path)?;
            let mut bookmarks: Vec<DashItem> = serde_json::from_str(&content).unwrap_or_default();
            bookmarks.retain(|b| b.id != id);
            let json = serde_json::to_string_pretty(&bookmarks)
                .map_err(|e| error::AppError::Validation(e.to_string()))?;
            std::fs::write(&bm_path, json)?;
        }
    }

    // ストアから対象アイテムを探す
    let items = store.all_items().await;
    if let Some(item) = items.into_iter().find(|i| i.id == id) {
        store.remove_item(&id).await;
        if item.source_id != "bookmark" {
            // 通常ソースのアイテム: bookmark タグを外してストアに戻す
            let mut updated = item;
            updated.tags.retain(|t| t != "bookmark");
            store.push(updated).await;
        }
        // source_id == "bookmark" のアーカイブ済みアイテムはストアから削除のみ
    }

    let _ = app.emit("dashboard_updated", ());
    Ok(())
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
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(config.clone())
        .manage(store.clone())
        .invoke_handler(tauri::generate_handler![
            refresh_dashboard,
            get_config,
            clear_store,
            delete_item,
            bookmark_item,
            unbookmark_item,
            add_note,
            edit_note,
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

            // ブックマークを起動時にストアへ読み込む
            use tauri::Manager;
            let fallback_dir = app.path().app_config_dir().ok();
            if let Some(bm_path) = resolve_bookmarks_path(&config.storage, fallback_dir) {
                let bookmarks = load_bookmarks(&bm_path);
                let count = bookmarks.len();
                let rt = tauri::async_runtime::handle();
                for mut item in bookmarks {
                    // ソースを "bookmark" に統一（「すべて」フィルタから除外するため）
                    item.source_id = "bookmark".to_string();
                    item.source_name = "Bookmark".to_string();
                    if !item.tags.contains(&"bookmark".to_string()) {
                        item.tags.push("bookmark".to_string());
                    }
                    let s = store.clone();
                    rt.block_on(async move { s.push(item).await; });
                }
                eprintln!("[bookmark] loaded {count} bookmarks from {}", bm_path.display());
            }

            // クリップボード監視起動
            let clipboard_cfg = config
                .sources
                .clipboard
                .clone()
                .unwrap_or_default();
            clipboard_monitor::start(app_handle.clone(), store, clipboard_cfg);

            // ---- タスクトレイ設定 ----
            setup_tray(app)?;

            // ---- グローバルショートカット: Cmd+Shift+N でメモ入力を開く ----
            {
                use tauri::{Emitter, Manager};
                use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

                let shortcut: Shortcut = "CommandOrControl+Shift+N".parse()
                    .expect("invalid shortcut");
                let handle = app.handle().clone();
                app.handle().global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        // ウィンドウを前面に出してからメモ入力イベントを送信
                        if let Some(window) = handle.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                        let _ = handle.emit("open_note_input", ());
                    }
                })?;
            }

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
