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
8. [Push 受信サーバー仕様](#8-push受信サーバー仕様)
9. [タスクトレイ常駐仕様](#9-タスクトレイ常駐仕様)
10. [設定ファイル仕様](#10-設定ファイル仕様)
11. [エラーハンドリング方針](#11-エラーハンドリング方針)
12. [フロントエンド設計](#12-フロントエンド設計)
13. [ビルド・開発](#13-ビルド開発)
14. [今後の拡張候補](#14-今後の拡張候補)

---

## 1. プロジェクト概要

DailyFlash は「情報の蓄積」を意図的に排除したダッシュボードアプリ。

- **揮発性**: アプリ終了でデータは完全消去。ブックマークを除く永続ストレージへの書き込みは行わない
- **ルックバック表示**: 当日＋直近 N 日（`lookback_days`）のアイテムを表示対象とする
- **低コンテキスト負荷**: 読み切れなかった情報は翌日には存在しない設計
- **ブラウザ連携**: アイテムカードをクリックするとシステムデフォルトブラウザで URL を開く
- **クリップボード監視**: テキスト・画像のコピーを自動検知してダッシュボードに表示
- **ブックマーク**: 重要なアイテムを JSON ファイルに永続保存し、再起動後も閲覧可能

---

## 2. コア・コンセプト

| 概念 | 説明 |
|------|------|
| **エフェメラル** | プロセス終了 = データ消去。ブックマーク以外は SQLite・ファイルキャッシュ等への永続化は行わない |
| **コネクタ方式** | `Connector` トレイトで抽象化された Pull 型ソース（RSS/Atom 等）を動的に追加可能 |
| **プッシュ受容** | 内蔵 HTTP サーバーが外部アプリ（CI、監視ツール等）からのリアルタイム通知を受信 |
| **クリップボード監視** | OS クリップボードを定期ポーリングし、テキスト・画像を自動キャプチャ |
| **リングバッファ** | 全ソース共通の最大保持件数を設定。容量超過時は最古アイテムを自動破棄 |
| **重複排除** | アイテムを `(source_id, item_id)` の組でハッシュ管理し、Pull 周期をまたいだ重複追加を防止 |
| **時系列混在表示** | 複数ソースのアイテムを `published_at` 降順でソートし、ソースに関係なく時系列に表示 |
| **ブックマーク** | ⭐ボタンで `bookmarks.json` に永続保存。起動時に自動読み込みされ Bookmark タブで閲覧可能 |

---

## 3. 技術スタック

### バックエンド (Rust)

| ライブラリ | 用途 |
|-----------|------|
| `tauri` v2 | デスクトップアプリ基盤・IPC |
| `tokio` | 非同期ランタイム |
| `axum` | Push 受信用ローカル HTTP サーバー |
| `reqwest` | RSS 取得用 HTTP クライアント |
| `feed-rs` | RSS/Atom フィード解析 |
| `chrono` | 日付フィルタリング |
| `serde` / `toml` | 設定ファイル管理 |
| `async-trait` | 非同期トレイト実装 |
| `thiserror` | 統一エラー型定義 |
| `arboard` | クリップボードアクセス（テキスト・画像） |
| `image` | クリップボード画像の PNG エンコード |
| `base64` | 画像データの Base64 変換（data URL 形式） |
| `dirs-next` | ホームディレクトリ取得（`~` 展開） |
| `tauri-plugin-opener` | システムブラウザでの URL オープン |
| `tauri-plugin-clipboard-manager` | Tauri クリップボードプラグイン |

### フロントエンド (TypeScript)

| ライブラリ | 用途 |
|-----------|------|
| `React` | UI フレームワーク |
| `@tauri-apps/api` | Tauri IPC バインディング（invoke / listen） |
| `@tauri-apps/plugin-opener` | ブラウザで URL を開く |

---

## 4. アーキテクチャ

```
┌─────────────────────────────────────────────────────────┐
│  Tauri App (Rust プロセス)                               │
│                                                         │
│  ┌──────────┐   Pull (起動直後 + 定期)  ┌─────────────┐  │
│  │ Connector │ ──────────────────────→ │             │  │
│  │  (RSS等)  │                         │  DashStore  │  │
│  └──────────┘                         │ (Arc<RwLock │  │
│                                       │  <RingBuf>>) │  │
│  ┌──────────┐   Push (即時)           │             │  │
│  │  axum    │ ──────────────────────→ │             │  │
│  │  Server  │                         │             │  │
│  └──────────┘                         │             │  │
│                                       │             │  │
│  ┌──────────┐   定期ポーリング        │             │  │
│  │Clipboard │ ──────────────────────→ │             │  │
│  │ Monitor  │  テキスト/画像キャプチャ │             │  │
│  └──────────┘                         └──────┬──────┘  │
│                                              │         │
│                                              │ emit    │
│  ┌──────────────────────────────┐            ↓         │
│  │  Tauri Commands              │  ←── dashboard_updated│
│  │  - refresh_dashboard         │                      │
│  │  - get_config                │                      │
│  │  - clear_store               │                      │
│  │  - bookmark_item             │                      │
│  │  - unbookmark_item           │                      │
│  │  - export_items              │                      │
│  └──────────┬───────────────────┘                      │
│             │              bookmarks.json (永続)        │
└─────────────│────────────────────────────────────────── ┘
              │ IPC (invoke / listen)
┌─────────────↓───────────────────────────────────────────┐
│  WebView (React フロントエンド)                          │
│  - published_at 降順でアイテム一覧表示                  │
│  - ソース別フィルタ / Bookmark タブ（チップ UI）        │
│  - カードクリック → システムブラウザで URL を開く       │
│  - ⭐ボタンでブックマーク / 🗑️ボタンで解除            │
│  - dashboard_updated イベントでリアルタイム更新         │
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
│   ├── components/
│   │   ├── Dashboard.tsx         # メインダッシュボード（フィルタ・一覧）
│   │   ├── ItemCard.tsx          # アイテムカード（日時・タイトル・タグ・ブックマーク）
│   │   └── SourceFilter.tsx      # ソース別フィルタ（Bookmark タブ含む）
│   └── hooks/
│       └── useDashboard.ts       # Tauri IPC フック（invoke / listen / polling）
├── src-tauri/
│   ├── Cargo.toml                # Rust 依存関係
│   ├── Cargo.lock
│   ├── build.rs                  # Tauri ビルドスクリプト
│   ├── tauri.conf.json           # Tauri アプリ設定
│   ├── capabilities/
│   │   └── default.json          # セキュリティ権限設定（opener 含む）
│   ├── icons/                    # アプリアイコン
│   └── src/
│       ├── main.rs               # エントリポイント
│       ├── lib.rs                # Tauri ランナー・コマンド定義・setup
│       ├── config.rs             # Config.toml 読み込みと構造体定義
│       ├── store.rs              # DashStore: RwLock + RingBuffer 実装
│       ├── scheduler.rs          # Pull スケジューラ（起動直後に即時実行）
│       ├── server.rs             # axum Push 受信サーバー
│       ├── clipboard_monitor.rs  # クリップボード監視（テキスト・画像対応）
│       ├── error.rs              # 統一エラー型定義
│       └── connectors/
│           ├── mod.rs            # Connector トレイト定義
│           ├── rss.rs            # RSS/Atom コネクタ実装（lookback_days フィルタ）
│           └── github.rs         # GitHub Events コネクタ実装
├── Config.toml                   # ローカル設定ファイル (.gitignore 済み)
├── Config.toml.example           # 設定ファイルのサンプル
├── .gitignore
└── README.md
```

---

## 6. データフロー

### 6.1 初期化

```
アプリ起動
  └─ Config.toml 読み込み（複数パス候補を順に検索）
  └─ DashStore (RingBuffer) をメモリ上に生成
  └─ Connector インスタンスを Config から構築
  └─ Pull スケジューラ起動
      └─ 起動直後に即時 fetch 実行
      └─ 以降 poll_interval_secs ごとに fetch
  └─ Push サーバー起動 (axum, 127.0.0.1:8080)
  └─ bookmarks.json 読み込み → ストアへ追加（source_id = "bookmark"）
  └─ クリップボード監視起動（poll_interval_secs 間隔）
```

### 6.2 Pull 処理（定期実行）

```
スケジューラ tick (poll_interval_secs)
  └─ 全 Connector を並行実行
      └─ HTTP GET → フィード解析 (feed-rs)
      └─ published_at が cutoff 以降のアイテムのみ通過
         cutoff = today - lookback_days
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
```

### 6.4 クリップボード監視

```
クリップボード監視タスク（OS スレッドで実行、macOS NSPasteboard 制約対応）
  └─ poll_interval_secs ごとにクリップボード内容を取得
      └─ テキスト変化を検知 → DashItem として push
      └─ テキストなし → 画像を確認
          └─ 画像変化を検知 → RGBA → PNG → Base64 data URL に変換
          └─ DashItem (image_data フィールド) として push
          └─ タイトルで重複排除（画像サイズが同じなら同一視）
  └─ max_items 超過時は古いクリップボードアイテムを削除
  └─ フロントエンドに dashboard_updated イベント emit
```

### 6.5 ブックマーク処理

```
⭐ボタンクリック → bookmark_item コマンド
  └─ ストアからアイテムを取得
  └─ "bookmark" タグを付与
  └─ bookmarks.json に追記（同 ID は上書き）
  └─ ストアのアイテムをタグ付き版に更新
  └─ dashboard_updated イベント emit

起動時
  └─ bookmarks.json 読み込み
  └─ source_id = "bookmark" として DashStore に追加
  └─ Bookmark タブにのみ表示（「すべて」には非表示）

🗑️ボタンクリック（ブックマーク済みアイテムのみ表示）→ unbookmark_item コマンド
  └─ bookmarks.json から該当アイテムを削除
  └─ source_id = "bookmark" のアイテム → ストアから削除
  └─ 通常ソースのアイテム → bookmark タグを外してストアに戻す
  └─ dashboard_updated イベント emit
```

### 6.6 表示処理

```
フロントエンド (dashboard_updated イベント受信 または 30 秒フォールバック polling)
  └─ invoke("refresh_dashboard") → DashStore 全件 JSON 取得
  └─ published_at 降順でソート
  └─ React state 更新 → 再描画
  └─ カードクリック → openUrl() でシステムブラウザを起動
```

---

## 7. コネクタ設計

### Connector トレイト

```rust
#[async_trait]
pub trait Connector: Send + Sync {
    /// ソース識別子 (Config のキーと対応)
    fn source_id(&self) -> &str;

    /// 最新アイテムを取得し、lookback_days 以内のもののみを返す
    async fn fetch(&self) -> Result<Vec<DashItem>, ConnectorError>;
}
```

### DashItem 共通構造

```rust
pub struct DashItem {
    pub id: String,              // アイテム固有ID (重複排除用)
    pub source_id: String,       // ソース識別子（"bookmark" は特別扱い）
    pub source_name: String,     // 表示用ソース名
    pub title: String,
    pub body: Option<String>,    // 本文サマリ (任意)
    pub url: Option<String>,
    pub image_data: Option<String>, // Base64 data URL（クリップボード画像等）
    pub published_at: DateTime<Local>,
    pub tags: Vec<String>,       // "bookmark" タグで永続保存済みを識別
}
```

### コネクタ追加手順

1. `connectors/` 以下に新ファイルを作成
2. `Connector` トレイトを実装
3. `Config.toml` に対応するソース設定を追加
4. `lib.rs` のコネクタ初期化ロジックに登録

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

## 9. タスクトレイ常駐仕様

| イベント | 動作 |
|---------|------|
| ウィンドウ「×」ボタン | `window.hide()` でバックグラウンド常駐（終了しない） |
| タスクトレイ 左クリック | ウィンドウの表示/非表示をトグル |
| タスクトレイ 右クリック | コンテキストメニュー表示（「ウィンドウを表示」「終了」） |
| Push 受信 (バックグラウンド時) | OS 標準通知トースト表示 |
| スタートアップ | `Config.toml` の `windows.startup` が `true` の場合、ログイン時に自動起動 |

---

## 10. 設定ファイル仕様

**パス検索順**: `./Config.toml` → `../Config.toml` → 実行バイナリ同階層 → その親ディレクトリ

```toml
[server]
port = 8080
auth_token = "your-secret-token-here"

[memory]
# 全ソース共通のリングバッファ最大保持件数
default_capacity = 50

# ストレージ設定
# [storage]
# bookmarks_path = "~/Documents/dailyflash_bookmarks.json"
# 未設定時は ~/Library/Application Support/com.dailyflash.app/bookmarks.json

# ハイライト表示するキーワード（タイトル・説明文・タグを検索、大文字小文字無視）
[display]
highlight_keywords = ["Claude", "Rust"]

[sources.rss]
poll_interval_secs = 300  # Pull 間隔 (秒)
lookback_days = 3         # 当日 + 直近 N 日分を表示対象とする

[[sources.rss.feeds]]
id = "zenn"
name = "Zenn"
url = "https://zenn.dev/feed"
# lookback_days = 7       # フィード個別に上書き可能（省略時はソースレベル値を使用）

[[sources.rss.feeds]]
id = "qiita"
name = "Qiita"
url = "https://qiita.com/popular-items/feed.atom"

# クリップボード監視（デフォルト有効）
# [sources.clipboard]
# enabled = true
# poll_interval_secs = 3   # ポーリング間隔
# min_chars = 4            # テキスト検知の最小文字数
# max_items = 10           # ダッシュボードに保持するクリップボードカードの最大枚数

# GitHub コネクタ（Personal Access Token が必要）
# [sources.github]
# token = "ghp_xxxxxxxxxxxx"
# username = "your-github-username"
# poll_interval_secs = 300
# lookback_days = 3

[windows]
startup = false          # ログイン時の自動起動
notifications = true     # バックグラウンド通知の有効化
tray_icon = true         # タスクトレイアイコン表示
start_hidden = false     # 起動時にウィンドウを非表示にする
```

### lookback_days について

デフォルト値: `3`

当日分のみを対象とすると、週末や祝日など記事が少ない日にダッシュボードが空になる。`lookback_days` を設定することで「今日 + 直近 N 日」のアイテムを表示し、常に情報が表示される状態を維持する。

---

## 11. エラーハンドリング方針

### コネクタのエラー

- **失敗しても他ソースに影響させない**: 各コネクタは独立した非同期タスクで実行し、エラーはログ出力のみ
- **リトライ**: 次の Pull 周期まで待機（即時リトライは行わない）

### Push サーバーのエラー

- 不正トークン → `401`
- バリデーション失敗 → `422` + エラー詳細 JSON
- DashStore 書き込み失敗 → `500`

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
    #[error("Validation error: {0}")]
    Validation(String),
}
```

---

## 12. フロントエンド設計

### イベント / コマンド一覧

| 種別 | 名前 | 方向 | 説明 |
|------|------|------|------|
| Command | `refresh_dashboard` | Front → Back | 全アイテム取得（published_at 降順） |
| Command | `get_config` | Front → Back | 現在の設定取得 |
| Command | `clear_store` | Front → Back | 手動でストアをクリア |
| Command | `delete_item` | Front → Back | 指定アイテムをストアから削除 |
| Command | `bookmark_item` | Front → Back | アイテムを bookmarks.json に保存し "bookmark" タグを付与 |
| Command | `unbookmark_item` | Front → Back | bookmarks.json から削除し bookmark タグを解除 |
| Command | `export_items` | Front → Back | 全アイテムを JSON ファイルにエクスポート |
| Event | `dashboard_updated` | Back → Front | 新規アイテム追加・変更通知 |

### UI コンポーネント構成

```
<App>
  └── <Dashboard>
        ├── <header>       アイテム件数・エクスポート(↓)・更新(↻)・クリア(✕)ボタン
        ├── <search-bar>   タイトル・本文・タグでのテキスト絞り込み
        ├── <SourceFilter> ソース別フィルタ（すべて / ⭐Bookmark / 各ソース）
        └── <ItemCard>     各アイテムカード
              ├── ソース名 / 年月日時刻
              ├── アクションボタン（ホバーで表示）
              │     ├── 📋 URLコピー（URL がある場合）
              │     ├── ⭐ ブックマーク（未ブックマーク時のみ）
              │     └── 🗑️ ブックマーク解除（ブックマーク済み時のみ）
              ├── タイトル（クリックでブラウザ起動）
              ├── 画像（クリップボード画像の場合）
              ├── 本文サマリ（3 行クランプ）
              └── タグ一覧（"bookmark" タグは ⭐ 付きで強調表示）
```

### ブックマークのフィルタ動作

| タブ | 表示対象 |
|------|---------|
| **すべて** | 通常アイテム + ブックマークタグ付きアイテム（起動時読み込み分は除外） |
| **⭐ Bookmark** | 起動時に bookmarks.json から読み込んだアーカイブ + 当セッションでブックマークしたアイテム |
| **ソース別** | 各ソースの通常アイテム + そのソースのブックマークタグ付きアイテム |

### useDashboard フック

- 初回マウント時に `refresh_dashboard` を呼び出し
- `dashboard_updated` イベントを `listen()` でリアルタイム受信
- 30 秒フォールバック polling（イベント取りこぼし対策）
- フロントエンド側でも `published_at` 降順ソートを適用

---

## 13. ビルド・開発

### 前提条件

- Rust (stable)
- Node.js 20+
- Tauri CLI v2 (`cargo install tauri-cli --version "^2.0.0" --locked`)

### セットアップ

```bash
git clone <repo>
cd DailyFlash
npm install

# Config.toml を作成（サンプルをコピーして編集）
cp Config.toml.example Config.toml
```

### 開発サーバー起動

```bash
cargo tauri dev
# または
npm run tauri
```

Vite dev サーバー (port 1420) が自動起動し、React フロントエンドを表示するウィンドウが開く。

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
    "body": "DailyFlash の Push 受信テストです",
    "url": "https://example.com",
    "tags": ["test"]
  }'
```

---

## 14. 今後の拡張候補

| 機能 | 概要 | 状態 |
|------|------|------|
| **GitHub コネクタ** | ユーザーの今日のイベント（Push, PR, Issue, Star 等）を Pull | ✅ 実装済み |
| **キーワードハイライト** | Config で設定したキーワードを含むアイテムをカード・テキストレベルでハイライト | ✅ 実装済み |
| **タスクトレイ常駐** | ウィンドウ「×」で非表示、トレイ左クリックでトグル、右クリックメニューで終了 | ✅ 実装済み |
| **フィード個別 lookback_days** | ソースごとに異なるルックバック日数を `lookback_days` で設定可能 | ✅ 実装済み |
| **クリップボード監視** | テキスト・画像のコピーを自動検知してダッシュボードに表示 | ✅ 実装済み |
| **ブックマーク機能** | ⭐ボタンで JSON ファイルに永続保存、Bookmark タブで閲覧・解除可能 | ✅ 実装済み |
| **JSON エクスポート** | 現在のダッシュボードアイテムを JSON ファイルに書き出す | ✅ 実装済み |
| **Slack Webhook 受信** | Slack の Outgoing Webhook を DailyFlash の Push に中継 | 未実装 |
| **ホットキー** | グローバルショートカットでウィンドウの表示/非表示トグル | 未実装 |
| **アイテム詳細パネル** | クリックで本文・メタデータをサイドパネルに展開表示 | 未実装 |

---

## ライセンス

MIT
