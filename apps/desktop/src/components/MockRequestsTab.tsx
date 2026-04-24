import { useMemo, useState } from "react";
import { downloadText, timestampSlug } from "../lib/downloadBlob";
import type { GatewayStatus, RequestLogEntry } from "../types";

type StatusFilter = "all" | "2xx" | "4xx" | "5xx";
const METHODS = ["ALL", "GET", "POST", "PUT", "PATCH", "DELETE"] as const;
type MethodFilter = (typeof METHODS)[number];

/**
 * Apply the (status-class, method) filter pair to a request log. Exported
 * so unit tests can pin the semantics; the component itself passes the
 * result straight into the render.
 */
export function filterRequests(
  log: RequestLogEntry[],
  status: StatusFilter,
  method: MethodFilter
): RequestLogEntry[] {
  return log.filter((entry) => {
    if (method !== "ALL" && entry.method.toUpperCase() !== method) return false;
    if (status === "all") return true;
    if (status === "2xx") return entry.status >= 200 && entry.status < 300;
    if (status === "4xx") return entry.status >= 400 && entry.status < 500;
    if (status === "5xx") return entry.status >= 500;
    return true;
  });
}

interface LogMetrics {
  total: number;
  status2xx: number;
  status4xx: number;
  status5xx: number;
  kindCounts: Record<string, number>;
  busiestRoute: { route: string; count: number } | null;
  averageLatencyMs: number;
  maxLatencyMs: number;
  /// Per-route breakdown (top-5 by hit count) with p50/p95 over the log.
  routeBreakdown: RouteBreakdownRow[];
}

export interface RouteBreakdownRow {
  route: string;
  count: number;
  p50: number;
  p95: number;
  max: number;
}

/**
 * Nearest-rank percentile over a pre-sorted array. Returns 0 for empty
 * arrays. `pct` is the percentile (1..=100).
 */
function percentile(sorted: number[], pct: number): number {
  if (sorted.length === 0) return 0;
  const idx = Math.max(0, Math.ceil((pct / 100) * sorted.length) - 1);
  return sorted[Math.min(idx, sorted.length - 1)];
}

export function computeMetrics(log: RequestLogEntry[]): LogMetrics {
  const routeCounts = new Map<string, number>();
  const routeLatencies = new Map<string, number[]>();
  const kindCounts: Record<string, number> = {};
  let total = 0;
  let status2xx = 0;
  let status4xx = 0;
  let status5xx = 0;
  let latencySum = 0;
  let latencyMax = 0;
  for (const entry of log) {
    total += 1;
    if (entry.status >= 200 && entry.status < 300) status2xx += 1;
    else if (entry.status >= 400 && entry.status < 500) status4xx += 1;
    else if (entry.status >= 500) status5xx += 1;

    const kindKey = entry.kind ?? entry.source ?? "unknown";
    kindCounts[kindKey] = (kindCounts[kindKey] ?? 0) + 1;

    const routeKey = entry.matched_route ?? `${entry.method} ${entry.path}`;
    routeCounts.set(routeKey, (routeCounts.get(routeKey) ?? 0) + 1);

    const ms = entry.latency_ms ?? 0;
    if (!routeLatencies.has(routeKey)) routeLatencies.set(routeKey, []);
    routeLatencies.get(routeKey)!.push(ms);

    latencySum += ms;
    if (ms > latencyMax) {
      latencyMax = ms;
    }
  }
  let busiestRoute: { route: string; count: number } | null = null;
  for (const [route, count] of routeCounts) {
    if (!busiestRoute || count > busiestRoute.count) {
      busiestRoute = { route, count };
    }
  }
  // Top-5 routes by hit count. Ties broken lexicographically so the list
  // is deterministic across reloads.
  const routeBreakdown: RouteBreakdownRow[] = Array.from(routeCounts.entries())
    .sort(
      ([a, ca], [b, cb]) => cb - ca || (a < b ? -1 : a > b ? 1 : 0)
    )
    .slice(0, 5)
    .map(([route, count]) => {
      const samples = (routeLatencies.get(route) ?? []).slice().sort(
        (x, y) => x - y
      );
      return {
        route,
        count,
        p50: percentile(samples, 50),
        p95: percentile(samples, 95),
        max: samples.length === 0 ? 0 : samples[samples.length - 1]
      };
    });
  return {
    total,
    status2xx,
    status4xx,
    status5xx,
    kindCounts,
    busiestRoute,
    averageLatencyMs: total === 0 ? 0 : Math.round(latencySum / total),
    maxLatencyMs: latencyMax,
    routeBreakdown
  };
}

