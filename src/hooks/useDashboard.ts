import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface DashItem {
  id: string;
  source_id: string;
  source_name: string;
  title: string;
  body: string | null;
  url: string | null;
  /** クリップボード画像などの base64 PNG data URL */
  image_data: string | null;
  published_at: string;
  tags: string[];
}

export function useDashboard() {
  const [items, setItems] = useState<DashItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [highlightKeywords, setHighlightKeywords] = useState<string[]>([]);

  // Config からハイライトキーワードを取得（初回のみ）
  useEffect(() => {
    invoke<{ display?: { highlight_keywords?: string[] } }>("get_config")
      .then((cfg) => {
        setHighlightKeywords(cfg.display?.highlight_keywords ?? []);
      })
      .catch(() => {});
  }, []);

  const refresh = useCallback(async () => {
    try {
      const data = await invoke<DashItem[]>("refresh_dashboard");
      // published_at 降順でソート（複数ソース混在時に確実に時系列順にする）
      data.sort(
        (a, b) =>
          new Date(b.published_at).getTime() - new Date(a.published_at).getTime(),
      );
      setItems(data);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();

    // バックエンドからの更新通知を受け取る
    const unlisten = listen("dashboard_updated", () => {
      refresh();
    });

    // 30秒フォールバックポーリング
    const interval = setInterval(refresh, 30_000);

    return () => {
      unlisten.then((f) => f());
      clearInterval(interval);
    };
  }, [refresh]);

  const clearStore = useCallback(async () => {
    await invoke("clear_store");
    setItems([]);
  }, []);

  return { items, loading, error, refresh, clearStore, highlightKeywords };
}
