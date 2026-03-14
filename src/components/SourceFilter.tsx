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
