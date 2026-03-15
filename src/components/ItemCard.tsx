import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { DashItem } from "../hooks/useDashboard";

interface Props {
  item: DashItem;
  highlightKeywords?: string[];
  onDelete?: (id: string) => void;
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

export function ItemCard({ item, highlightKeywords = [], onDelete }: Props) {
  const [copied, setCopied] = useState(false);
  const [bookmarked, setBookmarked] = useState(false);
  const [toast, setToast] = useState<string | null>(null);

  const dt = new Date(item.published_at);
  const time = dt.toLocaleDateString("ja-JP", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }) + " " + dt.toLocaleTimeString("ja-JP", {
    hour: "2-digit",
    minute: "2-digit",
  });

  // カードのキーワードハイライト判定
  const kw = highlightKeywords.map((k) => k.toLowerCase());
  const isHighlighted =
    kw.length > 0 &&
    (kw.some((k) => item.title.toLowerCase().includes(k)) ||
      (item.body ? kw.some((k) => item.body!.toLowerCase().includes(k)) : false) ||
      item.tags.some((t) => kw.some((k) => t.toLowerCase().includes(k))));

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 2000);
  };

  const handleOpen = (e: React.MouseEvent) => {
    if (item.url) {
      e.preventDefault();
      openUrl(item.url);
    }
  };

  const handleCopy = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!item.url) return;
    try {
      await navigator.clipboard.writeText(item.url);
      setCopied(true);
      showToast("URLをコピーしました");
      setTimeout(() => setCopied(false), 2000);
    } catch {
      showToast("コピー失敗");
    }
  };

  const handleBookmark = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      const path = await invoke<string>("bookmark_item", { id: item.id });
      setBookmarked(true);
      showToast(`保存: ${path}`);
    } catch (err) {
      showToast(`ブックマーク失敗: ${err}`);
    }
  };

  const handleDelete = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await invoke("delete_item", { id: item.id });
      onDelete?.(item.id);
    } catch (err) {
      showToast(`削除失敗: ${err}`);
    }
  };

  return (
    <article className={`item-card${isHighlighted ? " item-card--highlighted" : ""}`}>
      {toast && <div className="item-toast">{toast}</div>}

      <div className="item-meta">
        <span className="source-name">{item.source_name}</span>
        <span className="item-time">{time}</span>
        {/* ホバー時に表示されるアクションボタン */}
        <div className="item-actions">
          {item.url && (
            <button
              className={`action-btn${copied ? " action-btn--done" : ""}`}
              onClick={handleCopy}
              title="URLをコピー"
            >
              {copied ? "✓" : "📋"}
            </button>
          )}
          <button
            className={`action-btn${bookmarked ? " action-btn--done" : ""}`}
            onClick={handleBookmark}
            title="JSONファイルにブックマーク"
          >
            {bookmarked ? "✓" : "⭐"}
          </button>
          <button
            className="action-btn action-btn--danger"
            onClick={handleDelete}
            title="削除"
          >
            🗑️
          </button>
        </div>
      </div>

      <h3 className="item-title">
        {item.url ? (
          <a href={item.url} onClick={handleOpen}>
            {highlightText(item.title, highlightKeywords)}
          </a>
        ) : (
          highlightText(item.title, highlightKeywords)
        )}
      </h3>
      {item.image_data && (
        <div className="item-image-wrap">
          <img
            src={item.image_data}
            alt="クリップボード画像"
            className="item-image"
          />
        </div>
      )}
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
