import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { useDashboard } from "../hooks/useDashboard";
import { ItemCard } from "./ItemCard";
import { NoteInput } from "./NoteInput";
import { SourceFilter, BOOKMARK_FILTER } from "./SourceFilter";

export function Dashboard() {
  const { items, loading, error, refresh, highlightKeywords } = useDashboard();
  const [activeSource, setActiveSource] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [exportMsg, setExportMsg] = useState<string | null>(null);
  const [showNoteInput, setShowNoteInput] = useState(false);

  // Cmd+N / Ctrl+N（アプリ内）でメモ入力を開く
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "n") {
        e.preventDefault();
        setShowNoteInput(true);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  // Cmd+Shift+N グローバルショートカット（バックグラウンド時も動作）
  useEffect(() => {
    const unlisten = listen("open_note_input", () => {
      setShowNoteInput(true);
    });
    return () => { void unlisten.then((f) => f()); };
  }, []);

  // Bookmark (source_id === "bookmark") はアーカイブ扱いなので動的ソース一覧から除外
  const sources = useMemo(
    () =>
      [...new Set(
        items.filter((i) => i.source_id !== "bookmark").map((i) => i.source_name),
      )].sort(),
    [items],
  );

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return items.filter((i) => {
      if (activeSource === BOOKMARK_FILTER) {
        // Bookmark タブ: アーカイブ済み (source_id=bookmark) + 今セッションで⭐したもの
        if (i.source_id !== "bookmark" && !i.tags.includes("bookmark")) return false;
      } else {
        // すべて / ソース別: アーカイブ済みブックマーク（起動時読み込み分）のみ除外
        if (i.source_id === "bookmark") return false;
        if (activeSource && i.source_name !== activeSource) return false;
      }
      if (!q) return true;
      return (
        i.title.toLowerCase().includes(q) ||
        (i.body?.toLowerCase().includes(q) ?? false) ||
        i.tags.some((t) => t.toLowerCase().includes(q))
      );
    });
  }, [items, activeSource, query]);

  // カード削除時に即座に反映（次の refresh まで待たずに state を更新）
  const handleDelete = useCallback(
    (deletedId: string) => {
      // useDashboard の state を直接操作できないため refresh で再取得
      refresh();
      void deletedId; // lint 回避
    },
    [refresh],
  );

  // 保存ダイアログでファイルパスを選択してエクスポート
  const handleExport = useCallback(async () => {
    try {
      const now = new Date();
      const ts = now.toISOString().slice(0, 19).replace(/[-T:]/g, "").replace(/(\d{8})(\d{6})/, "$1_$2");
      const savePath = await save({
        filters: [{ name: "JSON", extensions: ["json"] }],
        defaultPath: `dailyflash_export_${ts}.json`,
      });
      if (!savePath) return; // キャンセル
      await invoke("export_items_to_path", { path: savePath });
      setExportMsg("保存しました");
      setTimeout(() => setExportMsg(null), 2000);
    } catch (e) {
      setExportMsg(`エクスポート失敗: ${e}`);
      setTimeout(() => setExportMsg(null), 3000);
    }
  }, []);

  // クリップボードに JSON をコピー
  const handleCopyJson = useCallback(async () => {
    try {
      const json = await invoke<string>("get_items_json");
      await writeText(json);
      setExportMsg("クリップボードにコピーしました");
      setTimeout(() => setExportMsg(null), 2000);
    } catch (e) {
      setExportMsg(`コピー失敗: ${e}`);
      setTimeout(() => setExportMsg(null), 3000);
    }
  }, []);

  return (
    <div className="dashboard">
      {showNoteInput && <NoteInput onClose={() => setShowNoteInput(false)} />}
      <header className="dashboard-header">
        <h1 className="dashboard-title">⚡ DailyFlash</h1>
        <div className="header-actions">
          <span className="item-count">{filtered.length} 件</span>
          <button className="btn-icon" onClick={() => setShowNoteInput(true)} title="メモを追加 (⌘N)">
            ✏️
          </button>
          <button className="btn-icon" onClick={handleCopyJson} title="JSON をクリップボードにコピー">
            📋
          </button>
          <button className="btn-icon" onClick={handleExport} title="JSON をファイルに保存">
            ↓
          </button>
          <button className="btn-icon" onClick={refresh} title="更新">
            ↻
          </button>
        </div>
      </header>

      {exportMsg && <div className="export-toast">{exportMsg}</div>}

      <div className="search-bar">
        <span className="search-icon">🔍</span>
        <input
          className="search-input"
          type="text"
          placeholder="タイトル・説明・タグで絞り込み…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        {query && (
          <button className="search-clear" onClick={() => setQuery("")} title="クリア">
            ✕
          </button>
        )}
      </div>

      {sources.length > 0 && (
        <SourceFilter
          sources={sources}
          active={activeSource}
          onChange={setActiveSource}
        />
      )}

      <main className="item-list">
        {loading && <p className="state-msg">読み込み中…</p>}
        {error && <p className="state-msg error">エラー: {error}</p>}
        {!loading && filtered.length === 0 && (
          <p className="state-msg">今日のアイテムはまだありません</p>
        )}
        {filtered.map((item) => (
          <ItemCard
            key={item.id}
            item={item}
            highlightKeywords={highlightKeywords}
            onDelete={handleDelete}
          />
        ))}
      </main>
    </div>
  );
}
