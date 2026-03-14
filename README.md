# DailyFlash

> **「今日、この瞬間」だけを映すエフェメラル統合ダッシュボード**

情報の蓄積を排除し、アプリを閉じれば全データが消去される、揮発性の情報ハブ。

---

## 目次

1. [プロジェクト概要](#1-プロジェクト概要)
2. [コア・コンセプト](#2-コアコンセプト)
3. [技術スタック](#3-技術スタック)
4. [アーキテクチャ](#4-アーキテクチャ)
5. [ディレクトリ構造](#5-ディレクトリ構造)
6. [データフロー](#6-データフロー)
7. [コネクタ設計](#7-コネクタ設計)
8. [Push受信サーバー仕様](#8-push受信サーバー仕様)
9. [Windows常駐仕様](#9-windows常駐仕様)
10. [設定ファイル仕様](#10-設定ファイル仕様)
11. [エラーハンドリング方針](#11-エラーハンドリング方針)
12. [フロントエンド設計](#12-フロントエンド設計)
13. [ビルド・開発](#13-ビルド開発)
14. [今後の拡張候補](#14-今後の拡張候補)

---

## 1. プロジェクト概要

DailyFlash は「情報の蓄積」を意図的に排除したダッシュボードアプリ。

- **揮発性**: アプリ終了でデータは完全消去。永続ストレージへの書き込みは行わない
- **今日限定**: RSS・Push いずれも「当日」のアイテムのみを表示対象とする
- **低コンテキスト負荷**: 読み切れなかった情報は翌日には存在しない設計

---

## 2. コア・コンセプト

| 概念 | 説明 |
|------|------|
| **エフェメラル** | プロセス終了 = データ消去。SQLite・ファイルキャッシュ等への永続化は行わない |
| **コネクタ方式** | `Connector` トレイトで抽象化された Pull 型ソース（RSS, GitHub 等）を動的に追加可能 |
| **プッシュ受容** | 内蔵 HTTP サーバーが外部アプリ（CI、監視ツール等）からのリアルタイム通知を受信 |
| **リングバッファ** | ソースごとに最大保持件数を設定。容量超過時は最古アイテムを自動破棄 |
| **重複排除** | アイテムを `(source_id, item_id)` の組でハッシュ管理し、Pull 周期をまたいだ重複追加を防止 |

---

## 3. 技術スタック

### バックエンド (Rust)

| ライブラリ | 用途 |
|-----------|------|
| `tauri` | デスクトップアプリ基盤・IPC |
| `tokio` | 非同期ランタイム |
| `axum` | Push 受信用ローカル HTTP サーバー |
| `reqwest` | RSS 取得用 HTTP クライアント |
| `feed-rs` | RSS/Atom フィード解析 |
| `chrono` | 日付フィルタリング |
| `serde` / `toml` | 設定ファイル管理 |
| `async_trait` | 非同期トレイト実装 |
| `tokio::sync::RwLock` | DashStore のスレッドセーフ読み書き |

### フロントエンド (TypeScript)

| ライブラリ | 用途 |
|-----------|------|
| `React` | UI フレームワーク |
| `@tauri-apps/api` | Tauri IPC バインディング |
| `TanStack Query` | コマンド呼び出しとキャッシュ管理 |
| `Tailwind CSS` | スタイリング |

> **フロントエンド技術選定の補足**: Svelte も候補だが、エコシステムの広さと型安全性から React + TypeScript を採用。状態管理は TanStack Query で Tauri コマンドを `queryFn` として扱うことで、ポーリング間隔制御・ローディング状態管理を一元化できる。

---

## 4. アーキテクチャ

```
┌─────────────────────────────────────────────────────────┐
│  Tauri App (Rust プロセス)                               │
│                                                         │
│  ┌──────────┐   Pull (定期)   ┌────────────────────┐   │
│  │ Connector │ ─────────────→ │                    │   │
│  │  (RSS等)  │                │    DashStore       │   │
│  └──────────┘                │  (Arc<RwLock<      │   │
│                              │   RingBuffer>>)    │   │
│  ┌──────────┐   Push (即時)  │                    │   │
│  │  axum    │ ─────────────→ │                    │   │
│  │  Server  │                └────────┬───────────┘   │
│  └──────────┘                         │               │
│                                       │ emit event    │
│  ┌──────────────────────┐             ↓               │
│  │  Tauri Commands      │  ←── refresh_dashboard      │
│  │  - refresh_dashboard │                             │
│  │  - get_config        │                             │
│  └──────────┬───────────┘                             │
└─────────────│───────────────────────────────────────────┘
              │ IPC (invoke / listen)
┌─────────────↓───────────────────────────────────────────┐
│  WebView (React フロントエンド)                          │
│  - アイテム一覧表示                                     │
│  - ソースフィルタ / ソート                              │
│  - Push 受信時のリアルタイム更新                        │
└─────────────────────────────────────────────────────────┘
```

---

## 5. ディレクトリ構造

```
DailyFlash/
├── index.html                    # Vite エントリポイント
├── package.json                  # フロントエンド依存関係
├── vite.config.ts                # Vite 設定 (port 1420)
├── tsconfig.json                 # TypeScript 設定
├── src/                          # フロントエンド (React + TypeScript)
│   ├── main.tsx                  # React エントリポイント
│   ├── index.css                 # グローバルスタイル
│   ├── App.tsx                   # ルートコンポーネント
│   ├── components/               # (予定)
│   │   ├── Dashboard.tsx         # メインダッシュボード
│   │   ├── ItemCard.tsx          # アイテム表示カード
│   │   └── SourceFilter.tsx      # ソース別フィルタ
│   └── hooks/                    # (予定)
│       └── useDashboard.ts       # Tauri IPC フック
├── src-tauri/
│   ├── Cargo.toml                # Rust 依存関係
│   ├── Cargo.lock
│   ├── build.rs                  # Tauri ビルドスクリプト
│   ├── tauri.conf.json           # Tauri アプリ設定
│   ├── capabilities/
│   │   └── default.json          # セキュリティ権限設定
│   ├── icons/                    # アプリアイコン
│   │   ├── icon.png
│   │   ├── 32x32.png
│   │   ├── 128x128.png
│   │   └── 512x512.png
│   └── src/
│       ├── main.rs               # エントリポイント
│       ├── lib.rs                # Tauri ランナー・コマンド定義・setup
│       ├── config.rs             # Config.toml 読み込みと構造体定義
│       ├── store.rs              # DashStore: RwLock + RingBuffer 実装
│       ├── scheduler.rs          # Pull 定期実行スケジューラ
│       ├── server.rs             # axum Push 受信サーバー
│       ├── error.rs              # 統一エラー型定義
│       └── connectors/
│           ├── mod.rs            # Connector トレイト定義
│           ├── rss.rs            # RSS/Atom コネクタ実装
│           └── github.rs         # (予定) GitHub コネクタ
├── Config.toml.example           # 設定ファイルのサンプル (Config.toml は .gitignore 済み)
├── .gitignore
└── README.md
```

---

## 6. データフロー

### 6.1 初期化

```
アプリ起動
  └─ Config.toml 読み込み
  └─ DashStore (RingBuffer) をメモリ上に生成
  └─ Connector インスタンスを Config から構築
  └─ Pull スケジューラ起動 (Tokio spawn)
  └─ Push サーバー起動 (axum, Tokio spawn)
  └─ フロントエンドに ready イベント送信
```

### 6.2 Pull 処理（定期実行）

```
スケジューラ tick (interval: Config 値)
  └─ 全 Connector を並行実行 (tokio::join_all)
      └─ HTTP GET → フィード解析
      └─ 「今日」のアイテムのみフィルタ (chrono)
      └─ 重複チェック (HashSet<(source_id, item_id)>)
      └─ DashStore.push(item) → リングバッファへ追加
  └─ フロントエンドに dashboard_updated イベント emit
```

### 6.3 Push 処理（常時待機）

```
POST /push リクエスト受信
  └─ Bearer トークン検証 (Config.server.auth_token)
  └─ JSON デシリアライズ → DashItem
  └─ DashStore.push(item)
  └─ フロントエンドに dashboard_updated イベント emit
  └─ バックグラウンド時: OS 通知トースト表示
```

### 6.4 表示処理

```
フロントエンド (dashboard_updated イベント受信 または 手動リフレッシュ)
  └─ invoke("refresh_dashboard") → DashStore 全件 JSON 取得
  └─ React state 更新 → 再描画
```

---

## 7. コネクタ設計

### Connector トレイト

```rust
#[async_trait]
pub trait Connector: Send + Sync {
    /// ソース識別子 (Config のキーと対応)
    fn source_id(&self) -> &str;

    /// 最新アイテムを取得し、今日分のみを返す
    async fn fetch(&self) -> Result<Vec<DashItem>, ConnectorError>;
}
```

### DashItem 共通構造

```rust
pub struct DashItem {
    pub id: String,              // アイテム固有ID (重複排除用)
    pub source_id: String,       // ソース識別子
    pub source_name: String,     // 表示用ソース名
    pub title: String,
    pub body: Option<String>,    // 本文サマリ (任意)
    pub url: Option<String>,
    pub published_at: DateTime<Local>,
    pub tags: Vec<String>,       // Push 側でのカテゴリ分類等に使用
}
```

### コネクタ追加手順

1. `connectors/` 以下に新ファイルを作成
2. `Connector` トレイトを実装
3. `Config.toml` に対応するソース設定を追加
4. `main.rs` のコネクタ初期化ロジックに登録

---

## 8. Push 受信サーバー仕様

### エンドポイント

| Method | Path | 説明 |
|--------|------|------|
| `POST` | `/push` | アイテムのプッシュ受信 |
| `GET` | `/health` | サーバー疎通確認 |

### 認証

- `Authorization: Bearer <token>` ヘッダー必須
- トークンは `Config.toml` の `server.auth_token` で設定
- 不一致時は `401 Unauthorized` を返す

### リクエストボディ (POST /push)

```json
{
  "source_id": "my-ci",
  "source_name": "GitHub Actions",
  "title": "Build succeeded: main",
  "body": "All 42 tests passed in 1m 23s",
  "url": "https://github.com/org/repo/actions/runs/12345",
  "tags": ["ci", "success"]
}
```

### レスポンス

```json
{ "status": "ok", "id": "<生成されたアイテムID>" }
```

---

## 9. Windows 常駐仕様

| イベント | 動作 |
|---------|------|
| ウィンドウ「×」ボタン | `window.hide()` でバックグラウンド常駐（終了しない） |
| タスクトレイ ダブルクリック | ウィンドウを前面表示 |
| タスクトレイ 右クリック | コンテキストメニュー表示（「完全終了」「設定を開く」） |
| Push 受信 (バックグラウンド時) | OS 標準通知トースト表示 |
| スタートアップ | `Config.toml` の `windows.startup` が `true` の場合、ログイン時に自動起動 |

---

## 10. 設定ファイル仕様

**パス**: アプリと同階層の `Config.toml`（将来的には `%APPDATA%/DailyFlash/Config.toml`）

```toml
[server]
port = 8080
auth_token = "your-secret-token-here"

[memory]
# ソースごとのリングバッファ最大保持件数
default_capacity = 50
# ソース個別のオーバーライド
# [memory.overrides]
# "my-ci" = 100

[sources.rss]
poll_interval_secs = 300  # Pull 間隔 (秒)

[[sources.rss.feeds]]
id = "hacker-news"
name = "Hacker News"
url = "https://news.ycombinator.com/rss"
icon = "hn"  # 組み込みアイコン識別子 or URL

[[sources.rss.feeds]]
id = "rust-blog"
name = "Rust Blog"
url = "https://blog.rust-lang.org/feed.xml"
icon = "rust"

[windows]
startup = false          # ログイン時の自動起動
notifications = true     # バックグラウンド通知の有効化
tray_icon = true         # タスクトレイアイコン表示
start_hidden = false     # 起動時にウィンドウを非表示にする
```

---

## 11. エラーハンドリング方針

### コネクタのエラー

- **失敗しても他ソースに影響させない**: 各コネクタは独立した `tokio::spawn` で実行し、エラーはログ出力のみ
- **リトライ**: 次の Pull 周期まで待機（即時リトライは行わない）
- **UI への通知**: フロントエンドのソースカード上に「最終更新失敗」インジケータを表示

### Push サーバーのエラー

- 不正トークン → `401`
- バリデーション失敗 → `422` + エラー詳細 JSON
- DashStore 書き込み失敗 → `500`（実運用上はほぼ発生しない）

### 統一エラー型

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Config load failed: {0}")]
    Config(#[from] toml::de::Error),
    #[error("Connector error [{source_id}]: {message}")]
    Connector { source_id: String, message: String },
    #[error("Auth token mismatch")]
    Unauthorized,
}
```

---

## 12. フロントエンド設計

### イベント / コマンド一覧

| 種別 | 名前 | 方向 | 説明 |
|------|------|------|------|
| Command | `refresh_dashboard` | Front → Back | 全アイテム取得 |
| Command | `get_config` | Front → Back | 現在の設定取得 |
| Command | `clear_store` | Front → Back | 手動でストアをクリア |
| Event | `dashboard_updated` | Back → Front | 新規アイテム追加通知 |

### UI コンポーネント構成

```
<App>
  ├── <TitleBar>        ウィンドウ制御・手動リフレッシュボタン
  ├── <SourceFilter>    ソース別フィルタ（チップ UI）
  └── <Dashboard>
        └── <ItemCard>  各アイテムカード（タイトル・ソース名・時刻・URL）
```

### ポーリング vs イベント駆動

- **基本**: `dashboard_updated` イベントを `listen()` でリアルタイム受信
- **フォールバック**: TanStack Query の `refetchInterval` で 30 秒ごとに `refresh_dashboard` を呼び出し（イベント取りこぼし対策）

---

## 13. ビルド・開発

### 前提条件

- Rust (stable)
- Node.js 20+
- Tauri CLI v2 (`cargo install tauri-cli --version "^2.0.0" --locked`)

### セットアップ

```bash
npm install
```

### 開発サーバー起動

```bash
cargo tauri dev
```

Vite devサーバー (port 1420) が自動起動し、React フロントエンドを表示するウィンドウが開く。

### プロダクションビルド

```bash
cargo tauri build
```

### Push テスト (curl)

```bash
curl -X POST http://localhost:8080/push \
  -H "Authorization: Bearer your-secret-token-here" \
  -H "Content-Type: application/json" \
  -d '{
    "source_id": "test",
    "source_name": "Manual Test",
    "title": "テスト通知",
    "body": "DailyFlash の Push 受信テストです"
  }'
```

---

## 14. 今後の拡張候補

| 機能 | 概要 |
|------|------|
| **GitHub コネクタ** | 自分のリポジトリの今日のイベント（PR, Issue, CI）を Pull |
| **Slack Webhook 受信** | Slack の Outgoing Webhook を DailyFlash の Push に中継 |
| **キーワードフィルタ** | Config で設定したキーワードを含むアイテムをハイライト or 除外 |
| **アイテム詳細パネル** | クリックで本文・メタデータをサイドパネルに表示 |
| **macOS 対応** | `window.hide()` / トレイ挙動の macOS 向け実装 |
| **ホットキー** | グローバルショートカットでウィンドウの表示/非表示トグル |

---

## ライセンス

MIT
