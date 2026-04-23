import { useCallback, useEffect, useState } from "react";

export interface TryItHistoryEntry {
  at: number;
  status: number;
  elapsedMs: number;
  method: string;
  url: string;
}

const STORAGE_PREFIX = "albert.tryit.history.";
const STORAGE_VERSION = 1;
const MAX_ENTRIES = 5;

function key(routeKey: string): string {
  return `${STORAGE_PREFIX}${STORAGE_VERSION}:${routeKey}`;
}

function load(routeKey: string): TryItHistoryEntry[] {
  try {
    const raw = window.localStorage.getItem(key(routeKey));
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((entry): entry is TryItHistoryEntry =>
        typeof entry === "object" &&
        entry !== null &&
        typeof (entry as TryItHistoryEntry).at === "number" &&
        typeof (entry as TryItHistoryEntry).status === "number" &&
        typeof (entry as TryItHistoryEntry).elapsedMs === "number" &&
        typeof (entry as TryItHistoryEntry).method === "string" &&
        typeof (entry as TryItHistoryEntry).url === "string"
      )
      .slice(0, MAX_ENTRIES);
  } catch {
    return [];
  }
}

function save(routeKey: string, entries: TryItHistoryEntry[]): void {
  try {
    window.localStorage.setItem(
      key(routeKey),
      JSON.stringify(entries.slice(0, MAX_ENTRIES))
    );
  } catch {
    /* quota or serialization — silently drop */
  }
}

/**
 * A bounded last-N history of Try-it responses keyed by `METHOD /path`.
 * Each call to `record` inserts at the front and truncates to MAX_ENTRIES.
 * Storage is `localStorage` so the history survives across sessions.
 */
export function useTryItHistory(routeKey: string): {
  history: TryItHistoryEntry[];
  record: (entry: Omit<TryItHistoryEntry, "at">) => void;
  clear: () => void;
} {
  const [history, setHistory] = useState<TryItHistoryEntry[]>(() =>
    load(routeKey)
  );

  useEffect(() => {
    setHistory(load(routeKey));
  }, [routeKey]);

  const record = useCallback(
    (entry: Omit<TryItHistoryEntry, "at">) => {
      setHistory((prev) => {
        const next = [{ ...entry, at: Date.now() }, ...prev].slice(
          0,
          MAX_ENTRIES
        );
        save(routeKey, next);
        return next;
      });
    },
    [routeKey]
  );

  const clear = useCallback(() => {
    setHistory([]);
    try {
      window.localStorage.removeItem(key(routeKey));
    } catch {
      /* ignore */
    }
  }, [routeKey]);

  return { history, record, clear };
}
