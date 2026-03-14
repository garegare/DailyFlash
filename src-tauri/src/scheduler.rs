use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::time;

use crate::connectors::Connector;
use crate::store::DashStore;

/// 全コネクタを定期実行するスケジューラを起動する
pub fn start(
    app: AppHandle,
    store: DashStore,
    connectors: Vec<Arc<dyn Connector>>,
    interval_secs: u64,
) {
    tokio::spawn(async move {
        let mut ticker = time::interval(Duration::from_secs(interval_secs));
        ticker.tick().await; // 最初のティックは即座に発火するためスキップ

        loop {
            ticker.tick().await;
            run_once(&app, &store, &connectors).await;
        }
    });
}

/// 全コネクタを並行実行し、取得結果を DashStore に追加する
pub async fn run_once(
    app: &AppHandle,
    store: &DashStore,
    connectors: &[Arc<dyn Connector>],
) {
    let handles: Vec<_> = connectors
        .iter()
        .map(|c| {
            let connector = Arc::clone(c);
            let store = store.clone();
            tokio::spawn(async move {
                match connector.fetch().await {
                    Ok(items) => {
                        for item in items {
                            store.push(item).await;
                        }
                    }
                    Err(e) => {
                        eprintln!("[scheduler] connector '{}' error: {}", connector.source_id(), e);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        let _ = h.await;
    }

    let _ = app.emit("dashboard_updated", ());
}
