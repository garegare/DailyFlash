import { openUrl } from "@tauri-apps/plugin-opener";
import type { DashItem } from "../hooks/useDashboard";

interface Props {
  item: DashItem;
  highlightKeywords?: string[];
}

/** テキスト中のキーワードを <mark> でハイライトして ReactNode 配列に変換 */
function highlightText(text: string, keywords: string[]): React.ReactNode {
  if (keywords.length === 0) return text;

  const pattern = keywords
    .map((k) => k.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
    .join("|");
  const regex = new RegExp(`(${pattern})`, "gi");
  const parts = text.split(regex);

  return parts.map((part, i) =>
    regex.test(part) ? <mark key={i} className="kw-mark">{part}</mark> : part,
  );
}

export function ItemCard({ item, highlightKeywords = [] }: Props) {
  const dt = new Date(item.published_at);
  const time = dt.toLocaleDateString("ja-JP", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }) + " " + dt.toLocaleTimeString("ja-JP", {
    hour: "2-digit",
    minute: "2-digit",
  });

  // カード全体をハイライト対象かどうか判定（左ボーダー強調用）
  const kw = highlightKeywords.map((k) => k.toLowerCase());
  const isHighlighted =
    kw.length > 0 &&
    (kw.some((k) => item.title.toLowerCase().includes(k)) ||
      (item.body ? kw.some((k) => item.body!.toLowerCase().includes(k)) : false) ||
      item.tags.some((t) => kw.some((k) => t.toLowerCase().includes(k))));

  const handleClick = (e: React.MouseEvent) => {
    if (item.url) {
      e.preventDefault();
      openUrl(item.url);
    }
  };

  return (
    <article className={`item-card${isHighlighted ? " item-card--highlighted" : ""}`}>
      <div className="item-meta">
        <span className="source-name">{item.source_name}</span>
        <span className="item-time">{time}</span>
      </div>
      <h3 className="item-title">
        {item.url ? (
          <a href={item.url} onClick={handleClick}>
            {highlightText(item.title, highlightKeywords)}
          </a>
        ) : (
          highlightText(item.title, highlightKeywords)
        )}
      </h3>
      {item.body && (
        <p className="item-body">
          {highlightText(item.body, highlightKeywords)}
        </p>
      )}
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
