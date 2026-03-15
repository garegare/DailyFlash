import { useState, useRef, useEffect } from "react";
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
  const isBookmarked = bookmarked || item.tags.includes("bookmark");
  const [toast, setToast] = useState<string | null>(null);
  const isNote = item.source_id === "note";
  const [editing, setEditing] = useState(false);
  const [editText, setEditText] = useState(item.body ?? item.title);
  const editRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (editing) {
      editRef.current?.focus();
      // カーソルを末尾に
      const len = editRef.current?.value.length ?? 0;
      editRef.current?.setSelectionRange(len, len);
    }
  }, [editing]);

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
      await invoke<string>("bookmark_item", { id: item.id });
      setBookmarked(true);
    } catch (err) {
      showToast(`ブックマーク失敗: ${err}`);
    }
  };

  const handleEditSave = async () => {
    if (!editText.trim()) return;
    try {
      await invoke("edit_note", { id: item.id, text: editText });
      setEditing(false);
    } catch (err) {
      showToast(`保存失敗: ${err}`);
    }
  };

  const handleEditKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      setEditText(item.body ?? item.title);
      setEditing(false);
    }
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      void handleEditSave();
    }
  };

  // ブックマーク解除（bookmarks.json から削除、タグを外す）
  const handleUnbookmark = async (e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await invoke("unbookmark_item", { id: item.id });
      setBookmarked(false);
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
          {isNote && !editing && (
            <button
              className="action-btn"
              onClick={(e) => { e.stopPropagation(); setEditing(true); }}
              title="編集"
            >
              ✏️
            </button>
          )}
          {!isBookmarked && (
            <button
              className="action-btn"
              onClick={handleBookmark}
              title="JSONファイルにブックマーク"
            >
              ⭐
            </button>
          )}
          {isBookmarked && (
            <button
              className="action-btn action-btn--danger"
              onClick={handleUnbookmark}
              title="ブックマークから削除"
            >
              🗑️
            </button>
          )}
        </div>
      </div>

      {editing ? (
        <div className="item-edit-area">
          <textarea
            ref={editRef}
            className="item-edit-textarea"
            value={editText}
            onChange={(e) => setEditText(e.target.value)}
            onKeyDown={handleEditKeyDown}
            rows={5}
          />
          <div className="item-edit-actions">
            <span className="note-hint">⌘+Enter で保存 / Esc でキャンセル</span>
            <div>
              <button className="btn-secondary" onClick={() => { setEditText(item.body ?? item.title); setEditing(false); }}>
                キャンセル
              </button>
              <button className="btn-primary" onClick={handleEditSave} disabled={!editText.trim()}>
                保存
              </button>
            </div>
          </div>
        </div>
      ) : (
        <>
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
        </>
      )}
      {item.tags.length > 0 && (
        <div className="item-tags">
          {item.tags.map((tag) => (
            <span
              key={tag}
              className={`tag${tag === "bookmark" ? " tag--bookmark" : ""}`}
            >
              {tag === "bookmark" ? "⭐ bookmark" : tag}
            </span>
          ))}
        </div>
      )}
    </article>
  );
}
