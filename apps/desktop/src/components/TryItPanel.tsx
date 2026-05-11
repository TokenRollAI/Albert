import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Icon } from "./Icon";
import { JsonView } from "./JsonView";
import { lintJson } from "../lib/jsonLint";
import {
  parseQueryString,
  serializeQueryString,
  type QueryPair
} from "../lib/queryString";
import {
  pickResponseSchema,
  validateAgainstSchema
} from "../lib/schemaValidation";
import { useTryItDraft } from "../hooks/useTryItDraft";
import { useTryItHistory } from "../hooks/useTryItHistory";
import type {
  AuthRequirementHint,
  EndpointTab,
  ExampleKind,
  GenerationContext,
  MockExample,
  RequestCacheEntry
} from "../types";

/**
 * Default value template for a given auth hint. Used to pre-seed the
 * Authorization (or custom) header row so users aren't retyping the
 * scheme prefix every time. Keeping this outside the component so a
 * unit test can pin the shapes.
 */
export function placeholderForAuthHint(
  hint: AuthRequirementHint
): string | null {
  switch (hint.scheme) {
    case "http_bearer":
    case "oauth2":
      return "Bearer ";
    case "http_basic":
      return "Basic ";
    case "api_key_header":
      return "";
    default:
      return null;
  }
}

interface TryItPanelProps {
  tab: EndpointTab;
  baseUrl: string | null;
  connected?: boolean;
  onSaveResponseAsExample?: (
    tab: EndpointTab,
    example: MockExample
  ) => Promise<MockExample | null>;
  onGenerateFromCache?: (
    tab: EndpointTab,
    kind: ExampleKind,
    context: GenerationContext
  ) => Promise<MockExample | null>;
  onPreviewPromptFromCache?: (
    tab: EndpointTab,
    kind: ExampleKind,
    context: GenerationContext
  ) => Promise<unknown>;
  canGenerateFromCache?: boolean;
  requestCacheRoutingEnabled?: boolean;
  onReloadRequestCache?: () => Promise<void>;
}

interface ResponseState {
  status: number;
  headers: Record<string, string>;
  body: unknown;
  elapsedMs: number;
  sizeBytes: number;
  url: string;
  requestSnapshot: RequestSnapshot;
}

interface RequestSnapshot {
  query: string;
  headers: Record<string, string>;
  body: unknown;
}

interface ResponseSnapshot {
  status: number;
  headers: Record<string, string>;
  body: unknown;
  elapsed_ms: number;
  size_bytes: number;
}

/** Format a byte count as a human-readable label (e.g. "1.2 kB"). */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "0 B";
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${kb.toFixed(kb >= 10 ? 0 : 1)} kB`;
  const mb = kb / 1024;
  return `${mb.toFixed(mb >= 10 ? 0 : 1)} MB`;
}

function extractPathParams(path: string): string[] {
  const regex = /\{(\w+)\}/g;
  const params: string[] = [];
  let match: RegExpExecArray | null;
  while ((match = regex.exec(path)) !== null) {
    params.push(match[1]);
  }
  return params;
}

function buildUrl(
  base: string,
  path: string,
  params: Record<string, string>,
  query: string
): string {
  let resolved = path;
  for (const [key, value] of Object.entries(params)) {
    resolved = resolved.replace(`{${key}}`, encodeURIComponent(value || ""));
  }
  const trimmedQuery = query.trim().replace(/^\?/, "");
  const suffix = trimmedQuery ? `?${trimmedQuery}` : "";
  return `${base.replace(/\/$/, "")}${resolved}${suffix}`;
}

const KIND_LABEL: Record<ExampleKind, string> = {
  success: "Success",
  empty: "Empty",
  error: "Error"
};

function kindFromStatus(status: number): ExampleKind {
  if (status >= 400) return "error";
  if (status === 204) return "empty";
  return "success";
}

function canPersistBody(body: unknown): boolean {
  return body !== "<invalid JSON>";
}

function isPersistedCollection(collectionId: string): boolean {
  return Boolean(collectionId) &&
    !collectionId.startsWith("preview:") &&
    !collectionId.startsWith("fallback:");
}

function snapshotBody(method: string, bodyDraft: string): unknown {
  if (method === "GET" || method === "HEAD" || !bodyDraft.trim()) {
    return null;
  }
  try {
    return JSON.parse(bodyDraft);
  } catch {
    return bodyDraft;
  }
}

function displayCacheTime(epochSeconds: string): string {
  const seconds = Number(epochSeconds);
  if (!Number.isFinite(seconds) || seconds <= 0) return "unknown time";
  return new Date(seconds * 1000).toLocaleTimeString(undefined, {
    hour12: false
  });
}

interface ValidateMockPayloadResult {
  valid: boolean;
  errors: string[];
}

const CACHE_STALE_AFTER_MS = 24 * 60 * 60 * 1000;

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function cacheResponseSnapshot(entry: RequestCacheEntry): ResponseSnapshot | null {
  if (!isRecord(entry.response_snapshot)) return null;
  const status = entry.response_snapshot.status;
  if (typeof status !== "number") return null;
  const headers = isRecord(entry.response_snapshot.headers)
    ? Object.fromEntries(
        Object.entries(entry.response_snapshot.headers).map(([key, value]) => [
          key,
          String(value)
        ])
      )
    : {};
  const elapsedMs = entry.response_snapshot.elapsed_ms;
  const sizeBytes = entry.response_snapshot.size_bytes;
  return {
    status,
    headers,
    body: entry.response_snapshot.body,
    elapsed_ms: typeof elapsedMs === "number" ? elapsedMs : 0,
    size_bytes: typeof sizeBytes === "number" ? sizeBytes : 0
  };
}

function kindFromCacheEntry(entry: RequestCacheEntry): ExampleKind {
  const snapshot = cacheResponseSnapshot(entry);
  return snapshot ? kindFromStatus(snapshot.status) : "success";
}

function cacheAgeMs(entry: RequestCacheEntry): number | null {
  const seconds = Number(entry.last_seen_at);
  if (!Number.isFinite(seconds) || seconds <= 0) return null;
  return Math.max(0, Date.now() - seconds * 1000);
}

function cacheAgeLabel(entry: RequestCacheEntry): string {
  const ageMs = cacheAgeMs(entry);
  if (ageMs === null) return "unknown age";
  const minutes = Math.floor(ageMs / 60000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m old`;
  const hours = Math.floor(minutes / 60);
  if (hours < 48) return `${hours}h old`;
  return `${Math.floor(hours / 24)}d old`;
}

function isCacheEntryStale(entry: RequestCacheEntry): boolean {
  const ageMs = cacheAgeMs(entry);
  return ageMs !== null && ageMs > CACHE_STALE_AFTER_MS;
}

