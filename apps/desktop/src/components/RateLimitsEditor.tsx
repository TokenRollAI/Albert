import { useMemo, useState } from "react";
import { Icon } from "./Icon";
import type { GatewayRouteSummary, RateLimitRule } from "../types";

interface RateLimitsEditorProps {
  running: boolean;
  routes: GatewayRouteSummary[];
  value: Record<string, RateLimitRule>;
  onApply: (next: Record<string, RateLimitRule>) => Promise<void>;
}

/**
 * Compact per-route rate-limit editor. Lets the user add, edit, and delete
 * sliding-window rules keyed by `METHOD /path`. Collects changes as a draft
 * so `Apply` sends one atomic update through the gateway hot-reload surface.
 */
export function RateLimitsEditor({
  running,
  routes,
  value,
  onApply
}: RateLimitsEditorProps) {
  const [draft, setDraft] = useState<Record<string, RateLimitRule>>(value);
  const [selectedRoute, setSelectedRoute] = useState<string>(() =>
    routes.length > 0 ? routeKeyOf(routes[0]) : ""
  );
  const [limit, setLimit] = useState<string>("10");
  const [windowMs, setWindowMs] = useState<string>("1000");
  const [busy, setBusy] = useState(false);

  const currentEntries = useMemo(
    () =>
      Object.entries(draft).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)),
    [draft]
  );

  const dirty = useMemo(
    () => JSON.stringify(draft) !== JSON.stringify(value),
    [draft, value]
  );

  function addRule() {
    const key = selectedRoute.trim();
    if (!key) return;
    const parsedLimit = Math.max(0, Number.parseInt(limit, 10) || 0);
    const parsedWindow = Math.max(1, Number.parseInt(windowMs, 10) || 1);
    setDraft((prev) => ({
      ...prev,
      [key]: { limit: parsedLimit, window_ms: parsedWindow }
    }));
  }

  function removeRule(key: string) {
    setDraft((prev) => {
      const { [key]: _dropped, ...rest } = prev;
      return rest;
    });
  }

  function updateDraftRule(key: string, patch: Partial<RateLimitRule>) {
    setDraft((prev) => ({
      ...prev,
      [key]: { ...prev[key], ...patch }
    }));
  }

  async function apply() {
    setBusy(true);
    try {
      await onApply(draft);
    } finally {
      setBusy(false);
    }
  }

  function resetToCurrent() {
    setDraft(value);
  }

  return (
    <section className="panel">
      <div className="panel__title panel__title--row">
        <h3>Rate limits</h3>
        <span className="panel__meta">
          sliding window · METHOD /path · 429 on exceed
        </span>
      </div>

      <div className="formgrid formgrid--three">
        <label className="field">
          <span className="field__label">Route</span>
          <select
            className="select"
            value={selectedRoute}
            onChange={(event) => setSelectedRoute(event.target.value)}
            disabled={!running || routes.length === 0}
          >
            {routes.length === 0 ? <option value="">No routes</option> : null}
            {routes.map((route) => {
              const key = routeKeyOf(route);
              return (
                <option key={key} value={key}>
                  {key}
                </option>
              );
            })}
          </select>
        </label>
        <label className="field">
          <span className="field__label">Limit</span>
          <input
            type="text"
            inputMode="numeric"
            value={limit}
            onChange={(event) => setLimit(event.target.value)}
            disabled={!running}
            spellCheck={false}
          />
        </label>
        <label className="field">
          <span className="field__label">Window (ms)</span>
          <input
            type="text"
            inputMode="numeric"
            value={windowMs}
            onChange={(event) => setWindowMs(event.target.value)}
            disabled={!running}
            spellCheck={false}
          />
        </label>
      </div>
      <div className="row-actions">
        <button
          type="button"
          className="btn btn--ghost btn--sm"
          onClick={addRule}
          disabled={!running || !selectedRoute}
        >
          <Icon name="plus" size={12} />
          <span>Add / replace rule</span>
        </button>
      </div>

      {currentEntries.length === 0 ? (
        <p className="hint">
          No rate limits defined. Add one to cap a noisy route (e.g. limit 5
          per 1000ms) or set limit 0 to simulate a maintenance window.
        </p>
      ) : (
        <ul className="rl-list">
          {currentEntries.map(([key, rule]) => (
            <li key={key} className="rl-list__row">
              <code className="rl-list__key">{key}</code>
              <label className="rl-list__field">
                <span className="rl-list__label">limit</span>
                <input
                  type="text"
                  inputMode="numeric"
                  value={String(rule.limit)}
                  onChange={(event) =>
                    updateDraftRule(key, {
                      limit: Math.max(
                        0,
                        Number.parseInt(event.target.value, 10) || 0
                      )
                    })
                  }
                  disabled={!running}
                />
              </label>
              <label className="rl-list__field">
                <span className="rl-list__label">window (ms)</span>
                <input
                  type="text"
                  inputMode="numeric"
                  value={String(rule.window_ms)}
                  onChange={(event) =>
                    updateDraftRule(key, {
                      window_ms: Math.max(
                        1,
                        Number.parseInt(event.target.value, 10) || 1
                      )
                    })
                  }
                  disabled={!running}
                />
              </label>
              <button
                type="button"
                className="btn btn--icon"
                onClick={() => removeRule(key)}
                title="Remove rule"
                aria-label={`Remove rate limit for ${key}`}
                disabled={!running}
              >
                <Icon name="close" size={12} />
              </button>
            </li>
          ))}
        </ul>
      )}

      <div className="row-actions">
        <button
          type="button"
          className="btn btn--primary btn--sm"
          onClick={apply}
          disabled={!running || busy || !dirty}
        >
          <Icon name="zap" size={12} />
          <span>{busy ? "Applying…" : "Apply"}</span>
        </button>
        {dirty ? (
          <button
            type="button"
            className="btn btn--ghost btn--sm"
            onClick={resetToCurrent}
            disabled={busy}
          >
            Reset changes
          </button>
        ) : null}
      </div>
    </section>
  );
}

function routeKeyOf(route: GatewayRouteSummary): string {
  return `${route.method} ${route.path}`;
}
