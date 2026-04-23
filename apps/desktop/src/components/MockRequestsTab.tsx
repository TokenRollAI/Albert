import { useMemo } from "react";
import type { GatewayStatus, RequestLogEntry } from "../types";

interface LogMetrics {
  total: number;
  status2xx: number;
  status4xx: number;
  status5xx: number;
  kindCounts: Record<string, number>;
  busiestRoute: { route: string; count: number } | null;
  averageLatencyMs: number;
  maxLatencyMs: number;
}

export function computeMetrics(log: RequestLogEntry[]): LogMetrics {
  const routeCounts = new Map<string, number>();
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

    latencySum += entry.latency_ms ?? 0;
    if ((entry.latency_ms ?? 0) > latencyMax) {
      latencyMax = entry.latency_ms ?? 0;
    }
  }
  let busiestRoute: { route: string; count: number } | null = null;
  for (const [route, count] of routeCounts) {
    if (!busiestRoute || count > busiestRoute.count) {
      busiestRoute = { route, count };
    }
  }
  return {
    total,
    status2xx,
    status4xx,
    status5xx,
    kindCounts,
    busiestRoute,
    averageLatencyMs: total === 0 ? 0 : Math.round(latencySum / total),
    maxLatencyMs: latencyMax
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
  onReplayRequest
}: MockRequestsTabProps) {
  const metrics = useMemo(() => computeMetrics(requests), [requests]);

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
          <span className="panel__meta">
            last {requests.length} · refreshes every 3s
          </span>
        </div>
        {requests.length === 0 ? (
          <div className="empty">
            No requests captured yet. Try{" "}
            <code>curl {baseUrl ?? "http://..."}</code> to hit a route.
          </div>
        ) : (
          <ul className="reqlog">
            {requests.map((entry, idx) => (
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
