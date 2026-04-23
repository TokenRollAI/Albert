import { useMemo, useState } from "react";
import { Icon } from "./Icon";
import type {
  GatewayStatus,
  MockExampleKind,
  RequestLogEntry
} from "../types";

interface MockServerPanelProps {
  open: boolean;
  onClose: () => void;
  connected: boolean;
  status: GatewayStatus;
  busy: boolean;
  error: string | null;
  requests: RequestLogEntry[];
  onStart: (port: number, host: string, cors: boolean) => Promise<void>;
  onStop: () => Promise<void>;
  onApplyOverrides: (
    overrides: Record<string, MockExampleKind>
  ) => Promise<void>;
}

type TabKey = "runtime" | "routes" | "requests";

export function MockServerPanel({
  open,
  onClose,
  connected,
  status,
  busy,
  error,
  requests,
  onStart,
  onStop,
  onApplyOverrides
}: MockServerPanelProps) {
  const [host, setHost] = useState<string>(status.config.host ?? "127.0.0.1");
  const [port, setPort] = useState<string>(
    String(status.config.port ?? 4317)
  );
  const [cors, setCors] = useState<boolean>(status.config.cors_enabled);
  const [copied, setCopied] = useState<string | null>(null);
  const [tab, setTab] = useState<TabKey>("runtime");
  const [draftOverrides, setDraftOverrides] = useState<
    Record<string, MockExampleKind>
  >({});
  const [applyBusy, setApplyBusy] = useState(false);

  const bind = status.bind_address ?? (status.running ? `${host}:${port}` : "—");
  const baseUrl = useMemo(
    () => (status.running && status.bind_address ? `http://${status.bind_address}` : null),
    [status.running, status.bind_address]
  );

  const mergedOverrides = useMemo(() => {
    const merged: Record<string, MockExampleKind> = {
      ...status.config.example_overrides
    };
    for (const [k, v] of Object.entries(draftOverrides)) {
      merged[k] = v;
    }
    return merged;
  }, [status.config.example_overrides, draftOverrides]);

  if (!open) return null;

  async function handleStart() {
    const numericPort = Number.parseInt(port, 10);
    if (!Number.isFinite(numericPort) || numericPort < 0 || numericPort > 65535) {
      return;
    }
    await onStart(numericPort, host || "127.0.0.1", cors);
  }

  async function copyToClipboard(value: string) {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(value);
      window.setTimeout(() => setCopied(null), 1200);
    } catch {
      // ignore
    }
  }

  async function applyOverrides() {
    setApplyBusy(true);
    try {
      await onApplyOverrides(mergedOverrides);
      setDraftOverrides({});
    } finally {
      setApplyBusy(false);
    }
  }

  function setOverrideForRoute(key: string, kind: MockExampleKind) {
    setDraftOverrides((prev) => ({ ...prev, [key]: kind }));
  }

  return (
    <div className="drawer" role="dialog" aria-label="Mock Server">
      <div className="drawer__backdrop" onClick={onClose} />
      <div className="drawer__panel drawer__panel--lg">
        <header className="drawer__head">
          <div className="drawer__title">
            <Icon name="server" size={16} />
            <h2>Mock Server</h2>
            <span
              className={
                status.running
                  ? "pill pill--ok"
                  : connected
                  ? "pill pill--idle"
                  : "pill pill--warn"
              }
            >
              {status.running ? "running" : connected ? "idle" : "offline"}
            </span>
          </div>
          <button
            type="button"
            className="btn btn--icon"
            onClick={onClose}
            aria-label="Close mock server panel"
          >
            <Icon name="close" size={16} />
          </button>
        </header>

        <div className="drawer__tabs" role="tablist">
          {(["runtime", "routes", "requests"] as TabKey[]).map((key) => (
            <button
              key={key}
              type="button"
              role="tab"
              className={tab === key ? "tab tab--active" : "tab"}
              onClick={() => setTab(key)}
            >
              {key === "runtime"
                ? "Runtime"
                : key === "routes"
                ? `Routes (${status.route_count})`
                : `Requests (${requests.length})`}
            </button>
          ))}
        </div>

        <div className="drawer__body">
          {tab === "runtime" ? (
            <section className="panel">
              <h3 className="panel__title">Listener</h3>
              <div className="formgrid">
                <label className="field">
                  <span className="field__label">Host</span>
                  <input
                    type="text"
                    value={host}
                    onChange={(event) => setHost(event.target.value)}
                    spellCheck={false}
                    disabled={status.running}
                  />
                </label>
                <label className="field">
                  <span className="field__label">Port</span>
                  <input
                    type="text"
                    inputMode="numeric"
                    value={port}
                    onChange={(event) => setPort(event.target.value)}
                    spellCheck={false}
                    disabled={status.running}
                  />
                </label>
                <label className="field field--check">
                  <input
                    type="checkbox"
                    checked={cors}
                    onChange={(event) => setCors(event.target.checked)}
                    disabled={status.running}
                  />
                  <span>Enable permissive CORS</span>
                </label>
              </div>
              <div className="row-actions">
                {status.running ? (
                  <button
                    type="button"
                    className="btn btn--danger"
                    onClick={onStop}
                    disabled={busy}
                  >
                    <Icon name="stop" size={14} />
                    <span>Stop</span>
                  </button>
                ) : (
                  <button
                    type="button"
                    className="btn btn--primary"
                    onClick={handleStart}
                    disabled={!connected || busy}
                  >
                    <Icon name="play" size={14} />
                    <span>Start</span>
                  </button>
                )}
                {baseUrl ? (
                  <button
                    type="button"
                    className="btn btn--ghost"
                    onClick={() => copyToClipboard(baseUrl)}
                  >
                    <Icon name="copy" size={14} />
                    <span>{copied === baseUrl ? "Copied!" : "Copy base URL"}</span>
                  </button>
                ) : null}
              </div>
              <p className="hint">
                Bound to: <code>{bind}</code>. Append{" "}
                <code>?__albert_mock=error</code> to any route to force the
                error example for a single request.
              </p>
              {error ? (
                <div className="banner banner--error" role="status">
                  {error}
                </div>
              ) : null}
            </section>
          ) : null}

          {tab === "routes" ? (
            <section className="panel">
              <div className="panel__title panel__title--row">
                <h3>Routes</h3>
                <span className="panel__meta">
                  {status.route_count} registered
                </span>
              </div>
              {status.routes.length === 0 ? (
                <div className="empty">
                  No routes. Import a collection first, then start the server.
                </div>
              ) : (
                <ul className="routelist">
                  {status.routes.map((route) => {
                    const key = `${route.method} ${route.path}`;
                    const override = mergedOverrides[key];
                    const selected =
                      override ??
                      route.selected_example ??
                      ("success" as MockExampleKind);
                    const url = baseUrl
                      ? `${baseUrl}${route.path}`
                      : route.path;
                    return (
                      <li key={key} className="routelist__item routelist__item--wide">
                        <span
                          className={`method method--${route.method.toLowerCase()}`}
                        >
                          {route.method.toUpperCase()}
                        </span>
                        <span className="routelist__path" title={url}>
                          {route.path}
                        </span>
                        <select
                          className="select"
                          value={selected}
                          onChange={(event) =>
                            setOverrideForRoute(
                              key,
                              event.target.value as MockExampleKind
                            )
                          }
                          disabled={!status.running}
                        >
                          {route.available_examples.map((kind) => (
                            <option key={kind} value={kind}>
                              {kind}
                            </option>
                          ))}
                        </select>
                        <button
                          type="button"
                          className="btn btn--ghost btn--sm"
                          disabled={!baseUrl}
                          onClick={() => copyToClipboard(url)}
                          title="Copy URL"
                        >
                          <Icon name="copy" size={12} />
                          <span>{copied === url ? "Copied" : "URL"}</span>
                        </button>
                      </li>
                    );
                  })}
                </ul>
              )}
              <div className="row-actions">
                <button
                  type="button"
                  className="btn btn--primary btn--sm"
                  onClick={applyOverrides}
                  disabled={
                    applyBusy ||
                    !status.running ||
                    Object.keys(draftOverrides).length === 0
                  }
                >
                  <Icon name="zap" size={12} />
                  <span>
                    {applyBusy
                      ? "Applying…"
                      : `Apply (${Object.keys(draftOverrides).length})`}
                  </span>
                </button>
                {Object.keys(draftOverrides).length > 0 ? (
                  <button
                    type="button"
                    className="btn btn--ghost btn--sm"
                    onClick={() => setDraftOverrides({})}
                  >
                    Clear changes
                  </button>
                ) : null}
              </div>
            </section>
          ) : null}

          {tab === "requests" ? (
            <section className="panel">
              <div className="panel__title panel__title--row">
                <h3>Recent requests</h3>
                <span className="panel__meta">
                  last {requests.length} · refreshes every 3s
                </span>
              </div>
              {requests.length === 0 ? (
                <div className="empty">
                  No requests captured yet. Try <code>curl {baseUrl ?? "http://..."}
                  </code> to hit a route.
                </div>
              ) : (
                <ul className="reqlog">
                  {requests.map((entry, idx) => (
                    <li
                      key={`${entry.at_epoch_ms}-${idx}-${entry.path}`}
                      className="reqlog__item"
                    >
                      <span className="reqlog__time">
                        {formatTime(entry.at_epoch_ms)}
                      </span>
                      <span
                        className={`method method--${entry.method.toLowerCase()}`}
                      >
                        {entry.method.toUpperCase()}
                      </span>
                      <span className="reqlog__path">
                        {entry.path}
                        {entry.query ? (
                          <span className="reqlog__query">?{entry.query}</span>
                        ) : null}
                      </span>
                      <span
                        className={
                          entry.status >= 400
                            ? "reqlog__status reqlog__status--err"
                            : "reqlog__status"
                        }
                      >
                        {entry.status}
                      </span>
                      {entry.kind ? (
                        <span className={`kind-chip kind-chip--${entry.kind}`}>
                          {entry.kind}
                        </span>
                      ) : (
                        <span className="kind-chip">{entry.source}</span>
                      )}
                    </li>
                  ))}
                </ul>
              )}
            </section>
          ) : null}
        </div>
      </div>
    </div>
  );
}

function formatTime(ms: number): string {
  const d = new Date(ms);
  return d.toLocaleTimeString(undefined, { hour12: false });
}
