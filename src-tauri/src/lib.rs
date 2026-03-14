use std::sync::Arc;

mod config;
mod connectors;
mod error;
mod scheduler;
mod server;
mod store;

use config::Config;
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

// ---- エントリポイント ----

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = Config::load_or_default();
    let store = DashStore::new(config.memory.default_capacity);

    tauri::Builder::default()
        .manage(config.clone())
        .manage(store.clone())
        .invoke_handler(tauri::generate_handler![
            refresh_dashboard,
            get_config,
            clear_store
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();

            // コネクタを Config から構築
            let connectors: Vec<Arc<dyn connectors::Connector>> = config
                .sources
                .rss
                .as_ref()
                .map(|rss| {
                    rss.feeds
                        .iter()
                        .map(|f| {
                            Arc::new(RssConnector::new(f.clone()))
                                as Arc<dyn connectors::Connector>
                        })
                        .collect()
                })
                .unwrap_or_default();

            // Pull スケジューラ起動
            let poll_interval = config
                .sources
                .rss
                .as_ref()
                .map(|r| r.poll_interval_secs)
                .unwrap_or(300);
            scheduler::start(app_handle.clone(), store.clone(), connectors, poll_interval);

            // Push サーバー起動
            server::start(
                app_handle,
                store,
                config.server.port,
                config.server.auth_token.clone(),
            );

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
