use std::time::Duration;

use chrono::Local;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tokio::time;
use uuid::Uuid;

use crate::config::ClipboardSourceConfig;
use crate::store::{DashItem, DashStore};

/// クリップボードを定期的に監視し、変化があったらストアに追加する
pub fn start(app: AppHandle, store: DashStore, config: ClipboardSourceConfig) {
    if !config.enabled {
        return;
    }

    tauri::async_runtime::spawn(async move {
        let mut last_text: Option<String> = None;
        let mut ticker = time::interval(Duration::from_secs(config.poll_interval_secs));

        loop {
            ticker.tick().await;

            // クリップボードのテキストを読み取る
            let text = match app.clipboard().read_text() {
                Ok(t) if !t.trim().is_empty() => t.trim().to_string(),
                _ => continue,
            };

            // 最小文字数チェック
            if text.len() < config.min_chars {
                continue;
            }

            // 前回と同じ内容は無視
            if last_text.as_deref() == Some(&text) {
                continue;
            }

            last_text = Some(text.clone());

            // URL か通常テキストかで title/url を振り分け
            let is_url = text.starts_with("http://") || text.starts_with("https://");
            let (title, url) = if is_url {
                // URL の場合: title は短縮表示、url に格納
                let display = if text.len() > 80 {
                    format!("{}…", &text[..80])
                } else {
                    text.clone()
                };
                (display, Some(text.clone()))
            } else {
                // テキストの場合: 1行目を title、本文全体を body に
                let first_line = text.lines().next().unwrap_or("").to_string();
                let title = if first_line.len() > 100 {
                    format!("{}…", &first_line[..100])
                } else {
                    first_line
                };
                (title, None)
            };

            let body = if is_url {
                None
            } else if text.lines().count() > 1 {
                Some(text.clone())
            } else {
                None
            };

            let item = DashItem {
                id: Uuid::new_v4().to_string(),
                source_id: "clipboard".to_string(),
                source_name: "Clipboard".to_string(),
                title,
                body,
                url,
                published_at: Local::now(),
                tags: if is_url {
                    vec!["url".to_string()]
                } else {
                    vec!["text".to_string()]
                },
            };

            store.push(item).await;
            let _ = app.emit("dashboard_updated", ());
            eprintln!("[clipboard] new item captured");
        }
    });
}
