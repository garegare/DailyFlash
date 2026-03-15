import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";

interface Props {
  onClose: () => void;
}

export function NoteInput({ onClose }: Props) {
  const [text, setText] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  const handleSubmit = async () => {
    if (!text.trim() || submitting) return;
    setSubmitting(true);
    try {
      await invoke("add_note", { text });
      onClose();
    } catch (err) {
      console.error("メモの追加に失敗:", err);
      setSubmitting(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      onClose();
    }
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
      void handleSubmit();
    }
  };

  return (
    <div className="note-overlay" onClick={onClose}>
      <div className="note-modal" onClick={(e) => e.stopPropagation()}>
        <div className="note-modal-header">
          <span className="note-modal-title">✏️ メモを追加</span>
          <span className="note-hint">⌘+Enter で投稿 / Esc でキャンセル</span>
        </div>
        <textarea
          ref={textareaRef}
          className="note-textarea"
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="メモを入力…"
          rows={6}
        />
        <div className="note-modal-footer">
          <button className="btn-secondary" onClick={onClose}>
            キャンセル
          </button>
          <button
            className="btn-primary"
            onClick={handleSubmit}
            disabled={!text.trim() || submitting}
          >
            投稿
          </button>
        </div>
      </div>
    </div>
  );
}
