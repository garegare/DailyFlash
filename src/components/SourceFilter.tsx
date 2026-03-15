/** Bookmark タブを識別するための特別なフィルタ値 */
export const BOOKMARK_FILTER = "__bookmark__";

interface Props {
  sources: string[];
  active: string | null;
  onChange: (source: string | null) => void;
}

export function SourceFilter({ sources, active, onChange }: Props) {
  return (
    <div className="source-filter">
      <button
        className={`filter-chip ${active === null ? "active" : ""}`}
        onClick={() => onChange(null)}
      >
        すべて
      </button>
      {/* Bookmark は常に表示される固定タブ */}
      <button
        className={`filter-chip filter-chip--bookmark ${active === BOOKMARK_FILTER ? "active" : ""}`}
        onClick={() => onChange(BOOKMARK_FILTER)}
      >
        ⭐ Bookmark
      </button>
      {sources.map((src) => (
        <button
          key={src}
          className={`filter-chip ${active === src ? "active" : ""}`}
          onClick={() => onChange(src)}
        >
          {src}
        </button>
      ))}
    </div>
  );
}