function cacheRequestSnapshot(entry: RequestCacheEntry): RequestSnapshot | null {
  if (!isRecord(entry.request_snapshot)) return null;
  const query =
    typeof entry.request_snapshot.query === "string"
      ? entry.request_snapshot.query
      : "";
  const headers = isRecord(entry.request_snapshot.headers)
    ? Object.fromEntries(
        Object.entries(entry.request_snapshot.headers).map(([key, value]) => [
          key,
          String(value)
        ])
      )
    : {};
  return {
    query,
    headers,
    body: entry.request_snapshot.body ?? null
  };
}

function bodyDraftFromSnapshot(body: unknown): string {
  if (body === null || body === undefined) return "";
  if (typeof body === "string") return body;
  return JSON.stringify(body, null, 2);
}

export function TryItPanel({
  tab,
  baseUrl,
  connected = false,
  onSaveResponseAsExample,
  onGenerateFromCache,
  onPreviewPromptFromCache,
  canGenerateFromCache = true,
  requestCacheRoutingEnabled = false,
  onReloadRequestCache
}: TryItPanelProps) {
  const pathParams = useMemo(
    () => extractPathParams(tab.endpoint.path),
    [tab.endpoint.path]
  );
  const routeKey = `${tab.method.toUpperCase()} ${tab.endpoint.path}`;
  const {
    draft,
    updateParams,
    updateQuery,
    updateBody,
    updateHeaders,
    reset
  } = useTryItDraft(routeKey);
  const history = useTryItHistory(routeKey);
  const params = draft.params;
  const query = draft.query;
  const bodyDraft = draft.body;
  const headers = draft.headers;
  const setParams = (next: Record<string, string>) => updateParams(next);
  const setQuery = (value: string) => updateQuery(value);
  const setBodyDraft = (value: string) => updateBody(value);
  const setHeaders = (next: Array<{ key: string; value: string }>) =>
    updateHeaders(next);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [response, setResponse] = useState<ResponseState | null>(null);
  const [savingResponse, setSavingResponse] = useState(false);
  const [saveResponseError, setSaveResponseError] = useState<string | null>(
    null
  );
  const [saveResponseWarning, setSaveResponseWarning] = useState<string | null>(
    null
  );
  const [responseSaved, setResponseSaved] = useState(false);
  const [captureKind, setCaptureKind] = useState<ExampleKind>("success");
  const [cacheEntry, setCacheEntry] = useState<RequestCacheEntry | null>(null);
  const [cacheEntries, setCacheEntries] = useState<RequestCacheEntry[]>([]);
  const [cacheListStatus, setCacheListStatus] = useState<
    "idle" | "loading" | "ready" | "unavailable"
  >("idle");
  const [cacheStatus, setCacheStatus] = useState<
    "idle" | "checking" | "hit" | "saved" | "unavailable"
  >("idle");
  const [cacheRoutingReloadState, setCacheRoutingReloadState] = useState<
    "idle" | "needed" | "reloading" | "reloaded"
  >("idle");
  const [cacheCaptureKinds, setCacheCaptureKinds] = useState<
    Record<string, ExampleKind>
  >({});
  const [savingCacheId, setSavingCacheId] = useState<string | null>(null);
  const [savedCacheId, setSavedCacheId] = useState<string | null>(null);
  const [removingCacheId, setRemovingCacheId] = useState<string | null>(null);
  const [clearingStaleCache, setClearingStaleCache] = useState(false);
  const [generatingCacheId, setGeneratingCacheId] = useState<string | null>(
    null
  );
  const [refreshedCacheId, setRefreshedCacheId] = useState<string | null>(null);
  const [previewingCacheId, setPreviewingCacheId] = useState<string | null>(
    null
  );
  const [generatingLatestResponse, setGeneratingLatestResponse] =
    useState(false);
  const [latestResponseRefreshed, setLatestResponseRefreshed] = useState(false);
  const [previewingLatestResponse, setPreviewingLatestResponse] =
    useState(false);
  const [refreshingStaleCache, setRefreshingStaleCache] = useState(false);
  const [staleRefreshProgress, setStaleRefreshProgress] = useState<{
    done: number;
    total: number;
  } | null>(null);
  const [staleRefreshSummary, setStaleRefreshSummary] = useState<string | null>(
    null
  );
  const bodyLint = useMemo(() => lintJson(bodyDraft), [bodyDraft]);
  const queryPairs: QueryPair[] = useMemo(
    () => parseQueryString(query),
    [query]
  );
  const staleCacheEntries = useMemo(
    () => cacheEntries.filter(isCacheEntryStale),
    [cacheEntries]
  );
  const refreshableStaleCacheEntries = useMemo(
    () =>
      staleCacheEntries.filter((entry) => {
        const snapshot = cacheResponseSnapshot(entry);
        return Boolean(snapshot && canPersistBody(snapshot.body));
      }),
    [staleCacheEntries]
  );
  const firstRefreshableStaleCacheEntry =
    refreshableStaleCacheEntries[0] ?? null;
  const staleRefreshDisabled =
    !connected ||
    !canGenerateFromCache ||
    refreshingStaleCache ||
    generatingLatestResponse ||
    generatingCacheId !== null ||
    clearingStaleCache;
  const staleRefreshLabel =
    refreshingStaleCache && staleRefreshProgress
      ? `Refreshing stale ${staleRefreshProgress.done}/${staleRefreshProgress.total}`
      : `AI refresh stale (${refreshableStaleCacheEntries.length})`;

  // Seed the Authorization (or custom) header the first time we see an
  // auth hint for a fresh draft. Runs per `routeKey`, not per render, so
  // flipping tabs doesn't clobber a value the user has typed — the guard
  // uses the currently-persisted header list at seed time.
  const seededFor = useRef<string | null>(null);
  useEffect(() => {
    const hint = tab.endpoint.auth;
    if (!hint) return;
    const placeholder = placeholderForAuthHint(hint);
    if (placeholder === null) return; // "other" — can't guess a shape.
    if (seededFor.current === routeKey) return;
    const alreadyHasHeader = headers.some(
      (row) => row.key.trim().toLowerCase() === hint.header_name.toLowerCase()
    );
    if (!alreadyHasHeader) {
      setHeaders([
        ...headers,
        { key: hint.header_name, value: placeholder }
      ]);
    }
    seededFor.current = routeKey;
    // We intentionally don't depend on `headers` here — this is a one-shot
    // per-route seed, not a reactive sync. Re-running on every header
    // edit would fight with the user.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [routeKey, tab.endpoint.auth]);

  const canSend = Boolean(baseUrl);
  const canUsePersistentCache =
    connected && isPersistedCollection(tab.collectionId);

  const loadRequestCache = useCallback(async () => {
    if (!canUsePersistentCache) {
      setCacheEntries([]);
      setCacheListStatus("unavailable");
      return;
    }
    setCacheListStatus("loading");
    try {
      const entries = await invoke<RequestCacheEntry[]>("list_request_cache", {
        args: {
          collection_id: tab.collectionId,
          method: tab.method,
          path: tab.endpoint.path,
          limit: 5,
          database_url: null
        }
      });
      const safeEntries = Array.isArray(entries) ? entries : [];
      setCacheEntries(safeEntries);
      setCacheCaptureKinds((current) => {
        const next = { ...current };
        for (const entry of safeEntries) {
          if (!next[entry.id]) {
            next[entry.id] = kindFromCacheEntry(entry);
          }
        }
        return next;
      });
      setCacheListStatus("ready");
    } catch {
      setCacheEntries([]);
      setCacheListStatus("unavailable");
    }
  }, [canUsePersistentCache, tab.collectionId, tab.endpoint.path, tab.method]);

  useEffect(() => {
    setCacheEntry(null);
    setCacheStatus("idle");
    setCacheRoutingReloadState("idle");
    setCacheEntries([]);
    setCacheCaptureKinds({});
    setSavedCacheId(null);
    setRemovingCacheId(null);
    setClearingStaleCache(false);
    setGeneratingCacheId(null);
    setRefreshedCacheId(null);
    setPreviewingCacheId(null);
    setGeneratingLatestResponse(false);
    setLatestResponseRefreshed(false);
    setPreviewingLatestResponse(false);
    setRefreshingStaleCache(false);
    setStaleRefreshProgress(null);
    setStaleRefreshSummary(null);
  }, [routeKey]);

  useEffect(() => {
    void loadRequestCache();
  }, [loadRequestCache]);

  async function send() {
    if (!baseUrl) return;
    setSending(true);
    setError(null);
    setSaveResponseError(null);
    setSaveResponseWarning(null);
    setResponseSaved(false);
    setLatestResponseRefreshed(false);
    try {
      const url = buildUrl(baseUrl, tab.endpoint.path, params, query);
      const method = tab.method.toUpperCase();
      const sendHeaders: Record<string, string> = {};
      for (const { key, value } of headers) {
        const trimmedKey = key.trim();
        if (!trimmedKey) continue;
        sendHeaders[trimmedKey] = value;
      }
      const requestSnapshot: RequestSnapshot = {
        query: query.trim().replace(/^\?/, ""),
        headers: sendHeaders,
        body: snapshotBody(method, bodyDraft)
      };
      const init: RequestInit = { method, headers: sendHeaders };
      if (bodyDraft.trim() && method !== "GET" && method !== "HEAD") {
        if (!Object.keys(sendHeaders).some((k) => k.toLowerCase() === "content-type")) {
          sendHeaders["content-type"] = "application/json";
        }
        init.body = bodyDraft;
      }
      const start = performance.now();
      const resp = await fetch(url, init);
      const elapsedMs = Math.round(performance.now() - start);
      const contentType = resp.headers.get("content-type") ?? "";
      // Read the raw bytes first so we can record size *before* JSON
      // parsing (which otherwise obscures the wire body). The overhead is
      // trivial for realistic mock payloads and keeps size accurate for
      // non-JSON responses too.
      const rawText = await resp.text();
      const sizeBytes = new Blob([rawText]).size;
      let body: unknown;
      if (contentType.includes("application/json")) {
        try {
          body = rawText ? JSON.parse(rawText) : "";
        } catch {
          body = "<invalid JSON>";
        }
      } else {
        body = rawText;
      }
      const headersOut: Record<string, string> = {};
      resp.headers.forEach((value, key) => {
        headersOut[key] = value;
      });
      setCaptureKind(kindFromStatus(resp.status));
      setResponse({
        status: resp.status,
        headers: headersOut,
        body,
        elapsedMs,
        sizeBytes,
        url,
        requestSnapshot
      });
      void syncRequestCache(requestSnapshot, {
        status: resp.status,
        headers: headersOut,
        body,
        elapsed_ms: elapsedMs,
        size_bytes: sizeBytes
      });
      history.record({
        status: resp.status,
        elapsedMs,
        method,
        url
      });
    } catch (err) {
      setError(`Request failed: ${String(err)}`);
    } finally {
      setSending(false);
    }
  }

  async function syncRequestCache(
    requestSnapshot: RequestSnapshot,
    responseSnapshot: ResponseSnapshot
  ) {
    if (!connected || !isPersistedCollection(tab.collectionId)) {
      setCacheStatus("unavailable");
      return;
    }
    setCacheStatus("checking");
    try {
      const saved = await invoke<RequestCacheEntry>("save_request_cache", {
        args: {
          collection_id: tab.collectionId,
          method: tab.method,
          path: tab.endpoint.path,
          request_snapshot: requestSnapshot,
          response_snapshot: responseSnapshot,
          database_url: null
        }
      });
      setCacheEntry(saved);
      setCacheStatus(saved.hit_count > 1 ? "hit" : "saved");
      setCacheRoutingReloadState(
        requestCacheRoutingEnabled && onReloadRequestCache ? "needed" : "idle"
      );
      await loadRequestCache();
    } catch {
      setCacheEntry(null);
      setCacheStatus("unavailable");
      setCacheRoutingReloadState("idle");
    }
  }

  async function reloadRequestCacheRouting() {
    if (!onReloadRequestCache) return;
    setCacheRoutingReloadState("reloading");
    setSaveResponseError(null);
    try {
      await onReloadRequestCache();
      setCacheRoutingReloadState("reloaded");
      window.setTimeout(() => setCacheRoutingReloadState("idle"), 2000);
    } catch (err) {
      setCacheRoutingReloadState("needed");
      setSaveResponseError(`Failed to reload request cache routing: ${String(err)}`);
    }
  }

  async function saveResponseAsExample() {
    if (!response || !onSaveResponseAsExample) return;
    if (!connected) {
      setSaveResponseError("Tauri runtime required to save captured responses.");
      return;
    }
    const kind = captureKind;
    const schema = pickResponseSchema(tab.endpoint.responses, kind);
    const schemaErrors = await validateCapturedPayload(schema, response.body);
    const schemaWarning =
      schemaErrors.length > 0
        ? `Captured response does not match the ${kind} schema: ${schemaErrors
            .slice(0, 3)
            .join("; ")}${schemaErrors.length > 3 ? `; +${schemaErrors.length - 3} more` : ""}`
        : null;
    const example: MockExample = {
      kind,
      title: `${KIND_LABEL[kind]} from Try-it`,
      payload: response.body,
      note: schemaWarning
        ? `Captured from Try-it response (${response.status}, ${response.elapsedMs}ms). Warning: schema mismatch.`
        : `Captured from Try-it response (${response.status}, ${response.elapsedMs}ms).`
    };
    setSavingResponse(true);
    setSaveResponseError(null);
    setSaveResponseWarning(schemaWarning);
    setResponseSaved(false);
    try {
      await onSaveResponseAsExample(tab, example);
      setResponseSaved(true);
      window.setTimeout(() => setResponseSaved(false), 1600);
    } catch (err) {
      setSaveResponseError(String(err));
    } finally {
      setSavingResponse(false);
    }
  }

  async function saveCacheEntryAsExample(entry: RequestCacheEntry) {
    if (!onSaveResponseAsExample) return;
    if (!connected) {
      setSaveResponseError("Tauri runtime required to save cached responses.");
      return;
    }
    const snapshot = cacheResponseSnapshot(entry);
    if (!snapshot || !canPersistBody(snapshot.body)) {
      setSaveResponseError("Cached response cannot be saved as a mock payload.");
      return;
    }
    const kind = cacheCaptureKinds[entry.id] ?? kindFromCacheEntry(entry);
    const schema = pickResponseSchema(tab.endpoint.responses, kind);
    const schemaErrors = await validateCapturedPayload(schema, snapshot.body);
    const schemaWarning =
      schemaErrors.length > 0
        ? `Cached response does not match the ${kind} schema: ${schemaErrors
            .slice(0, 3)
            .join("; ")}${schemaErrors.length > 3 ? `; +${schemaErrors.length - 3} more` : ""}`
        : null;
    const example: MockExample = {
      kind,
      title: `${KIND_LABEL[kind]} from cache`,
      payload: snapshot.body,
      note: schemaWarning
        ? `Captured from request cache (${snapshot.status}, hit ${entry.hit_count}). Warning: schema mismatch.`
        : `Captured from request cache (${snapshot.status}, hit ${entry.hit_count}).`
    };
    setSavingCacheId(entry.id);
    setSaveResponseError(null);
    setSaveResponseWarning(schemaWarning);
    setSavedCacheId(null);
    try {
      await onSaveResponseAsExample(tab, example);
      setSavedCacheId(entry.id);
      window.setTimeout(() => setSavedCacheId(null), 1600);
    } catch (err) {
      setSaveResponseError(String(err));
    } finally {
      setSavingCacheId(null);
    }
  }

  function replayCacheEntry(entry: RequestCacheEntry) {
    const snapshot = cacheRequestSnapshot(entry);
    if (!snapshot) {
      setSaveResponseError("Cached request cannot be replayed.");
      return;
    }
    setQuery(snapshot.query);
    setHeaders(
      Object.entries(snapshot.headers).map(([key, value]) => ({ key, value }))
    );
    setBodyDraft(bodyDraftFromSnapshot(snapshot.body));
    setSaveResponseError(null);
  }

  async function removeCacheEntry(entry: RequestCacheEntry) {
    if (!canUsePersistentCache) {
      setSaveResponseError("Tauri runtime required to remove cached requests.");
      return;
    }
    setRemovingCacheId(entry.id);
    setSaveResponseError(null);
    try {
      await invoke<boolean>("delete_request_cache", {
        args: {
          collection_id: tab.collectionId,
          method: tab.method,
          path: tab.endpoint.path,
          cache_id: entry.id,
          database_url: null
        }
      });
      setCacheEntries((current) =>
        current.filter((cached) => cached.id !== entry.id)
      );
      setCacheCaptureKinds((current) => {
        const next = { ...current };
        delete next[entry.id];
        return next;
      });
      setSavedCacheId((current) => (current === entry.id ? null : current));
      setCacheEntry((current) => (current?.id === entry.id ? null : current));
      await loadRequestCache();
    } catch (err) {
      setSaveResponseError(`Failed to remove cached fingerprint: ${String(err)}`);
    } finally {
      setRemovingCacheId(null);
    }
  }

  async function clearStaleCacheEntries() {
    if (!canUsePersistentCache) {
      setSaveResponseError("Tauri runtime required to clear cached requests.");
      return;
    }
    if (staleCacheEntries.length === 0) return;
    const threshold = Math.floor((Date.now() - CACHE_STALE_AFTER_MS) / 1000);
    setClearingStaleCache(true);
    setSaveResponseError(null);
    try {
      await invoke<number>("delete_stale_request_cache", {
        args: {
          collection_id: tab.collectionId,
          method: tab.method,
          path: tab.endpoint.path,
          stale_before_epoch_seconds: threshold,
          database_url: null
        }
      });
      const staleIds = new Set(staleCacheEntries.map((entry) => entry.id));
      setCacheEntries((current) =>
        current.filter((entry) => !staleIds.has(entry.id))
      );
      setCacheCaptureKinds((current) => {
        const next = { ...current };
        for (const id of staleIds) {
          delete next[id];
        }
        return next;
      });
      setSavedCacheId((current) =>
        current && staleIds.has(current) ? null : current
      );
      setCacheEntry((current) =>
        current && staleIds.has(current.id) ? null : current
      );
      await loadRequestCache();
    } catch (err) {
      setSaveResponseError(`Failed to clear stale fingerprints: ${String(err)}`);
    } finally {
      setClearingStaleCache(false);
    }
  }

  function buildGenerationContextFromCache(
    entry: RequestCacheEntry
  ): GenerationContext | null {
    const snapshot = cacheResponseSnapshot(entry);
    const requestSnapshot = cacheRequestSnapshot(entry);
    if (!snapshot || !canPersistBody(snapshot.body)) {
      return null;
    }
    return {
      request_snapshot: requestSnapshot ?? entry.request_snapshot,
      response_snapshot: snapshot,
      note: `request cache ${entry.fingerprint}, hit ${entry.hit_count}, last seen ${entry.last_seen_at}`
    };
  }

  function buildGenerationContextFromLatestResponse(
    latest: ResponseState
  ): GenerationContext | null {
    if (!canPersistBody(latest.body)) {
      return null;
    }
    return {
      request_snapshot: latest.requestSnapshot,
      response_snapshot: {
        status: latest.status,
        headers: latest.headers,
        body: latest.body,
        elapsed_ms: latest.elapsedMs,
        size_bytes: latest.sizeBytes
      },
      note: `latest Try-it response ${latest.status}, ${latest.elapsedMs}ms, ${latest.url}`
    };
  }

  async function generateFromLatestResponse() {
    if (!response || !onGenerateFromCache) return;
    if (!connected) {
      setSaveResponseError("Tauri runtime required to refresh mocks with AI.");
      return;
    }
    if (!canGenerateFromCache) {
      setSaveResponseError("Provider model and base URL are required.");
      return;
    }
    const context = buildGenerationContextFromLatestResponse(response);
    if (!context) {
      setSaveResponseError("Latest response cannot be used as AI refresh context.");
      return;
    }
    setGeneratingLatestResponse(true);
    setLatestResponseRefreshed(false);
    setSaveResponseError(null);
    try {
      await onGenerateFromCache(tab, captureKind, context);
      setLatestResponseRefreshed(true);
      window.setTimeout(() => setLatestResponseRefreshed(false), 1600);
    } catch (err) {
      setSaveResponseError(`AI refresh failed: ${String(err)}`);
    } finally {
      setGeneratingLatestResponse(false);
    }
  }

  async function previewPromptFromLatestResponse() {
    if (!response || !onPreviewPromptFromCache) return;
    if (!connected) {
      setSaveResponseError("Tauri runtime required to preview AI prompts.");
      return;
    }
    const context = buildGenerationContextFromLatestResponse(response);
    if (!context) {
      setSaveResponseError("Latest response cannot be used as prompt context.");
      return;
    }
    setPreviewingLatestResponse(true);
    setSaveResponseError(null);
    try {
      await onPreviewPromptFromCache(tab, captureKind, context);
    } catch (err) {
      setSaveResponseError(`Prompt preview failed: ${String(err)}`);
    } finally {
      setPreviewingLatestResponse(false);
    }
  }

  async function generateFromCacheEntry(entry: RequestCacheEntry) {
    if (!onGenerateFromCache) return;
    if (!connected) {
      setSaveResponseError("Tauri runtime required to refresh mocks with AI.");
      return;
    }
    const context = buildGenerationContextFromCache(entry);
    if (!context) {
      setSaveResponseError("Cached response cannot be used as AI refresh context.");
      return;
    }
    const kind = cacheCaptureKinds[entry.id] ?? kindFromCacheEntry(entry);
    setGeneratingCacheId(entry.id);
    setSaveResponseError(null);
    try {
      await onGenerateFromCache(tab, kind, context);
      setRefreshedCacheId(entry.id);
      window.setTimeout(() => setRefreshedCacheId(null), 1600);
    } catch (err) {
      setSaveResponseError(`AI refresh failed: ${String(err)}`);
    } finally {
      setGeneratingCacheId(null);
    }
  }

  async function refreshStaleCacheEntries() {
    if (!onGenerateFromCache) return;
    if (!connected) {
      setSaveResponseError("Tauri runtime required to refresh mocks with AI.");
      return;
    }
    if (!canGenerateFromCache) {
      setSaveResponseError("Provider model and base URL are required.");
      return;
    }

    const entries = refreshableStaleCacheEntries;
    if (entries.length === 0 || refreshingStaleCache || generatingCacheId) {
      return;
    }

    setRefreshingStaleCache(true);
    setStaleRefreshProgress({ done: 0, total: entries.length });
    setStaleRefreshSummary(null);
    setSaveResponseError(null);
    setGeneratingCacheId(null);
    setRefreshedCacheId(null);

    let successCount = 0;
    const failures: string[] = [];
    for (const entry of entries) {
      const context = buildGenerationContextFromCache(entry);
      if (!context) {
        failures.push(entry.fingerprint);
        setStaleRefreshProgress((current) =>
          current ? { ...current, done: current.done + 1 } : current
        );
        continue;
      }
      const kind = cacheCaptureKinds[entry.id] ?? kindFromCacheEntry(entry);
      setGeneratingCacheId(entry.id);
      try {
        await onGenerateFromCache(tab, kind, context);
        successCount += 1;
        setRefreshedCacheId(entry.id);
      } catch {
        failures.push(entry.fingerprint);
      } finally {
        setStaleRefreshProgress((current) =>
          current ? { ...current, done: current.done + 1 } : current
        );
      }
    }

    setGeneratingCacheId(null);
    setRefreshingStaleCache(false);
    setStaleRefreshProgress(null);
    setStaleRefreshSummary(
      `Refreshed ${successCount}/${entries.length} stale cached fingerprints.`
    );
    if (successCount > 0) {
      window.setTimeout(() => setRefreshedCacheId(null), 1600);
    }
    if (failures.length > 0) {
      setSaveResponseError(
        `AI refresh failed for ${failures.length} stale cached fingerprint${
          failures.length === 1 ? "" : "s"
        }: ${failures.slice(0, 3).join(", ")}${
          failures.length > 3 ? `, +${failures.length - 3} more` : ""
        }`
      );
    } else {
      window.setTimeout(() => setStaleRefreshSummary(null), 2400);
    }
  }

  async function previewPromptFromCacheEntry(entry: RequestCacheEntry) {
    if (!onPreviewPromptFromCache) return;
    if (!connected) {
      setSaveResponseError("Tauri runtime required to preview AI prompts.");
      return;
    }
    const context = buildGenerationContextFromCache(entry);
    if (!context) {
      setSaveResponseError("Cached response cannot be used as prompt context.");
      return;
    }
    const kind = cacheCaptureKinds[entry.id] ?? kindFromCacheEntry(entry);
    setPreviewingCacheId(entry.id);
    setSaveResponseError(null);
    try {
      await onPreviewPromptFromCache(tab, kind, context);
    } catch (err) {
      setSaveResponseError(`Prompt preview failed: ${String(err)}`);
    } finally {
      setPreviewingCacheId(null);
    }
  }

  async function validateCapturedPayload(
    schema: ReturnType<typeof pickResponseSchema>,
    payload: unknown
  ): Promise<string[]> {
    if (!schema) return [];
    if (connected) {
      try {
        const result = await invoke<ValidateMockPayloadResult>(
          "validate_mock_payload",
          {
            args: {
              schema,
              payload
            }
          }
        );
        return result.errors;
      } catch {
        // Static previews and older backends may not expose the command yet.
        // Keep a lightweight client-side warning path instead of blocking save.
      }
    }
    return validateAgainstSchema(schema, payload);
  }

  function handlePanelKey(event: React.KeyboardEvent<HTMLElement>) {
    // Mod+Enter fires from anywhere inside the panel, including the body
    // textarea. This matches Postman / Insomnia muscle memory.
    const isMac = /mac|iphone|ipad|ipod/i.test(navigator.platform);
    const mod = isMac ? event.metaKey : event.ctrlKey;
    if (mod && event.key === "Enter" && canSend && !sending) {
      event.preventDefault();
      void send();
    }
  }

  return (
    <section className="tryit" onKeyDown={handlePanelKey}>
      <header className="tryit__head">
        <h3>Try it</h3>
        {baseUrl ? (
          <span className="tryit__base" title={baseUrl}>
            {baseUrl}
          </span>
        ) : (
          <span className="tryit__base tryit__base--warn">
            mock server not running
          </span>
        )}
      </header>

      {pathParams.length > 0 ? (
        <div className="tryit__section">
          <span className="tryit__label">Path params</span>
          <div className="tryit__grid">
            {pathParams.map((name) => (
              <label key={name} className="tryit__field">
                <span>{`{${name}}`}</span>
                <input
                  type="text"
                  value={params[name] ?? ""}
                  onChange={(event) =>
                    setParams({ ...params, [name]: event.target.value })
                  }
                  spellCheck={false}
                />
              </label>
            ))}
          </div>
        </div>
      ) : null}

      <div className="tryit__section">
        <div className="tryit__label tryit__label--row">
          <span>Query string</span>
          <button
            type="button"
            className="btn btn--ghost btn--sm"
            onClick={() => {
              const next = [...queryPairs, { key: "", value: "" }];
              setQuery(serializeQueryString(next));
            }}
          >
            <Icon name="plus" size={12} />
            <span>Add</span>
          </button>
        </div>
        {queryPairs.length === 0 ? (
          <div className="tryit__hint">No query parameters.</div>
        ) : (
          <div className="tryit__headers">
            {queryPairs.map((pair, idx) => (
              <div key={idx} className="tryit__header-row">
                <input
                  type="text"
                  placeholder="key"
                  value={pair.key}
                  onChange={(event) => {
                    const next = queryPairs.map((p, i) =>
                      i === idx ? { ...p, key: event.target.value } : p
                    );
                    setQuery(serializeQueryString(next));
                  }}
                  spellCheck={false}
                />
                <input
                  type="text"
                  placeholder="value"
                  value={pair.value}
                  onChange={(event) => {
                    const next = queryPairs.map((p, i) =>
                      i === idx ? { ...p, value: event.target.value } : p
                    );
                    setQuery(serializeQueryString(next));
                  }}
                  spellCheck={false}
                />
                <button
                  type="button"
                  className="btn btn--icon"
                  onClick={() => {
                    const next = queryPairs.filter((_, i) => i !== idx);
                    setQuery(serializeQueryString(next));
                  }}
                  title="Remove"
                  aria-label={`Remove query param ${pair.key || "(blank)"}`}
                >
                  <Icon name="close" size={12} />
                </button>
              </div>
            ))}
          </div>
        )}
        <details className="tryit__raw-query">
          <summary>Raw ({query.length ? `${query.length} chars` : "empty"})</summary>
          <input
            type="text"
            placeholder="e.g. status=paid&limit=10"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            spellCheck={false}
            aria-label="Raw query string"
          />
        </details>
      </div>

      <div className="tryit__section">
        <div className="tryit__label tryit__label--row">
          <span>Headers</span>
          <button
            type="button"
            className="btn btn--ghost btn--sm"
            onClick={() => setHeaders([...headers, { key: "", value: "" }])}
          >
            <Icon name="plus" size={12} />
            <span>Add</span>
          </button>
        </div>
        {headers.length === 0 ? (
          <div className="tryit__hint">No custom headers.</div>
        ) : (
          <div className="tryit__headers">
            {headers.map((header, idx) => (
              <div key={idx} className="tryit__header-row">
                <input
                  type="text"
                  placeholder="Authorization"
                  value={header.key}
                  onChange={(event) => {
                    const next = [...headers];
                    next[idx] = { ...next[idx], key: event.target.value };
                    setHeaders(next);
                  }}
                  spellCheck={false}
                />
                <input
                  type="text"
                  placeholder="Bearer …"
                  value={header.value}
                  onChange={(event) => {
                    const next = [...headers];
                    next[idx] = { ...next[idx], value: event.target.value };
                    setHeaders(next);
                  }}
                  spellCheck={false}
                />
                <button
                  type="button"
                  className="btn btn--icon btn--icon-sm"
                  onClick={() => setHeaders(headers.filter((_, i) => i !== idx))}
                  aria-label="Remove header"
                >
                  <Icon name="close" size={10} />
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      {tab.method.toUpperCase() !== "GET" &&
      tab.method.toUpperCase() !== "HEAD" ? (
        <div className="tryit__section">
          <label className="tryit__field">
            <span className="tryit__label--row">
              <span>Request body (JSON)</span>
              {tab.endpoint.request_body ? (
                <button
                  type="button"
                  className="btn btn--ghost btn--sm"
                  onClick={async () => {
                    try {
                      const synthesized = await invoke<unknown>(
                        "synthesize_request_body",
                        { endpoint: tab.endpoint }
                      );
                      if (synthesized !== null && synthesized !== undefined) {
                        setBodyDraft(JSON.stringify(synthesized, null, 2));
                      }
                    } catch {
                      /* ignore — fall back to manual entry */
                    }
                  }}
                  title="Generate a sample body from the endpoint schema"
                >
                  <Icon name="zap" size={12} />
                  <span>Fill from schema</span>
                </button>
              ) : null}
            </span>
            <textarea
              value={bodyDraft}
              onChange={(event) => setBodyDraft(event.target.value)}
              placeholder={'{\n  "name": "value"\n}'}
              spellCheck={false}
              rows={6}
              aria-invalid={!bodyLint.ok}
            />
            <div
              className={
                bodyLint.ok
                  ? "tryit__lint tryit__lint--ok"
                  : "tryit__lint tryit__lint--err"
              }
              role="status"
              aria-live="polite"
            >
              {bodyLint.ok
                ? bodyLint.empty
                  ? "empty body"
                  : "✓ valid JSON"
                : bodyLint.line && bodyLint.column
                ? `× line ${bodyLint.line}, col ${bodyLint.column} — ${bodyLint.message}`
                : `× ${bodyLint.message}`}
            </div>
          </label>
        </div>
      ) : null}

      <div className="row-actions">
        <button
          type="button"
          className="btn btn--primary btn--sm"
          onClick={send}
          disabled={!canSend || sending}
        >
          <Icon name="paper-plane" size={12} />
          <span>{sending ? "Sending…" : `Send ${tab.method}`}</span>
        </button>
        <button
          type="button"
          className="btn btn--ghost btn--sm"
          onClick={() => {
            reset();
            setResponse(null);
            setError(null);
          }}
          disabled={sending}
          title="Clear inputs for this endpoint"
        >
          Clear
        </button>
        {response ? (
          <span className="tryit__meta">
            {response.status} · {response.elapsedMs}ms ·{" "}
            {formatBytes(response.sizeBytes)}
          </span>
        ) : null}
        {response ? (
          <span
            className={
              cacheStatus === "hit"
                ? "tryit__cache tryit__cache--hit"
                : "tryit__cache"
            }
            title={cacheEntry ? `fingerprint ${cacheEntry.fingerprint}` : undefined}
          >
            {cacheStatus === "checking"
              ? "cache syncing…"
              : cacheStatus === "hit" && cacheEntry
                ? `cache hit ×${cacheEntry.hit_count}`
                : cacheStatus === "saved" && cacheEntry
                  ? "cached"
                  : "cache unavailable"}
          </span>
        ) : null}
        {response ? (
          <button
            type="button"
            className="btn btn--ghost btn--sm"
            onClick={() => {
              const body =
                typeof response.body === "string"
                  ? response.body
                  : JSON.stringify(response.body, null, 2);
              void navigator.clipboard?.writeText(body);
            }}
            title="Copy the response body to clipboard"
          >
            <Icon name="copy" size={12} />
            <span>Copy body</span>
          </button>
        ) : null}
        {response && onSaveResponseAsExample ? (
          <>
            <label className="tryit__capture-kind">
              <span>as</span>
              <select
                value={captureKind}
                onChange={(event) =>
                  setCaptureKind(event.target.value as ExampleKind)
                }
                disabled={savingResponse}
                aria-label="Mock kind for captured response"
              >
                <option value="success">Success</option>
                <option value="empty">Empty</option>
                <option value="error">Error</option>
              </select>
            </label>
            <button
              type="button"
              className="btn btn--ghost btn--sm"
              onClick={saveResponseAsExample}
              disabled={
                savingResponse ||
                !connected ||
                !canPersistBody(response.body)
              }
              title={
                connected
                  ? "Save the latest Try-it response as this endpoint's mock payload"
                  : "Tauri runtime required to persist captured responses"
              }
            >
              <Icon name="save" size={12} />
              <span>
                {savingResponse
                  ? "Saving…"
                  : responseSaved
                    ? "Saved"
                : "Save as mock"}
              </span>
            </button>
            {onGenerateFromCache ? (
              <button
                type="button"
                className="btn btn--ghost btn--sm"
                onClick={() => void generateFromLatestResponse()}
                disabled={
                  !connected ||
                  !canGenerateFromCache ||
                  generatingLatestResponse ||
                  refreshingStaleCache ||
                  generatingCacheId !== null ||
                  !canPersistBody(response.body)
                }
                title={
                  canGenerateFromCache
                    ? "Refresh this mock with AI using the latest Try-it request context"
                    : "Provider model and base URL are required"
                }
              >
                <Icon name="sparkles" size={12} />
                <span>
                  {generatingLatestResponse
                    ? "Refreshing…"
                    : latestResponseRefreshed
                      ? "Refreshed"
                      : "AI refresh latest"}
                </span>
              </button>
            ) : null}
            {onPreviewPromptFromCache ? (
              <button
                type="button"
                className="btn btn--ghost btn--sm"
                onClick={() => void previewPromptFromLatestResponse()}
                disabled={
                  !connected ||
                  previewingLatestResponse ||
                  !canPersistBody(response.body)
                }
                title="Preview the AI prompt using the latest Try-it request context"
              >
                <Icon name="info" size={12} />
                <span>
                  {previewingLatestResponse ? "Opening…" : "Prompt latest"}
                </span>
              </button>
            ) : null}
          </>
        ) : null}
      </div>

      {error ? (
        <div className="banner banner--error" role="status">
          {error}
        </div>
      ) : null}

      {saveResponseError ? (
        <div className="banner banner--error" role="status">
          {saveResponseError}
        </div>
      ) : null}

      {saveResponseWarning ? (
        <div className="banner banner--warn" role="status">
          <Icon name="info" size={12} /> {saveResponseWarning}
        </div>
      ) : null}

      {staleRefreshSummary ? (
        <div className="tryit__cache-detail" role="status">
          {staleRefreshSummary}
        </div>
      ) : null}

      {cacheListStatus === "ready" && staleCacheEntries.length > 0 ? (
        <section className="tryit__refresh-queue" aria-label="Stale cache refresh queue">
          <div className="tryit__refresh-queue-head">
            <div>
              <strong>Refresh queue</strong>
              <span>
                {staleCacheEntries.length} stale ·{" "}
                {refreshableStaleCacheEntries.length} refreshable
              </span>
            </div>
            <span className="tryit__cache-age tryit__cache-age--stale">
              {cacheAgeLabel(staleCacheEntries[0])}
            </span>
          </div>
          <p>
            Stale cached fingerprints can refresh mock examples with the saved
            request and response context. Albert does not resend requests or
            delete cache rows during AI refresh.
          </p>
          {refreshableStaleCacheEntries.length > 0 ? (
            <div className="tryit__refresh-queue-preview">
              {refreshableStaleCacheEntries.slice(0, 3).map((entry) => (
                <code key={entry.id} title={entry.fingerprint}>
                  {entry.fingerprint}
                </code>
              ))}
              {refreshableStaleCacheEntries.length > 3 ? (
                <span>+{refreshableStaleCacheEntries.length - 3} more</span>
              ) : null}
            </div>
          ) : (
            <div className="tryit__cache-detail">
              No stale responses have reusable JSON/text bodies.
            </div>
          )}
          <div className="row-actions">
            {onGenerateFromCache && refreshableStaleCacheEntries.length > 0 ? (
              <button
                type="button"
                className="btn btn--primary btn--sm"
                onClick={() => void refreshStaleCacheEntries()}
                disabled={staleRefreshDisabled}
                title={
                  canGenerateFromCache
                    ? "Refresh stale mocks with AI using cached request contexts"
                    : "Provider model and base URL are required"
                }
              >
                <Icon name="sparkles" size={12} />
                <span>{staleRefreshLabel}</span>
              </button>
            ) : null}
            {onPreviewPromptFromCache && firstRefreshableStaleCacheEntry ? (
              <button
                type="button"
                className="btn btn--ghost btn--sm"
                onClick={() =>
                  void previewPromptFromCacheEntry(firstRefreshableStaleCacheEntry)
                }
                disabled={
                  !connected ||
                  previewingCacheId === firstRefreshableStaleCacheEntry.id ||
                  refreshingStaleCache
                }
                title="Preview the AI prompt for the first stale cached request"
              >
                <Icon name="info" size={12} />
                <span>
                  {previewingCacheId === firstRefreshableStaleCacheEntry.id
                    ? "Opening…"
                    : "Preview first"}
                </span>
              </button>
            ) : null}
            <button
              type="button"
              className="btn btn--ghost btn--sm"
              onClick={() => void clearStaleCacheEntries()}
              disabled={clearingStaleCache || refreshingStaleCache}
              title="Remove stale cached fingerprints for this endpoint"
            >
              <Icon name="close" size={12} />
              <span>
                {clearingStaleCache
                  ? "Clearing…"
                  : `Clear stale (${staleCacheEntries.length})`}
              </span>
            </button>
          </div>
        </section>
      ) : null}

      {response ? (
        <div className="tryit__response">
          {cacheEntry ? (
            <div className="tryit__cache-detail">
              Request fingerprint cached at {displayCacheTime(cacheEntry.last_seen_at)}.
            </div>
          ) : null}
          {cacheRoutingReloadState !== "idle" ? (
            <div className="tryit__cache-routing" role="status">
              <span>
                {cacheRoutingReloadState === "reloaded"
                  ? "Request cache routing reloaded."
                  : "Reload routing to serve this cached response immediately."}
              </span>
              {cacheRoutingReloadState !== "reloaded" ? (
                <button
                  type="button"
                  className="btn btn--ghost btn--sm"
                  onClick={() => void reloadRequestCacheRouting()}
                  disabled={cacheRoutingReloadState === "reloading"}
                >
                  <Icon name="refresh" size={12} />
                  <span>
                    {cacheRoutingReloadState === "reloading"
                      ? "Reloading…"
                      : "Reload routing"}
                  </span>
                </button>
              ) : null}
            </div>
          ) : null}
          <div className="tryit__response-heads">
            {Object.entries(response.headers)
              .filter(([key]) => key.startsWith("x-albert-") || key === "content-type")
              .map(([key, value]) => (
                <span key={key} className="tryit__header">
                  <b>{key}</b>: {value}
                </span>
              ))}
          </div>
          {typeof response.body === "string" ? (
            <pre className="code-block code-block--wrap">{response.body}</pre>
          ) : (
            <JsonView value={response.body} />
          )}
        </div>
      ) : null}

      {cacheListStatus === "ready" && cacheEntries.length > 0 ? (
        <details className="tryit__cache-list">
          <summary>
            <span>Cached fingerprints ({cacheEntries.length})</span>
            {staleCacheEntries.length > 0 ? (
              <span className="tryit__cache-age tryit__cache-age--stale">
                {staleCacheEntries.length} stale
              </span>
            ) : null}
          </summary>
          <ul className="tryit__cache-list-rows">
            {cacheEntries.map((entry) => {
              const snapshot = cacheResponseSnapshot(entry);
              const kind = cacheCaptureKinds[entry.id] ?? kindFromCacheEntry(entry);
              const requestSnapshot = cacheRequestSnapshot(entry);
              const stale = isCacheEntryStale(entry);
              return (
                <li key={entry.id} className="tryit__cache-list-row">
                  <div className="tryit__cache-list-main">
                    <span
                      className={
                        snapshot && snapshot.status >= 400
                          ? "tryit__history-status tryit__history-status--err"
                          : "tryit__history-status"
                      }
                    >
                      {snapshot ? snapshot.status : "?"}
                    </span>
                    <span className="tryit__history-time">
                      {displayCacheTime(entry.last_seen_at)}
                    </span>
                    <span
                      className="tryit__history-url"
                      title={entry.fingerprint}
                    >
                      {entry.fingerprint}
                    </span>
                    <span className="tryit__history-ms">
                      hit ×{entry.hit_count}
                    </span>
                    <span
                      className={
                        stale
                          ? "tryit__cache-age tryit__cache-age--stale"
                          : "tryit__cache-age"
                      }
                    >
                      {stale ? `stale · ${cacheAgeLabel(entry)}` : cacheAgeLabel(entry)}
                    </span>
                  </div>
                  <div className="tryit__cache-list-actions">
                    <button
                      type="button"
                      className="btn btn--ghost btn--sm"
                      onClick={() => replayCacheEntry(entry)}
                      disabled={!requestSnapshot}
                      title="Load this cached request into the Try-it draft"
                    >
                      <Icon name="refresh" size={12} />
                      <span>Replay</span>
                    </button>
                    {onSaveResponseAsExample ? (
                      <>
                        <select
                          value={kind}
                          onChange={(event) =>
                            setCacheCaptureKinds({
                              ...cacheCaptureKinds,
                              [entry.id]: event.target.value as ExampleKind
                            })
                          }
                          disabled={savingCacheId === entry.id}
                          aria-label={`Mock kind for cached response ${entry.fingerprint}`}
                        >
                          <option value="success">Success</option>
                          <option value="empty">Empty</option>
                          <option value="error">Error</option>
                        </select>
                        <button
                          type="button"
                          className="btn btn--ghost btn--sm"
                          onClick={() => void saveCacheEntryAsExample(entry)}
                          disabled={
                            !connected ||
                            savingCacheId === entry.id ||
                            !snapshot ||
                            !canPersistBody(snapshot.body)
                          }
                          title="Save this cached response as the endpoint mock payload"
                        >
                          <Icon name="save" size={12} />
                          <span>
                            {savingCacheId === entry.id
                              ? "Saving…"
                              : savedCacheId === entry.id
                                ? "Saved"
                                : "Save"}
                          </span>
                        </button>
                      </>
                    ) : null}
                    {onGenerateFromCache ? (
                      <button
                        type="button"
                        className="btn btn--ghost btn--sm"
                        onClick={() => void generateFromCacheEntry(entry)}
                        disabled={
                          !connected ||
                          !canGenerateFromCache ||
                          refreshingStaleCache ||
                          generatingLatestResponse ||
                          generatingCacheId === entry.id ||
                          !snapshot ||
                          !canPersistBody(snapshot.body)
                        }
                        title={
                          canGenerateFromCache
                            ? "Refresh this mock with AI using the cached request context"
                            : "Provider model and base URL are required"
                        }
                      >
                        <Icon name="sparkles" size={12} />
                        <span>
                          {generatingCacheId === entry.id
                            ? "Refreshing…"
                            : refreshedCacheId === entry.id
                              ? "Refreshed"
                              : "AI refresh"}
                        </span>
                      </button>
                    ) : null}
                    {onPreviewPromptFromCache ? (
                      <button
                        type="button"
                        className="btn btn--ghost btn--sm"
                        onClick={() => void previewPromptFromCacheEntry(entry)}
                        disabled={
                          !connected ||
                          previewingCacheId === entry.id ||
                          !snapshot ||
                          !canPersistBody(snapshot.body)
                        }
                        title="Preview the AI prompt using this cached request context"
                      >
                        <Icon name="info" size={12} />
                        <span>
                          {previewingCacheId === entry.id
                            ? "Opening…"
                            : "Prompt"}
                        </span>
                      </button>
                    ) : null}
                    <button
                      type="button"
                      className="btn btn--ghost btn--sm"
                      onClick={() => void removeCacheEntry(entry)}
                      disabled={removingCacheId === entry.id}
                      title="Remove this cached fingerprint"
                    >
                      <Icon name="close" size={12} />
                      <span>
                        {removingCacheId === entry.id ? "Removing…" : "Remove"}
                      </span>
                    </button>
                  </div>
                </li>
              );
            })}
          </ul>
        </details>
      ) : cacheListStatus === "loading" ? (
        <div className="tryit__cache-detail">Loading cached fingerprints…</div>
      ) : null}

      {history.history.length > 0 ? (
        <details className="tryit__history">
          <summary>
            Recent ({history.history.length})
            <button
              type="button"
              className="btn btn--ghost btn--sm tryit__history-clear"
              onClick={(event) => {
                event.preventDefault();
                event.stopPropagation();
                history.clear();
              }}
            >
              Clear
            </button>
          </summary>
          <ul className="tryit__history-list">
            {history.history.map((entry, idx) => (
              <li key={`${entry.at}-${idx}`} className="tryit__history-row">
                <span
                  className={
                    entry.status >= 400
                      ? "tryit__history-status tryit__history-status--err"
                      : "tryit__history-status"
                  }
                >
                  {entry.status}
                </span>
                <span className="tryit__history-time">
                  {new Date(entry.at).toLocaleTimeString(undefined, {
                    hour12: false
                  })}
                </span>
                <span className="tryit__history-url" title={entry.url}>
                  {entry.method} {entry.url}
                </span>
                <span className="tryit__history-ms">{entry.elapsedMs}ms</span>
              </li>
            ))}
          </ul>
        </details>
      ) : null}
    </section>
  );
}
