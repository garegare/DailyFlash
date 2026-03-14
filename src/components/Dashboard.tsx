import { useMemo, useState } from "react";
import { useDashboard } from "../hooks/useDashboard";
import { ItemCard } from "./ItemCard";
import { SourceFilter } from "./SourceFilter";

export function Dashboard() {
  const { items, loading, error, refresh, clearStore } = useDashboard();
  const [activeSource, setActiveSource] = useState<string | null>(null);
  const [query, setQuery] = useState("");

  const sources = useMemo(
    () => [...new Set(items.map((i) => i.source_name))].sort(),
    [items],
  );

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return items.filter((i) => {
      if (activeSource && i.source_name !== activeSource) return false;
      if (!q) return true;
      return (
        i.title.toLowerCase().includes(q) ||
        (i.body?.toLowerCase().includes(q) ?? false) ||
        i.tags.some((t) => t.toLowerCase().includes(q))
      );
    });
  }, [items, activeSource, query]);

  return (
    <div className="dashboard">
      <header className="dashboard-header">
        <h1 className="dashboard-title">⚡ DailyFlash</h1>
        <div className="header-actions">
          <span className="item-count">{filtered.length} 件</span>
          <button className="btn-icon" onClick={refresh} title="更新">
            ↻
          </button>
          <button className="btn-icon danger" onClick={clearStore} title="クリア">
            ✕
          </button>
        </div>
      </header>

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
          <ItemCard key={item.id} item={item} />
        ))}
      </main>
    </div>
  );
}
