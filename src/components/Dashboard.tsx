import { useMemo, useState } from "react";
import { useDashboard } from "../hooks/useDashboard";
import { ItemCard } from "./ItemCard";
import { SourceFilter } from "./SourceFilter";

export function Dashboard() {
  const { items, loading, error, refresh, clearStore } = useDashboard();
  const [activeSource, setActiveSource] = useState<string | null>(null);

  const sources = useMemo(
    () => [...new Set(items.map((i) => i.source_name))].sort(),
    [items],
  );

  const filtered = useMemo(
    () =>
      activeSource
        ? items.filter((i) => i.source_name === activeSource)
        : items,
    [items, activeSource],
  );

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
