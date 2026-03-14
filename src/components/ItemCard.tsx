import { openUrl } from "@tauri-apps/plugin-opener";
import type { DashItem } from "../hooks/useDashboard";

interface Props {
  item: DashItem;
}

export function ItemCard({ item }: Props) {
  const dt = new Date(item.published_at);
  const time = dt.toLocaleDateString("ja-JP", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }) + " " + dt.toLocaleTimeString("ja-JP", {
    hour: "2-digit",
    minute: "2-digit",
  });

  const handleClick = (e: React.MouseEvent) => {
    if (item.url) {
      e.preventDefault();
      openUrl(item.url);
    }
  };

  return (
    <article className="item-card">
      <div className="item-meta">
        <span className="source-name">{item.source_name}</span>
        <span className="item-time">{time}</span>
      </div>
      <h3 className="item-title">
        {item.url ? (
          <a href={item.url} onClick={handleClick}>
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
