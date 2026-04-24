import { useMemo, useRef, useState } from "react";
import { Icon } from "./Icon";
import { MockRequestsTab } from "./MockRequestsTab";
import { MockRuntimeTab } from "./MockRuntimeTab";
import type {
  GatewayStatus,
  MockExampleKind,
  RateLimitRule,
  RequestLogEntry,
  StoredScenarioSummary
} from "../types";

interface MockServerPanelProps {
  open: boolean;
  onClose: () => void;
  connected: boolean;
  status: GatewayStatus;
  busy: boolean;
  error: string | null;
  requests: RequestLogEntry[];
  savedPreferences?: {
    host?: string;
    port?: number;
    cors_enabled?: boolean;
  } | null;
  onStart: (port: number, host: string, cors: boolean) => Promise<void>;
  onStop: () => Promise<void>;
  onApplyOverrides: (
    overrides: Record<string, MockExampleKind>
  ) => Promise<void>;
  onApplyChaos: (defaultLatencyMs: number, errorRate: number) => Promise<void>;
  onToggleCaptureBodies: (enabled: boolean) => Promise<void>;
  onToggleEnforceRequestBodies: (enabled: boolean) => Promise<void>;
  onApplyRateLimits: (rules: Record<string, RateLimitRule>) => Promise<void>;
  onApplyStatusOverrides: (rules: Record<string, number>) => Promise<void>;
  onApplyResponseHeaders: (
    rules: Record<string, Record<string, string>>
  ) => Promise<void>;
  onSeedRequiredHeadersFromHints: () => Promise<void>;
  onApplyProxyUpstream?: (upstream: string | null) => Promise<void>;
  onClearLog?: () => Promise<void>;
  onExportBundle?: () => Promise<void>;
  onImportBundle?: (bundleJson: string) => Promise<void>;
  onReplayRequest?: (entry: RequestLogEntry) => void;
  scenarios?: {
    list: () => Promise<StoredScenarioSummary[]>;
    save: (name: string) => Promise<void>;
    load: (name: string) => Promise<void>;
    del: (name: string) => Promise<void>;
    rename: (oldName: string, newName: string) => Promise<void>;
  };
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
  savedPreferences,
  onStart,
  onStop,
  onApplyOverrides,
  onApplyChaos,
  onToggleCaptureBodies,
  onToggleEnforceRequestBodies,
  onApplyRateLimits,
  onApplyStatusOverrides,
  onApplyResponseHeaders,
  onSeedRequiredHeadersFromHints,
  onApplyProxyUpstream,
  onClearLog,
  onExportBundle,
  onImportBundle,
  onReplayRequest,
  scenarios
}: MockServerPanelProps) {
  const [copied, setCopied] = useState<string | null>(null);
  const [tab, setTab] = useState<TabKey>("runtime");
  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const [draftOverrides, setDraftOverrides] = useState<
    Record<string, MockExampleKind>
  >({});
  const [applyBusy, setApplyBusy] = useState(false);

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
          <div className="drawer__head-actions">
            {baseUrl ? (
              <a
                className="btn btn--ghost btn--sm"
                href={`${baseUrl}/__albert/docs`}
                target="_blank"
                rel="noopener noreferrer"
                title="Open Swagger UI for the live spec in your browser"
              >
                <Icon name="link" size={12} />
                <span>Docs</span>
              </a>
            ) : null}
            {onExportBundle ? (
              <button
                type="button"
                className="btn btn--ghost btn--sm"
                onClick={() => void onExportBundle()}
                disabled={!status.running}
                title={
                  status.running
                    ? "Download the live config as a JSON bundle"
                    : "Start the server first"
                }
              >
                <Icon name="save" size={12} />
                <span>Export</span>
              </button>
            ) : null}
            {onImportBundle ? (
              <>
                <button
                  type="button"
                  className="btn btn--ghost btn--sm"
                  onClick={() => fileInputRef.current?.click()}
                  disabled={!status.running}
                  title={
                    status.running
                      ? "Apply a previously-exported config bundle"
                      : "Start the server first"
                  }
                >
                  <Icon name="import" size={12} />
                  <span>Import</span>
                </button>
                <input
                  ref={fileInputRef}
                  type="file"
                  accept="application/json,.json"
                  style={{ display: "none" }}
                  onChange={async (event) => {
                    const file = event.target.files?.[0];
                    event.target.value = "";
                    if (!file) return;
                    try {
                      const text = await file.text();
                      await onImportBundle(text);
                    } catch {
                      /* surfaced via toast from the caller */
                    }
                  }}
                />
              </>
            ) : null}
            <button
              type="button"
              className="btn btn--icon"
              onClick={onClose}
              aria-label="Close mock server panel"
            >
              <Icon name="close" size={16} />
            </button>
          </div>
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
            <MockRuntimeTab
              status={status}
              connected={connected}
              busy={busy}
              error={error}
              savedPreferences={savedPreferences}
              onStart={onStart}
              onStop={onStop}
              onApplyChaos={onApplyChaos}
              onToggleEnforceRequestBodies={onToggleEnforceRequestBodies}
              onApplyRateLimits={onApplyRateLimits}
              onApplyStatusOverrides={onApplyStatusOverrides}
              onApplyResponseHeaders={onApplyResponseHeaders}
              onSeedRequiredHeadersFromHints={onSeedRequiredHeadersFromHints}
              onApplyProxyUpstream={onApplyProxyUpstream}
              scenarios={scenarios}
            />
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
            <MockRequestsTab
              status={status}
              requests={requests}
              baseUrl={baseUrl}
              onToggleCaptureBodies={onToggleCaptureBodies}
              onClearLog={onClearLog}
              onReplayRequest={onReplayRequest}
            />
          ) : null}
        </div>
      </div>
    </div>
  );
}
