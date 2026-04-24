import { useMemo, useState } from "react";
import { downloadText, timestampSlug } from "../lib/downloadBlob";
import { buildCurlFromLogEntry } from "./UrlBar";
import type { GatewayStatus, RequestLogEntry } from "../types";

type StatusFilter = "all" | "2xx" | "4xx" | "5xx";
const METHODS = ["ALL", "GET", "POST", "PUT", "PATCH", "DELETE"] as const;
type MethodFilter = (typeof METHODS)[number];

/**
 * Serialize a request log into an Excel-compatible CSV string. Columns
 * are in stable order so downstream spreadsheets can rely on them; the
 * first row is a header. Non-string columns (numbers, nulls, empty)
 * are rendered as expected, and strings containing commas / quotes /
 * newlines are wrapped in double quotes with embedded quotes doubled
 * per RFC 4180.
 */
export function toCsvRows(log: RequestLogEntry[]): string {
  const headers = [
    "timestamp_ms",
    "method",
    "path",
    "matched_route",
    "status",
    "kind",
    "source",
    "latency_ms",
    "collection_name",
    "request_id",
    "query"
  ];
  const rows = [headers.join(",")];
  for (const entry of log) {
    rows.push(
      [
        entry.at_epoch_ms,
        entry.method,
        entry.path,
        entry.matched_route ?? "",
        entry.status,
        entry.kind ?? "",
        entry.source ?? "",
        entry.latency_ms,
        entry.collection_name ?? "",
        entry.request_id ?? "",
        entry.query ?? ""
      ]
        .map(csvCell)
        .join(",")
    );
  }
  return rows.join("\n");
}

function csvCell(value: string | number | null | undefined): string {
  if (value === null || value === undefined) return "";
  const raw = String(value);
  const needsQuoting =
    raw.includes(",") ||
    raw.includes("\"") ||
    raw.includes("\n") ||
    raw.includes("\r");
  if (!needsQuoting) return raw;
  return `"${raw.replace(/"/g, '""')}"`;
}

/**
 * Apply the (status-class, method, free-text) filter trio to a request
 * log. Exported so unit tests can pin the semantics; the component
 * itself passes the result straight into the render. The free-text
 * search matches case-insensitively against path / matched_route / the
 * numeric status / the request_id, so users can paste an id from
 * their client logs and find the matching row.
 */
