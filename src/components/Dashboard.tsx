import { useCallback, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useDashboard } from "../hooks/useDashboard";
import { ItemCard } from "./ItemCard";
import { SourceFilter, BOOKMARK_FILTER } from "./SourceFilter";

export function Dashboard() {
  const { items, loading, error, refresh, clearStore, highlightKeywords } = useDashboard();
  const [activeSource, setActiveSource] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [exportMsg, setExportMsg] = useState<string | null>(null);

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
        // すべて / ソース別: アーカイブブックマークは除外
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

  const handleExport = useCallback(async () => {
    try {
      const path = await invoke<string>("export_items");
      setExportMsg(`保存: ${path}`);
      setTimeout(() => setExportMsg(null), 3000);
    } catch (e) {
      setExportMsg(`エクスポート失敗: ${e}`);
      setTimeout(() => setExportMsg(null), 3000);
    }
  }, []);

  return (
    <div className="dashboard">
      <header className="dashboard-header">
        <h1 className="dashboard-title">⚡ DailyFlash</h1>
        <div className="header-actions">
          <span className="item-count">{filtered.length} 件</span>
          <button className="btn-icon" onClick={handleExport} title="JSON エクスポート">
            ↓
          </button>
          <button className="btn-icon" onClick={refresh} title="更新">
            ↻
          </button>
          <button className="btn-icon danger" onClick={clearStore} title="クリア">
            ✕
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
