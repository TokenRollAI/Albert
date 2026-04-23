import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useRef, useState } from "react";
import { Icon } from "./Icon";
import { JsonView } from "./JsonView";
import { useTryItDraft } from "../hooks/useTryItDraft";
import { useTryItHistory } from "../hooks/useTryItHistory";
import type { AuthRequirementHint, EndpointTab } from "../types";

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
}

interface ResponseState {
  status: number;
  headers: Record<string, string>;
  body: unknown;
  elapsedMs: number;
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

export function TryItPanel({ tab, baseUrl }: TryItPanelProps) {
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

  async function send() {
    if (!baseUrl) return;
    setSending(true);
    setError(null);
    try {
      const url = buildUrl(baseUrl, tab.endpoint.path, params, query);
      const method = tab.method.toUpperCase();
      const sendHeaders: Record<string, string> = {};
      for (const { key, value } of headers) {
        const trimmedKey = key.trim();
        if (!trimmedKey) continue;
        sendHeaders[trimmedKey] = value;
      }
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
      let body: unknown;
      if (contentType.includes("application/json")) {
        body = await resp.json().catch(() => "<invalid JSON>");
      } else {
        body = await resp.text();
      }
      const headersOut: Record<string, string> = {};
      resp.headers.forEach((value, key) => {
        headersOut[key] = value;
      });
      setResponse({
        status: resp.status,
        headers: headersOut,
        body,
        elapsedMs
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
        <label className="tryit__field">
          <span>Query string</span>
          <input
            type="text"
            placeholder="e.g. status=paid&limit=10"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            spellCheck={false}
          />
        </label>
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
            />
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
            {response.status} · {response.elapsedMs}ms
          </span>
        ) : null}
      </div>

      {error ? (
        <div className="banner banner--error" role="status">
          {error}
        </div>
      ) : null}

      {response ? (
        <div className="tryit__response">
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
