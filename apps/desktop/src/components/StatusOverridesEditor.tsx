import { useMemo, useState } from "react";
import { Icon } from "./Icon";
import type { GatewayRouteSummary } from "../types";

interface StatusOverridesEditorProps {
  running: boolean;
  routes: GatewayRouteSummary[];
  value: Record<string, number>;
  onApply: (next: Record<string, number>) => Promise<void>;
}

/**
 * Per-route HTTP status override editor. Mirrors the RateLimitsEditor
 * pattern: pick a route, type a status code, "Add / replace" drafts the
 * change, "Apply" sends one atomic update via `update_mock_server`.
 *
 * Validation is intentionally lenient: out-of-range codes (outside
 * 100–599) are rejected at the UI layer with a visible hint, and the
 * gateway silently falls back to the kind-default for stale entries that
 * slip through (e.g. via direct command call), so a bad config never
 * strands a route.
 */
export function StatusOverridesEditor({
  running,
  routes,
  value,
  onApply
}: StatusOverridesEditorProps) {
  const [draft, setDraft] = useState<Record<string, number>>(value);
  const [selectedRoute, setSelectedRoute] = useState<string>(() =>
    routes.length > 0 ? routeKeyOf(routes[0]) : ""
  );
  const [code, setCode] = useState<string>("201");
  const [busy, setBusy] = useState(false);

  const parsedCode = Number.parseInt(code, 10);
  const codeValid =
    Number.isFinite(parsedCode) && parsedCode >= 100 && parsedCode <= 599;

  const entries = useMemo(
    () =>
      Object.entries(draft).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)),
    [draft]
  );

  const dirty = useMemo(
    () => JSON.stringify(draft) !== JSON.stringify(value),
    [draft, value]
  );

  function addRule() {
    if (!selectedRoute.trim() || !codeValid) return;
    setDraft((prev) => ({ ...prev, [selectedRoute]: parsedCode }));
  }

  function removeRule(key: string) {
    setDraft((prev) => {
      const { [key]: _dropped, ...rest } = prev;
      return rest;
    });
  }

  function updateDraftCode(key: string, nextCode: number) {
    setDraft((prev) => ({ ...prev, [key]: nextCode }));
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
        <h3>Status overrides</h3>
        <span className="panel__meta">
          per-route HTTP status · overrides kind default
        </span>
      </div>

      <div className="formgrid">
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
          <span className="field__label">Status (100–599)</span>
          <input
            type="text"
            inputMode="numeric"
            value={code}
            onChange={(event) => setCode(event.target.value)}
            disabled={!running}
            spellCheck={false}
            aria-invalid={!codeValid}
          />
        </label>
      </div>
      <div className="row-actions">
        <button
          type="button"
          className="btn btn--ghost btn--sm"
          onClick={addRule}
          disabled={!running || !selectedRoute || !codeValid}
        >
          <Icon name="plus" size={12} />
          <span>Add / replace</span>
        </button>
        {!codeValid && code.trim() !== "" ? (
          <span className="tryit__lint tryit__lint--err">
            × status must be between 100 and 599
          </span>
        ) : null}
      </div>

      {entries.length === 0 ? (
        <p className="hint">
          No status overrides. Defaults are 200 for success/empty examples
          and 400 for error. Add one to model <code>201 Created</code>,
          <code> 204 No Content</code>, or a differentiated error like
          <code> 403 Forbidden</code>.
        </p>
      ) : (
        <ul className="rl-list">
          {entries.map(([key, status]) => (
            <li key={key} className="rl-list__row rl-list__row--two">
              <code className="rl-list__key">{key}</code>
              <label className="rl-list__field">
                <span className="rl-list__label">status</span>
                <input
                  type="text"
                  inputMode="numeric"
                  value={String(status)}
                  onChange={(event) => {
                    const next = Number.parseInt(event.target.value, 10);
                    if (
                      Number.isFinite(next) &&
                      next >= 100 &&
                      next <= 599
                    ) {
                      updateDraftCode(key, next);
                    }
                  }}
                  disabled={!running}
                />
              </label>
              <button
                type="button"
                className="btn btn--icon"
                onClick={() => removeRule(key)}
                title="Remove override"
                aria-label={`Remove status override for ${key}`}
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
            Reset
          </button>
        ) : null}
      </div>
    </section>
  );
}

function routeKeyOf(route: GatewayRouteSummary): string {
  return `${route.method} ${route.path}`;
}
