import { useMemo, useState } from "react";
import { Icon } from "./Icon";
import type { GatewayStatus } from "../types";

interface MockServerPanelProps {
  open: boolean;
  onClose: () => void;
  connected: boolean;
  status: GatewayStatus;
  busy: boolean;
  error: string | null;
  onStart: (port: number, host: string, cors: boolean) => Promise<void>;
  onStop: () => Promise<void>;
}

export function MockServerPanel({
  open,
  onClose,
  connected,
  status,
  busy,
  error,
  onStart,
  onStop
}: MockServerPanelProps) {
  const [host, setHost] = useState<string>(status.config.host ?? "127.0.0.1");
  const [port, setPort] = useState<string>(
    String(status.config.port ?? 4317)
  );
  const [cors, setCors] = useState<boolean>(status.config.cors_enabled);
  const [copied, setCopied] = useState<string | null>(null);

  const bind = status.bind_address ?? (status.running ? `${host}:${port}` : "—");
  const baseUrl = useMemo(
    () => (status.running && status.bind_address ? `http://${status.bind_address}` : null),
    [status.running, status.bind_address]
  );

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

        <div className="drawer__body">
          <section className="panel">
            <h3 className="panel__title">Runtime</h3>
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
                  <span>
                    {copied === baseUrl ? "Copied!" : "Copy base URL"}
                  </span>
                </button>
              ) : null}
            </div>
            {error ? (
              <div className="banner banner--error" role="status">
                {error}
              </div>
            ) : null}
          </section>

          <section className="panel">
            <div className="panel__title panel__title--row">
              <h3>Registered routes</h3>
              <span className="panel__meta">
                {status.route_count} route{status.route_count === 1 ? "" : "s"}
                {" · "}bound to {bind}
              </span>
            </div>
            {status.routes.length === 0 ? (
              <div className="empty">
                No routes registered. Import a collection first.
              </div>
            ) : (
              <ul className="routelist">
                {status.routes.map((route) => {
                  const url = baseUrl ? `${baseUrl}${route.path}` : route.path;
                  const key = `${route.method} ${route.path}`;
                  return (
                    <li key={key} className="routelist__item">
                      <span
                        className={`method method--${route.method.toLowerCase()}`}
                      >
                        {route.method.toUpperCase()}
                      </span>
                      <span className="routelist__path" title={url}>
                        {route.path}
                      </span>
                      <span className="routelist__meta">
                        {route.collection_name}
                        {route.selected_example ? (
                          <span
                            className={`kind-chip kind-chip--${route.selected_example}`}
                          >
                            {route.selected_example}
                          </span>
                        ) : null}
                      </span>
                      <button
                        type="button"
                        className="btn btn--ghost btn--sm"
                        disabled={!baseUrl}
                        onClick={() => copyToClipboard(url)}
                        title="Copy URL"
                      >
                        <Icon name="copy" size={12} />
                        <span>{copied === url ? "Copied" : "Copy"}</span>
                      </button>
                    </li>
                  );
                })}
              </ul>
            )}
            {baseUrl ? (
              <p className="hint">
                Tip: append <code>?__albert_mock=error</code> to force the error
                example for any route.
              </p>
            ) : null}
          </section>
        </div>
      </div>
    </div>
  );
}
