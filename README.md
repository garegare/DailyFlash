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
9. [Windows 常駐仕様](#9-windows常駐仕様)
10. [設定ファイル仕様](#10-設定ファイル仕様)
11. [エラーハンドリング方針](#11-エラーハンドリング方針)
12. [フロントエンド設計](#12-フロントエンド設計)
13. [ビルド・開発](#13-ビルド開発)
14. [今後の拡張候補](#14-今後の拡張候補)

---

## 1. プロジェクト概要

DailyFlash は「情報の蓄積」を意図的に排除したダッシュボードアプリ。

- **揮発性**: アプリ終了でデータは完全消去。永続ストレージへの書き込みは行わない
- **ルックバック表示**: 当日＋直近 N 日（`lookback_days`）のアイテムを表示対象とする
- **低コンテキスト負荷**: 読み切れなかった情報は翌日には存在しない設計
- **ブラウザ連携**: アイテムカードをクリックするとシステムデフォルトブラウザで URL を開く

---

## 2. コア・コンセプト

| 概念 | 説明 |
|------|------|
| **エフェメラル** | プロセス終了 = データ消去。SQLite・ファイルキャッシュ等への永続化は行わない |
| **コネクタ方式** | `Connector` トレイトで抽象化された Pull 型ソース（RSS/Atom 等）を動的に追加可能 |
| **プッシュ受容** | 内蔵 HTTP サーバーが外部アプリ（CI、監視ツール等）からのリアルタイム通知を受信 |
| **リングバッファ** | 全ソース共通の最大保持件数を設定。容量超過時は最古アイテムを自動破棄 |
| **重複排除** | アイテムを `(source_id, item_id)` の組でハッシュ管理し、Pull 周期をまたいだ重複追加を防止 |
| **時系列混在表示** | 複数ソースのアイテムを `published_at` 降順でソートし、ソースに関係なく時系列に表示 |

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
| `tauri-plugin-opener` | システムブラウザでの URL オープン |

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
│  │  Server  │                         └──────┬──────┘  │
│  └──────────┘                                │         │
│                                              │ emit    │
│  ┌──────────────────────┐                    ↓         │
│  │  Tauri Commands      │  ←── refresh_dashboard       │
│  │  - refresh_dashboard │                              │
│  │  - get_config        │                              │
│  │  - clear_store       │                              │
│  └──────────┬───────────┘                              │
└─────────────│────────────────────────────────────────── ┘
              │ IPC (invoke / listen)
┌─────────────↓───────────────────────────────────────────┐
│  WebView (React フロントエンド)                          │
│  - published_at 降順でアイテム一覧表示                  │
│  - ソース別フィルタ（チップ UI）                        │
│  - カードクリック → システムブラウザで URL を開く       │
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
│   │   ├── ItemCard.tsx          # アイテムカード（日時・タイトル・タグ・ブラウザ連携）
│   │   └── SourceFilter.tsx      # ソース別フィルタ（チップ UI）
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

### 6.4 表示処理

```
フロントエンド (dashboard_updated イベント受信 または 30 秒フォールバック polling)
  └─ invoke("refresh_dashboard") → DashStore 全件 JSON 取得
  └─ published_at 降順でソート（Rust 側 + フロントエンド側で二重保証）
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
    pub source_id: String,       // ソース識別子
    pub source_name: String,     // 表示用ソース名
    pub title: String,
    pub body: Option<String>,    // 本文サマリ (任意)
    pub url: Option<String>,
    pub published_at: DateTime<Local>,
    pub tags: Vec<String>,
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
| Event | `dashboard_updated` | Back → Front | 新規アイテム追加通知 |

### UI コンポーネント構成

```
<App>
  └── <Dashboard>
        ├── <header>       アイテム件数・更新ボタン (↻)・クリアボタン (✕)
        ├── <SourceFilter> ソース別フィルタ（チップ UI）
        └── <ItemCard>     各アイテムカード
              ├── ソース名 / 年月日時刻
              ├── タイトル（クリックでブラウザ起動）
              ├── 本文サマリ（3 行クランプ）
              └── タグ一覧
```

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
| **Slack Webhook 受信** | Slack の Outgoing Webhook を DailyFlash の Push に中継 | 未実装 |
| **ホットキー** | グローバルショートカットでウィンドウの表示/非表示トグル | 未実装 |
| **アイテム詳細パネル** | クリックで本文・メタデータをサイドパネルに展開表示 | 未実装 |

---

## ライセンス

MIT
