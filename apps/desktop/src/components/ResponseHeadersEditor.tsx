import { useMemo, useState } from "react";
import { Icon } from "./Icon";
import type { GatewayRouteSummary } from "../types";

interface ResponseHeadersEditorProps {
  running: boolean;
  routes: GatewayRouteSummary[];
  value: Record<string, Record<string, string>>;
  onApply: (next: Record<string, Record<string, string>>) => Promise<void>;
}

/**
 * Per-route response-header editor. Unlike the other per-route rule maps
 * this one is two-level: `{ METHOD /path: { header: value } }`. The UI
 * flattens that into a single list of rows `{ route, name, value }` so
 * editing is straightforward; reconstruction happens on Apply.
 */
export function ResponseHeadersEditor({
  running,
  routes,
  value,
  onApply
}: ResponseHeadersEditorProps) {
  type Row = { route: string; name: string; value: string };
  const initialRows: Row[] = useMemo(() => flatten(value), [value]);
  const [rows, setRows] = useState<Row[]>(initialRows);
  const [selectedRoute, setSelectedRoute] = useState<string>(() =>
    routes.length > 0 ? routeKeyOf(routes[0]) : ""
  );
  const [newName, setNewName] = useState<string>("x-request-id");
  const [newValue, setNewValue] = useState<string>("abc-123");
  const [busy, setBusy] = useState(false);

  const draft = useMemo(() => unflatten(rows), [rows]);
  const dirty = useMemo(
    () => JSON.stringify(draft) !== JSON.stringify(value),
    [draft, value]
  );

  function addRow() {
    const key = selectedRoute.trim();
    const headerName = newName.trim();
    if (!key || !headerName) return;
    // Replace if a row for (route, name) already exists — editing the
    // value is the common case, not adding dupes.
    setRows((prev) => {
      const next = prev.filter(
        (r) =>
          !(
            r.route === key &&
            r.name.toLowerCase() === headerName.toLowerCase()
          )
      );
      next.push({ route: key, name: headerName, value: newValue });
      return next;
    });
  }

  function removeRow(index: number) {
    setRows((prev) => prev.filter((_, i) => i !== index));
  }

  function updateRow(index: number, patch: Partial<Row>) {
    setRows((prev) =>
      prev.map((row, i) => (i === index ? { ...row, ...patch } : row))
    );
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
    setRows(initialRows);
  }

  return (
    <section className="panel">
      <div className="panel__title panel__title--row">
        <h3>Response headers</h3>
        <span className="panel__meta">
          per-route extras · merged with built-in `x-albert-*`
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
          <span className="field__label">Header name</span>
          <input
            type="text"
            value={newName}
            onChange={(event) => setNewName(event.target.value)}
            disabled={!running}
            spellCheck={false}
          />
        </label>
        <label className="field">
          <span className="field__label">Value</span>
          <input
            type="text"
            value={newValue}
            onChange={(event) => setNewValue(event.target.value)}
            disabled={!running}
            spellCheck={false}
          />
        </label>
      </div>
      <div className="row-actions">
        <button
          type="button"
          className="btn btn--ghost btn--sm"
          onClick={addRow}
          disabled={!running || !selectedRoute || !newName.trim()}
        >
          <Icon name="plus" size={12} />
          <span>Add / replace</span>
        </button>
      </div>

      {rows.length === 0 ? (
        <p className="hint">
          No response header overrides. Add rows like
          <code> x-ratelimit-remaining: 42</code> or
          <code> cache-control: no-store</code> to model provider
          behavior. Invalid header names are silently ignored at serve
          time rather than failing the response.
        </p>
      ) : (
        <ul className="rl-list">
          {rows.map((row, idx) => (
            <li key={idx} className="rl-list__row rl-list__row--headers">
              <code className="rl-list__key">{row.route}</code>
              <input
                type="text"
                value={row.name}
                onChange={(event) =>
                  updateRow(idx, { name: event.target.value })
                }
                disabled={!running}
                placeholder="header"
                aria-label={`Header name for ${row.route}`}
              />
              <input
                type="text"
                value={row.value}
                onChange={(event) =>
                  updateRow(idx, { value: event.target.value })
                }
                disabled={!running}
                placeholder="value"
                aria-label={`Header value for ${row.route}`}
              />
              <button
                type="button"
                className="btn btn--icon"
                onClick={() => removeRow(idx)}
                title="Remove header"
                aria-label={`Remove ${row.name} header for ${row.route}`}
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

export function flatten(
  value: Record<string, Record<string, string>>
): Array<{ route: string; name: string; value: string }> {
  const rows: Array<{ route: string; name: string; value: string }> = [];
  const routeKeys = Object.keys(value).sort();
  for (const route of routeKeys) {
    const headers = value[route];
    const names = Object.keys(headers).sort();
    for (const name of names) {
      rows.push({ route, name, value: headers[name] });
    }
  }
  return rows;
}

export function unflatten(
  rows: Array<{ route: string; name: string; value: string }>
): Record<string, Record<string, string>> {
  const out: Record<string, Record<string, string>> = {};
  for (const { route, name, value } of rows) {
    const routeKey = route.trim();
    const headerName = name.trim();
    if (!routeKey || !headerName) continue;
    if (!out[routeKey]) out[routeKey] = {};
    out[routeKey][headerName] = value;
  }
  return out;
}

function routeKeyOf(route: GatewayRouteSummary): string {
  return `${route.method} ${route.path}`;
}
