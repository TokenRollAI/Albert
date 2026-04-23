import { useEffect, useMemo, useState } from "react";
import { Icon } from "./Icon";
import { JsonView } from "./JsonView";
import type { EndpointTab } from "../types";

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
  const [params, setParams] = useState<Record<string, string>>({});
  const [query, setQuery] = useState<string>("");
  const [bodyDraft, setBodyDraft] = useState<string>("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [response, setResponse] = useState<ResponseState | null>(null);

  useEffect(() => {
    setParams({});
    setQuery("");
    setBodyDraft("");
    setResponse(null);
    setError(null);
  }, [tab.id]);

  const canSend = Boolean(baseUrl);

  async function send() {
    if (!baseUrl) return;
    setSending(true);
    setError(null);
    try {
      const url = buildUrl(baseUrl, tab.endpoint.path, params, query);
      const method = tab.method.toUpperCase();
      const headers: Record<string, string> = {};
      const init: RequestInit = { method, headers };
      if (bodyDraft.trim() && method !== "GET" && method !== "HEAD") {
        headers["content-type"] = "application/json";
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
    } catch (err) {
      setError(`Request failed: ${String(err)}`);
    } finally {
      setSending(false);
    }
  }

  return (
    <section className="tryit">
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
                    setParams((prev) => ({ ...prev, [name]: event.target.value }))
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

      {tab.method.toUpperCase() !== "GET" &&
      tab.method.toUpperCase() !== "HEAD" ? (
        <div className="tryit__section">
          <label className="tryit__field">
            <span>Request body (JSON)</span>
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
    </section>
  );
}
