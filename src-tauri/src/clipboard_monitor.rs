use std::thread;
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::Local;
use image::{DynamicImage, ImageBuffer, Rgba};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::config::ClipboardSourceConfig;
use crate::store::{DashItem, DashStore};

/// 文字単位で最大 max_chars 文字に切り詰める（マルチバイト対応）
fn truncate_chars(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let mut result: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        result.push('…');
    }
    result
}

/// arboard::ImageData を PNG にエンコードして base64 data URL に変換する
fn encode_image_to_data_url(img: &arboard::ImageData) -> Option<String> {
    let ib = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(
        img.width as u32,
        img.height as u32,
        img.bytes.to_vec(),
    )?;
    let dynamic = DynamicImage::ImageRgba8(ib);
    let mut cursor = std::io::Cursor::new(Vec::<u8>::new());
    dynamic
        .write_to(&mut cursor, image::ImageFormat::Png)
        .ok()?;
    let b64 = STANDARD.encode(cursor.into_inner());
    Some(format!("data:image/png;base64,{b64}"))
}

/// 直前のクリップボード内容（重複検出用）
enum LastContent {
    Text(String),
    /// 画像の生バイト列サイズ（同一画像の簡易判定に使用）
    ImageBytesLen(usize),
}

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

        let mut last: Option<LastContent> = None;

        eprintln!(
            "[clipboard] monitor started (interval={}s, min_chars={}, max_items={})",
            config.poll_interval_secs, config.min_chars, config.max_items
        );

        loop {
            thread::sleep(Duration::from_secs(config.poll_interval_secs));

            // ---- テキストを試みる ----
            match clipboard.get_text() {
                Ok(t) => {
                    let trimmed = t.trim().to_string();
                    if trimmed.is_empty() {
                        continue;
                    }
                    // 最小文字数チェック（char 単位でマルチバイト対応）
                    if trimmed.chars().count() < config.min_chars {
                        continue;
                    }
                    // 前回と同じテキストは無視
                    if let Some(LastContent::Text(ref prev)) = last {
                        if prev == &trimmed {
                            continue;
                        }
                    }
                    last = Some(LastContent::Text(trimmed.clone()));

                    // URL か通常テキストかで title/url を振り分け
                    let is_url =
                        trimmed.starts_with("http://") || trimmed.starts_with("https://");
                    let (title, url) = if is_url {
                        let display = truncate_chars(&trimmed, 80);
                        (display, Some(trimmed.clone()))
                    } else {
                        let first_line = trimmed.lines().next().unwrap_or("").to_string();
                        let title = truncate_chars(&first_line, 100);
                        (title, None)
                    };

                    let body = if !is_url && trimmed.lines().count() > 1 {
                        Some(trimmed.clone())
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
                        image_data: None,
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
                    eprintln!("[clipboard] new text captured");
                }

                Err(arboard::Error::ContentNotAvailable) => {
                    // ---- テキストがない場合は画像を試みる ----
                    match clipboard.get_image() {
                        Ok(img) => {
                            let bytes_len = img.bytes.len();
                            // 前回と同じバイト数の画像は無視（同一画像の簡易判定）
                            if let Some(LastContent::ImageBytesLen(prev_len)) = last {
                                if prev_len == bytes_len {
                                    continue;
                                }
                            }
                            last = Some(LastContent::ImageBytesLen(bytes_len));

                            let width = img.width;
                            let height = img.height;

                            match encode_image_to_data_url(&img) {
                                Some(data_url) => {
                                    // 同解像度の画像は同タイトルになり push_clipboard が古いものを削除する
                                    let title = format!("🖼 画像 ({}×{})", width, height);
                                    let item = DashItem {
                                        id: Uuid::new_v4().to_string(),
                                        source_id: "clipboard".to_string(),
                                        source_name: "Clipboard".to_string(),
                                        title,
                                        body: None,
                                        url: None,
                                        image_data: Some(data_url),
                                        published_at: Local::now(),
                                        tags: vec!["image".to_string()],
                                    };
                                    rt.block_on(async {
                                        store
                                            .push_clipboard(item, config.max_items)
                                            .await;
                                    });
                                    let _ = app.emit("dashboard_updated", ());
                                    eprintln!(
                                        "[clipboard] new image captured ({}×{})",
                                        width, height
                                    );
                                }
                                None => {
                                    eprintln!("[clipboard] image encode failed");
                                }
                            }
                        }
                        Err(_) => continue, // 画像もなければ何もしない
                    }
                }

                Err(e) => {
                    eprintln!("[clipboard] read error: {e}");
                    continue;
                }
            }
        }
    });
}
