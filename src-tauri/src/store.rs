use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

/// フロントエンドに渡すアイテムの共通構造
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashItem {
    pub id: String,
    pub source_id: String,
    pub source_name: String,
    pub title: String,
    pub body: Option<String>,
    pub url: Option<String>,
    pub published_at: DateTime<Local>,
    pub tags: Vec<String>,
}

/// ソースごとの固定容量リングバッファ
struct RingBuffer {
    capacity: usize,
    items: std::collections::VecDeque<DashItem>,
    seen: HashSet<(String, String)>, // (source_id, item_id)
}

impl RingBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            items: std::collections::VecDeque::with_capacity(capacity),
            seen: HashSet::new(),
        }
    }

    /// 重複チェック付きでアイテムを追加。容量超過時は最古を破棄。
    fn push(&mut self, item: DashItem) -> bool {
        let key = (item.source_id.clone(), item.id.clone());
        if self.seen.contains(&key) {
            return false;
        }
        if self.items.len() >= self.capacity {
            if let Some(evicted) = self.items.pop_front() {
                self.seen.remove(&(evicted.source_id, evicted.id));
            }
        }
        self.seen.insert(key);
        self.items.push_back(item);
        true
    }

    fn items(&self) -> Vec<DashItem> {
        self.items.iter().cloned().collect()
    }

    /// 指定 ID のアイテムを削除。存在しない場合は何もしない。
    fn remove(&mut self, id: &str) {
        if let Some(pos) = self.items.iter().position(|i| i.id == id) {
            let item = self.items.remove(pos).unwrap();
            self.seen.remove(&(item.source_id, item.id));
        }
    }

    fn clear(&mut self) {
        self.items.clear();
        self.seen.clear();
    }
}

/// スレッドセーフなダッシュボードストア
#[derive(Clone)]
pub struct DashStore {
    inner: Arc<RwLock<RingBuffer>>,
}

impl DashStore {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(RingBuffer::new(capacity))),
        }
    }

    /// アイテムを追加。重複は無視。追加されたら true を返す。
    pub async fn push(&self, item: DashItem) -> bool {
        self.inner.write().await.push(item)
    }

    /// 全アイテムを published_at 降順で返す
    pub async fn all_items(&self) -> Vec<DashItem> {
        let mut items = self.inner.read().await.items();
        items.sort_by(|a, b| b.published_at.cmp(&a.published_at));
        items
    }

    /// 指定 ID のアイテムをストアから削除する
    pub async fn remove_item(&self, id: &str) {
        self.inner.write().await.remove(id);
    }

    /// クリップボード専用の push。
    /// - 同じタイトル（＝同じコピー内容）の既存クリップボードカードを先に削除する
    /// - クリップボードカードが max_items 以上あれば古い順に削除する
    /// - その後に新アイテムを追加する
    pub async fn push_clipboard(&self, item: DashItem, max_items: usize) {
        let mut buf = self.inner.write().await;

        // 同内容（タイトル一致）の古いクリップボードカードを削除
        let same_title_ids: Vec<String> = buf
            .items
            .iter()
            .filter(|i| i.source_id == "clipboard" && i.title == item.title)
            .map(|i| i.id.clone())
            .collect();
        for id in &same_title_ids {
            buf.remove(id);
        }

        // クリップボードカードが max_items 以上なら古い順に削除
        loop {
            let clipboard_count = buf.items.iter().filter(|i| i.source_id == "clipboard").count();
            if clipboard_count < max_items {
                break;
            }
            // 最古のクリップボードカードを探して削除
            let oldest_id = buf
                .items
                .iter()
                .find(|i| i.source_id == "clipboard")
                .map(|i| i.id.clone());
            if let Some(id) = oldest_id {
                buf.remove(&id);
            } else {
                break;
            }
        }

        buf.push(item);
    }

    pub async fn clear(&self) {
        self.inner.write().await.clear();
    }
}
