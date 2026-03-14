import type { DashItem } from "../hooks/useDashboard";

interface Props {
  item: DashItem;
}

export function ItemCard({ item }: Props) {
  const time = new Date(item.published_at).toLocaleTimeString("ja-JP", {
    hour: "2-digit",
    minute: "2-digit",
  });

  return (
    <article className="item-card">
      <div className="item-meta">
        <span className="source-name">{item.source_name}</span>
        <span className="item-time">{time}</span>
      </div>
      <h3 className="item-title">
        {item.url ? (
          <a href={item.url} target="_blank" rel="noopener noreferrer">
            {item.title}
          </a>
        ) : (
          item.title
        )}
      </h3>
      {item.body && <p className="item-body">{item.body}</p>}
      {item.tags.length > 0 && (
        <div className="item-tags">
          {item.tags.map((tag) => (
            <span key={tag} className="tag">
              {tag}
            </span>
          ))}
        </div>
      )}
    </article>
  );
}
