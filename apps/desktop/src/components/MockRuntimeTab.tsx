import { useMemo, useState } from "react";
import { Icon } from "./Icon";
import { RateLimitsEditor } from "./RateLimitsEditor";
import { ResponseHeadersEditor } from "./ResponseHeadersEditor";
import { StatusOverridesEditor } from "./StatusOverridesEditor";
import type { GatewayStatus, RateLimitRule } from "../types";

interface MockRuntimeTabProps {
  status: GatewayStatus;
  connected: boolean;
  busy: boolean;
  error: string | null;
  savedPreferences?: {
    host?: string;
    port?: number;
    cors_enabled?: boolean;
  } | null;
  onStart: (port: number, host: string, cors: boolean) => Promise<void>;
  onStop: () => Promise<void>;
  onApplyChaos: (defaultLatencyMs: number, errorRate: number) => Promise<void>;
  onApplyRateLimits: (rules: Record<string, RateLimitRule>) => Promise<void>;
  onApplyStatusOverrides: (rules: Record<string, number>) => Promise<void>;
  onApplyResponseHeaders: (
    rules: Record<string, Record<string, string>>
  ) => Promise<void>;
  onSeedRequiredHeadersFromHints: () => Promise<void>;
}

/**
 * The "Runtime" tab of the Mock Server drawer. Owns the host/port/CORS
 * form + Chaos controls, and embeds the per-route editors
 * (RateLimitsEditor / StatusOverridesEditor / ResponseHeadersEditor).
 * Extracted from MockServerPanel so the parent can focus on chrome
 * (tab switcher, drawer layout) instead of runtime detail.
 */
export function MockRuntimeTab({
  status,
  connected,
  busy,
  error,
  savedPreferences,
  onStart,
  onStop,
  onApplyChaos,
  onApplyRateLimits,
  onApplyStatusOverrides,
  onApplyResponseHeaders,
  onSeedRequiredHeadersFromHints
}: MockRuntimeTabProps) {
  const initialHost =
    savedPreferences?.host ?? status.config.host ?? "127.0.0.1";
  const initialPort = String(
    savedPreferences?.port ?? status.config.port ?? 4317
  );
  const initialCors = savedPreferences?.cors_enabled ?? status.config.cors_enabled;
  const [host, setHost] = useState<string>(initialHost);
  const [port, setPort] = useState<string>(initialPort);
  const [cors, setCors] = useState<boolean>(initialCors);
  const [copied, setCopied] = useState<string | null>(null);
  const [latencyMs, setLatencyMs] = useState<string>(
    String(status.config.default_latency_ms ?? 0)
  );
  const [errorRatePct, setErrorRatePct] = useState<string>(
    String(Math.round((status.config.error_rate ?? 0) * 100))
  );
  const [chaosBusy, setChaosBusy] = useState(false);

  const bind = status.bind_address ?? (status.running ? `${host}:${port}` : "—");
  const baseUrl = useMemo(
    () => (status.running && status.bind_address ? `http://${status.bind_address}` : null),
    [status.running, status.bind_address]
  );

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

  async function applyChaos() {
    const latency = Math.max(0, Number.parseInt(latencyMs, 10) || 0);
    const errorPct = Math.max(0, Math.min(100, Number.parseInt(errorRatePct, 10) || 0));
    setChaosBusy(true);
    try {
      await onApplyChaos(latency, errorPct / 100);
    } finally {
      setChaosBusy(false);
    }
  }

  const requiredHeaderCount = Object.keys(
    status.config.required_headers ?? {}
  ).length;

  return (
    <>
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

      <section className="panel">
        <div className="panel__title panel__title--row">
          <h3>Chaos controls</h3>
          <span className="panel__meta">
            latency floor · random error rate
          </span>
        </div>
        <div className="formgrid">
          <label className="field">
            <span className="field__label">Default latency (ms)</span>
            <input
              type="text"
              inputMode="numeric"
              value={latencyMs}
              onChange={(event) => setLatencyMs(event.target.value)}
              spellCheck={false}
              disabled={!status.running}
            />
          </label>
          <label className="field">
            <span className="field__label">Error rate (0–100%)</span>
            <input
              type="text"
              inputMode="numeric"
              value={errorRatePct}
              onChange={(event) => setErrorRatePct(event.target.value)}
              spellCheck={false}
              disabled={!status.running}
            />
          </label>
        </div>
        <div className="row-actions">
          <button
            type="button"
            className="btn btn--primary btn--sm"
            onClick={applyChaos}
            disabled={!status.running || chaosBusy}
          >
            <Icon name="zap" size={12} />
            <span>{chaosBusy ? "Applying…" : "Apply chaos"}</span>
          </button>
          <button
            type="button"
            className="btn btn--ghost btn--sm"
            onClick={() => {
              setLatencyMs("0");
              setErrorRatePct("0");
              void onApplyChaos(0, 0);
            }}
            disabled={!status.running || chaosBusy}
          >
            Reset
          </button>
        </div>
        <p className="hint">
          Delay and error rate apply to all routes while the server runs.
          Per-route latency overrides can be added via the Tauri API
          directly.
        </p>
      </section>

      <RateLimitsEditor
        running={status.running}
        routes={status.routes}
        value={status.config.rate_limits ?? {}}
        onApply={onApplyRateLimits}
      />

      <StatusOverridesEditor
        running={status.running}
        routes={status.routes}
        value={status.config.status_overrides ?? {}}
        onApply={onApplyStatusOverrides}
      />

      <ResponseHeadersEditor
        running={status.running}
        routes={status.routes}
        value={status.config.response_headers ?? {}}
        onApply={onApplyResponseHeaders}
      />

      <section className="panel">
        <div className="panel__title panel__title--row">
          <h3>Auth gates</h3>
          <span className="panel__meta">
            {requiredHeaderCount} active rule
            {requiredHeaderCount === 1 ? "" : "s"}
          </span>
        </div>
        <p className="hint">
          Seed <code>required_headers</code> rules from the OpenAPI
          <code> securitySchemes</code> declarations captured at import
          time. Unauthorized requests then return 401 before touching
          mock data. Only HTTP bearer / basic, OAuth2, and
          header-placed API keys are seedable; other schemes surface
          as notes.
        </p>
        <div className="row-actions">
          <button
            type="button"
            className="btn btn--primary btn--sm"
            onClick={() => void onSeedRequiredHeadersFromHints()}
            disabled={!status.running}
          >
            <Icon name="shield" size={12} />
            <span>Seed from OpenAPI security</span>
          </button>
        </div>
      </section>
    </>
  );
}
