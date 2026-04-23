import { useCallback, useEffect, useState } from "react";

export interface TryItHeader {
  key: string;
  value: string;
}

export interface TryItDraft {
  params: Record<string, string>;
  query: string;
  body: string;
  headers: TryItHeader[];
}

const EMPTY: TryItDraft = {
  params: {},
  query: "",
  body: "",
  headers: []
};

const STORAGE_PREFIX = "albert.tryit.";
const STORAGE_SCHEMA_VERSION = 1;

function storageKey(routeKey: string): string {
  return `${STORAGE_PREFIX}${STORAGE_SCHEMA_VERSION}:${routeKey}`;
}

function load(routeKey: string): TryItDraft {
  try {
    const raw = window.localStorage.getItem(storageKey(routeKey));
    if (!raw) return EMPTY;
    const parsed = JSON.parse(raw) as Partial<TryItDraft>;
    return {
      params: parsed.params ?? {},
      query: parsed.query ?? "",
      body: parsed.body ?? "",
      headers: Array.isArray(parsed.headers)
        ? parsed.headers.map((h) => ({
            key: h.key ?? "",
            value: h.value ?? ""
          }))
        : []
    };
  } catch {
    return EMPTY;
  }
}

function save(routeKey: string, draft: TryItDraft) {
  try {
    window.localStorage.setItem(storageKey(routeKey), JSON.stringify(draft));
  } catch {
    /* quota or serialization — ignore */
  }
}

const SEED_EVENT = "albert:tryit:seed";

export function seedTryItDraft(routeKey: string, draft: Partial<TryItDraft>) {
  try {
    const existing = load(routeKey);
    const merged: TryItDraft = {
      params: draft.params ?? existing.params,
      query: draft.query ?? existing.query,
      body: draft.body ?? existing.body,
      headers: draft.headers ?? existing.headers
    };
    save(routeKey, merged);
    // Dispatch an event so any live TryItPanel mounted on this route
    // picks up the change without a remount round-trip.
    window.dispatchEvent(
      new CustomEvent(SEED_EVENT, { detail: { routeKey, draft: merged } })
    );
  } catch {
    /* ignore */
  }
}

export function useTryItDraft(routeKey: string): {
  draft: TryItDraft;
  updateParams: (params: Record<string, string>) => void;
  updateQuery: (value: string) => void;
  updateBody: (value: string) => void;
  updateHeaders: (headers: TryItHeader[]) => void;
  reset: () => void;
} {
  const [draft, setDraft] = useState<TryItDraft>(() => load(routeKey));

  useEffect(() => {
    setDraft(load(routeKey));
  }, [routeKey]);

  useEffect(() => {
    save(routeKey, draft);
  }, [routeKey, draft]);

  useEffect(() => {
    function onSeed(event: Event) {
      const detail = (event as CustomEvent<{
        routeKey: string;
        draft: TryItDraft;
      }>).detail;
      if (detail.routeKey === routeKey) {
        setDraft(detail.draft);
      }
    }
    window.addEventListener(SEED_EVENT, onSeed as EventListener);
    return () => {
      window.removeEventListener(SEED_EVENT, onSeed as EventListener);
    };
  }, [routeKey]);

  const updateParams = useCallback(
    (params: Record<string, string>) => {
      setDraft((prev) => ({ ...prev, params }));
    },
    []
  );
  const updateQuery = useCallback((value: string) => {
    setDraft((prev) => ({ ...prev, query: value }));
  }, []);
  const updateBody = useCallback((value: string) => {
    setDraft((prev) => ({ ...prev, body: value }));
  }, []);
  const updateHeaders = useCallback((headers: TryItHeader[]) => {
    setDraft((prev) => ({ ...prev, headers }));
  }, []);
  const reset = useCallback(() => {
    setDraft(EMPTY);
    try {
      window.localStorage.removeItem(storageKey(routeKey));
    } catch {
      /* ignore */
    }
  }, [routeKey]);

  return { draft, updateParams, updateQuery, updateBody, updateHeaders, reset };
}