function formatTime(ms: number): string {
  const d = new Date(ms);
  return d.toLocaleTimeString(undefined, { hour12: false });
}

interface MockRequestsTabProps {
  status: GatewayStatus;
  requests: RequestLogEntry[];
  baseUrl: string | null;
  onToggleCaptureBodies: (enabled: boolean) => Promise<void>;
  onClearLog?: () => Promise<void>;
  onReplayRequest?: (entry: RequestLogEntry) => void;
}

/**
 * Renders the Metrics + Recent-requests panels in the Mock Server drawer's
 * Requests tab. Kept in its own file so the parent panel stays focused on
 * layout + wiring.
 */
export function MockRequestsTab({
  status,
  requests,
  baseUrl,
  onToggleCaptureBodies,
  onClearLog,
  onReplayRequest
}: MockRequestsTabProps) {
  // Metrics always reflect the full log so the user keeps a global picture
  // even when the list itself is filtered.
  const metrics = useMemo(() => computeMetrics(requests), [requests]);
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const [methodFilter, setMethodFilter] = useState<MethodFilter>("ALL");
  const filtered = useMemo(
    () => filterRequests(requests, statusFilter, methodFilter),
    [requests, statusFilter, methodFilter]
  );
  const filterActive =
    statusFilter !== "all" || methodFilter !== "ALL";

  return (
    <>
      <section className="panel">
        <div className="panel__title panel__title--row">
          <h3>Metrics</h3>
          <span className="panel__meta">
            from the last {metrics.total} request(s)
          </span>
        </div>
        <div className="metrics-grid">
          <div className="metric">
            <span className="metric__label">Total</span>
            <span className="metric__value">{metrics.total}</span>
          </div>
          <div className="metric metric--ok">
            <span className="metric__label">2xx</span>
            <span className="metric__value">{metrics.status2xx}</span>
          </div>
          <div className="metric metric--warn">
            <span className="metric__label">4xx</span>
            <span className="metric__value">{metrics.status4xx}</span>
          </div>
          <div className="metric metric--err">
            <span className="metric__label">5xx</span>
            <span className="metric__value">{metrics.status5xx}</span>
          </div>
          <div className="metric">
            <span className="metric__label">avg ms</span>
            <span className="metric__value">{metrics.averageLatencyMs}</span>
          </div>
          <div className="metric">
            <span className="metric__label">max ms</span>
            <span className="metric__value">{metrics.maxLatencyMs}</span>
          </div>
        </div>
        {metrics.busiestRoute ? (
          <div className="metrics-row">
            <span className="metrics-row__label">Busiest:</span>
            <code>{metrics.busiestRoute.route}</code>
            <span className="metrics-row__tail">
              · {metrics.busiestRoute.count} hit
              {metrics.busiestRoute.count === 1 ? "" : "s"}
            </span>
          </div>
        ) : null}
        {metrics.routeBreakdown.length > 1 ? (
          <table className="route-breakdown">
            <thead>
              <tr>
                <th>Route</th>
                <th className="route-breakdown__num">hits</th>
                <th className="route-breakdown__num">p50</th>
                <th className="route-breakdown__num">p95</th>
                <th className="route-breakdown__num">max</th>
              </tr>
            </thead>
            <tbody>
              {metrics.routeBreakdown.map((row) => (
                <tr key={row.route}>
                  <td>
                    <code>{row.route}</code>
                  </td>
                  <td className="route-breakdown__num">{row.count}</td>
                  <td className="route-breakdown__num">{row.p50}ms</td>
                  <td className="route-breakdown__num">{row.p95}ms</td>
                  <td className="route-breakdown__num">{row.max}ms</td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : null}
      </section>

      <section className="panel">
        <div className="panel__title panel__title--row">
          <h3>Recent requests</h3>
          <label className="toggle">
            <input
              type="checkbox"
              checked={status.config.capture_bodies ?? false}
              onChange={(event) =>
                void onToggleCaptureBodies(event.target.checked)
              }
              disabled={!status.running}
            />
            <span>Capture request bodies</span>
          </label>
          <button
            type="button"
            className="btn btn--ghost btn--sm"
            onClick={() =>
              downloadText(
                `albert-request-log-${timestampSlug()}.json`,
                "application/json",
                JSON.stringify(requests, null, 2)
              )
            }
            disabled={requests.length === 0}
            title={
              requests.length === 0
                ? "No requests to export"
                : "Download the current log as JSON"
            }
          >
            Export JSON
          </button>
          {onClearLog ? (
            <button
              type="button"
              className="btn btn--ghost btn--sm"
              onClick={() => {
                if (requests.length === 0) return;
                void onClearLog();
              }}
              disabled={requests.length === 0 || !status.running}
              title={
                requests.length === 0
                  ? "Log is already empty"
                  : "Wipe the log + reset cumulative metrics"
              }
            >
              Clear
            </button>
          ) : null}
          <span className="panel__meta">
            {filterActive
              ? `showing ${filtered.length} of ${requests.length}`
              : `last ${requests.length} · refreshes every 3s`}
          </span>
        </div>
        <div className="reqlog-filters" role="toolbar" aria-label="Request filters">
          {(["all", "2xx", "4xx", "5xx"] as StatusFilter[]).map((key) => (
            <button
              key={key}
              type="button"
              className={
                statusFilter === key
                  ? "chip chip--active"
                  : "chip"
              }
              onClick={() => setStatusFilter(key)}
              aria-pressed={statusFilter === key}
            >
              {key}
            </button>
          ))}
          <label className="reqlog-filters__method">
            <span className="reqlog-filters__label">Method</span>
            <select
              className="select"
              value={methodFilter}
              onChange={(event) =>
                setMethodFilter(event.target.value as MethodFilter)
              }
            >
              {METHODS.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
          </label>
          {filterActive ? (
            <button
              type="button"
              className="chip chip--ghost"
              onClick={() => {
                setStatusFilter("all");
                setMethodFilter("ALL");
              }}
            >
              clear
            </button>
          ) : null}
        </div>
        {requests.length === 0 ? (
          <div className="empty">
            No requests captured yet. Try{" "}
            <code>curl {baseUrl ?? "http://..."}</code> to hit a route.
          </div>
        ) : filtered.length === 0 ? (
          <div className="empty">
            No requests match the current filter.
          </div>
        ) : (
          <ul className="reqlog">
            {filtered.map((entry, idx) => (
              <li
                key={`${entry.at_epoch_ms}-${idx}-${entry.path}`}
                className={
                  onReplayRequest && entry.matched_route
                    ? "reqlog__item reqlog__item--clickable"
                    : "reqlog__item"
                }
                onClick={() => {
                  if (onReplayRequest && entry.matched_route) {
                    onReplayRequest(entry);
                  }
                }}
                onKeyDown={(event) => {
                  if (
                    onReplayRequest &&
                    entry.matched_route &&
                    (event.key === "Enter" || event.key === " ")
                  ) {
                    event.preventDefault();
                    onReplayRequest(entry);
                  }
                }}
                role={
                  onReplayRequest && entry.matched_route ? "button" : undefined
                }
                tabIndex={
                  onReplayRequest && entry.matched_route ? 0 : undefined
                }
                title={
                  onReplayRequest && entry.matched_route
                    ? "Click to replay in Try-it"
                    : undefined
                }
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
                {entry.latency_ms > 0 ? (
                  <span className="reqlog__latency">
                    {entry.latency_ms}ms
                  </span>
                ) : (
                  <span className="reqlog__latency reqlog__latency--zero">
                    —
                  </span>
                )}
                {entry.kind ? (
                  <span className={`kind-chip kind-chip--${entry.kind}`}>
                    {entry.kind}
                  </span>
                ) : (
                  <span
                    className={`kind-chip kind-chip--source-${entry.source.replace(/[^a-z0-9]/gi, "-")}`}
                  >
                    {entry.source}
                  </span>
                )}
                {entry.request_id ? (
                  <button
                    type="button"
                    className="reqlog__reqid"
                    title="Copy request id"
                    onClick={(e) => {
                      e.stopPropagation();
                      void navigator.clipboard?.writeText(entry.request_id ?? "");
                    }}
                  >
                    id:{(entry.request_id ?? "").slice(0, 8)}
                  </button>
                ) : null}
                {entry.request_body ? (
                  <details className="reqlog__body">
                    <summary>body</summary>
                    <pre className="code-block code-block--wrap">
                      {entry.request_body}
                    </pre>
                  </details>
                ) : null}
              </li>
            ))}
          </ul>
        )}
      </section>
    </>
  );
}