export function filterRequests(
  log: RequestLogEntry[],
  status: StatusFilter,
  method: MethodFilter,
  search: string = ""
): RequestLogEntry[] {
  const query = search.trim().toLowerCase();
  return log.filter((entry) => {
    if (method !== "ALL" && entry.method.toUpperCase() !== method) return false;
    if (status === "2xx" && !(entry.status >= 200 && entry.status < 300))
      return false;
    if (status === "4xx" && !(entry.status >= 400 && entry.status < 500))
      return false;
    if (status === "5xx" && !(entry.status >= 500)) return false;
    if (query) {
      const haystack = [
        entry.path,
        entry.matched_route ?? "",
        String(entry.status),
        entry.request_id ?? "",
        entry.query ?? ""
      ]
        .join(" ")
        .toLowerCase();
      if (!haystack.includes(query)) return false;
    }
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

export interface SparkBucket {
  minuteEpochMs: number;
  count: number;
  status5xx: number;
}

/**
 * Bucket the request log by wall-clock minute for a sparkline. The
 * newest minute comes last so the chart naturally reads left-to-right.
 * Capped at 15 minutes to fit comfortably in the Requests tab without
 * making tiny bars. Empty minutes between observed activity are
 * back-filled with zero buckets so the chart spacing reflects elapsed
 * time, not just "last N hit minutes".
 */
export function computeSparkline(
  log: RequestLogEntry[],
  window = 15
): SparkBucket[] {
  if (log.length === 0) return [];
  const minuteOf = (ms: number): number => Math.floor(ms / 60_000) * 60_000;
  const raw = new Map<number, { count: number; status5xx: number }>();
  let latestMinute = -Infinity;
  for (const entry of log) {
    const minute = minuteOf(entry.at_epoch_ms);
    if (minute > latestMinute) latestMinute = minute;
    const existing = raw.get(minute) ?? { count: 0, status5xx: 0 };
    existing.count += 1;
    if (entry.status >= 500 && entry.status < 600) existing.status5xx += 1;
    raw.set(minute, existing);
  }
  if (!Number.isFinite(latestMinute)) return [];
  const out: SparkBucket[] = [];
  for (let offset = window - 1; offset >= 0; offset--) {
    const minute = latestMinute - offset * 60_000;
    const hit = raw.get(minute);
    out.push({
      minuteEpochMs: minute,
      count: hit?.count ?? 0,
      status5xx: hit?.status5xx ?? 0
    });
  }
  return out;
}

/**
 * Return a prettified view of a captured request body if it parses as
 * JSON, otherwise the raw string unchanged. Keeps the "<capture failed:
 * …>" sentinel visible by short-circuiting on it so the failure reason
 * isn't accidentally parsed away.
 */
export function prettifyRequestBody(raw: string): string {
  if (!raw) return raw;
  if (raw.startsWith("<capture failed:")) return raw;
  // The gateway appends `…[truncated]` when a body crosses the 4KB cap.
  // Don't try to parse that — strip it, parse the prefix, then re-append.
  const sentinel = "…[truncated]";
  const hasSentinel = raw.endsWith(sentinel);
  const body = hasSentinel ? raw.slice(0, raw.length - sentinel.length) : raw;
  try {
    const parsed = JSON.parse(body);
    const pretty = JSON.stringify(parsed, null, 2);
    return hasSentinel ? `${pretty}\n${sentinel}` : pretty;
  } catch {
    return raw;
  }
}

/**
 * Tiny per-minute bar chart over the request log. Each bar's height is
 * proportional to the peak count in the visible window; a 5xx share is
 * overlaid in an error-tinted segment at the bar's base. No axes, no
 * legend — this is a glanceable "is traffic arriving" indicator, not a
 * full dashboard.
 */
function Sparkline({ buckets }: { buckets: SparkBucket[] }) {
  const peak = Math.max(1, ...buckets.map((b) => b.count));
  return (
    <div
      className="sparkline"
      role="img"
      aria-label={`Request rate over the last ${buckets.length} minutes; peak ${peak}/min`}
    >
      {buckets.map((bucket, idx) => {
        const height = Math.round((bucket.count / peak) * 100);
        const errHeight =
          bucket.count === 0
            ? 0
            : Math.round((bucket.status5xx / bucket.count) * height);
        const minute = new Date(bucket.minuteEpochMs).toLocaleTimeString(
          undefined,
          { hour12: false, minute: "2-digit", hour: "2-digit" }
        );
        const title = `${minute} — ${bucket.count} req${
          bucket.count === 1 ? "" : "s"
        }${bucket.status5xx ? ` (${bucket.status5xx} 5xx)` : ""}`;
        return (
          <span
            key={`${bucket.minuteEpochMs}-${idx}`}
            className="sparkline__bar"
            style={{ height: `${Math.max(height, 2)}%` }}
            title={title}
          >
            {errHeight > 0 ? (
              <span
                className="sparkline__bar-err"
                style={{ height: `${errHeight}%` }}
              />
            ) : null}
          </span>
        );
      })}
    </div>
  );
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
  const sparkline = useMemo(() => computeSparkline(requests, 15), [requests]);
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const [methodFilter, setMethodFilter] = useState<MethodFilter>("ALL");
  const [searchText, setSearchText] = useState("");
  const filtered = useMemo(
    () => filterRequests(requests, statusFilter, methodFilter, searchText),
    [requests, statusFilter, methodFilter, searchText]
  );
  const filterActive =
    statusFilter !== "all" || methodFilter !== "ALL" || searchText.trim() !== "";

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
        {sparkline.length > 0 ? (
          <Sparkline buckets={sparkline} />
        ) : null}
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
          <button
            type="button"
            className="btn btn--ghost btn--sm"
            onClick={() =>
              downloadText(
                `albert-request-log-${timestampSlug()}.csv`,
                "text/csv;charset=utf-8",
                toCsvRows(requests)
              )
            }
            disabled={requests.length === 0}
            title={
              requests.length === 0
                ? "No requests to export"
                : "Download the current log as CSV (spreadsheet-ready)"
            }
          >
            Export CSV
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
          <input
            type="search"
            className="reqlog-filters__search"
            placeholder="search path / id / status"
            value={searchText}
            onChange={(event) => setSearchText(event.target.value)}
            aria-label="Search requests"
            spellCheck={false}
          />
          {filterActive ? (
            <button
              type="button"
              className="chip chip--ghost"
              onClick={() => {
                setStatusFilter("all");
                setMethodFilter("ALL");
                setSearchText("");
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
                <button
                  type="button"
                  className="reqlog__curl"
                  title="Copy as cURL (uses the gateway's live URL; replays the original body if captured)"
                  onClick={(e) => {
                    e.stopPropagation();
                    void navigator.clipboard?.writeText(
                      buildCurlFromLogEntry(entry, baseUrl)
                    );
                  }}
                >
                  cURL
                </button>
                {entry.request_body ? (
                  <details className="reqlog__body">
                    <summary>body</summary>
                    <pre className="code-block code-block--wrap">
                      {prettifyRequestBody(entry.request_body)}
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
