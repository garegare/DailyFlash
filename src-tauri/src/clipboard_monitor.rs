use std::thread;
use std::time::Duration;

use chrono::Local;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::config::ClipboardSourceConfig;

/// 文字単位で最大 max_chars 文字に切り詰める（マルチバイト対応）
fn truncate_chars(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let mut result: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        result.push('…');
    }
    result
}
use crate::store::{DashItem, DashStore};

/// クリップボードを定期監視し、変化があったらストアに追加する。
/// arboard を OS スレッドで直接呼び出して macOS スレッド制限を回避する。
pub fn start(app: AppHandle, store: DashStore, config: ClipboardSourceConfig) {
    if !config.enabled {
        eprintln!("[clipboard] monitor disabled");
        return;
    }

    // tokio ランタイムハンドルをメインスレッドで取得しておく
    let rt = tauri::async_runtime::handle();

    thread::spawn(move || {
        // arboard::Clipboard は OS スレッドから生成する
        let mut clipboard = match arboard::Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[clipboard] failed to initialize: {e}");
                return;
            }
        };

        let mut last_text: Option<String> = None;

        eprintln!("[clipboard] monitor started (interval={}s, min_chars={})",
            config.poll_interval_secs, config.min_chars);

        loop {
            thread::sleep(Duration::from_secs(config.poll_interval_secs));

            // クリップボードのテキストを読み取る
            let text = match clipboard.get_text() {
                Ok(t) => {
                    let trimmed = t.trim().to_string();
                    if trimmed.is_empty() {
                        continue;
                    }
                    trimmed
                }
                Err(arboard::Error::ContentNotAvailable) => continue, // 空クリップボード
                Err(e) => {
                    eprintln!("[clipboard] read error: {e}");
                    continue;
                }
            };

            // 最小文字数チェック
            if text.len() < config.min_chars {
                continue;
            }

            // 前回と同じ内容は無視
            if last_text.as_deref() == Some(text.as_str()) {
                continue;
            }

            last_text = Some(text.clone());

            // URL か通常テキストかで title/url を振り分け
            let is_url = text.starts_with("http://") || text.starts_with("https://");
            let (title, url) = if is_url {
                let display = truncate_chars(&text, 80);
                (display, Some(text.clone()))
            } else {
                let first_line = text.lines().next().unwrap_or("").to_string();
                let title = truncate_chars(&first_line, 100);
                (title, None)
            };

            let body = if !is_url && text.lines().count() > 1 {
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

            rt.block_on(async {
                store.push_clipboard(item, config.max_items).await;
            });
            let _ = app.emit("dashboard_updated", ());
            eprintln!("[clipboard] new item captured");
        }
    });
}
