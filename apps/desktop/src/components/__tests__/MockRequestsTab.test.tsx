import { describe, expect, test } from "vitest";
import { computeMetrics, filterRequests } from "../MockRequestsTab";
import type { RequestLogEntry } from "../../types";

function entry(overrides: Partial<RequestLogEntry> = {}): RequestLogEntry {
  return {
    at_epoch_ms: 0,
    method: "GET",
    path: "/x",
    query: null,
    matched_route: null,
    collection_name: null,
    status: 200,
    kind: null,
    source: "default",
    latency_ms: 0,
    request_body: null,
    ...overrides
  };
}

describe("computeMetrics", () => {
  test("bucket status classes and counts totals", () => {
    const metrics = computeMetrics([
      entry({ status: 200 }),
      entry({ status: 204 }),
      entry({ status: 404 }),
      entry({ status: 500 }),
      entry({ status: 502 })
    ]);
    expect(metrics.total).toBe(5);
    expect(metrics.status2xx).toBe(2);
    expect(metrics.status4xx).toBe(1);
    expect(metrics.status5xx).toBe(2);
  });

  test("computes average and max latency", () => {
    const metrics = computeMetrics([
      entry({ latency_ms: 10 }),
      entry({ latency_ms: 30 }),
      entry({ latency_ms: 200 })
    ]);
    // (10 + 30 + 200) / 3 = 80
    expect(metrics.averageLatencyMs).toBe(80);
    expect(metrics.maxLatencyMs).toBe(200);
  });

  test("picks the busiest route by hit count", () => {
    const metrics = computeMetrics([
      entry({ matched_route: "GET /a" }),
      entry({ matched_route: "GET /a" }),
      entry({ matched_route: "GET /b" }),
      entry({ matched_route: "GET /a" })
    ]);
    expect(metrics.busiestRoute).toEqual({ route: "GET /a", count: 3 });
  });

  test("empty log returns zeros, not NaN", () => {
    const metrics = computeMetrics([]);
    expect(metrics.total).toBe(0);
    expect(metrics.averageLatencyMs).toBe(0);
    expect(metrics.maxLatencyMs).toBe(0);
    expect(metrics.busiestRoute).toBe(null);
  });

  test("falls back to METHOD path when matched_route is null", () => {
    const metrics = computeMetrics([
      entry({ method: "POST", path: "/lost" }),
      entry({ method: "POST", path: "/lost" })
    ]);
    expect(metrics.busiestRoute).toEqual({ route: "POST /lost", count: 2 });
  });

  test("routeBreakdown carries per-route hit count + p50/p95", () => {
    const metrics = computeMetrics([
      entry({ matched_route: "GET /a", latency_ms: 10 }),
      entry({ matched_route: "GET /a", latency_ms: 30 }),
      entry({ matched_route: "GET /a", latency_ms: 50 }),
      entry({ matched_route: "GET /a", latency_ms: 20 }),
      entry({ matched_route: "GET /a", latency_ms: 40 }),
      entry({ matched_route: "POST /b", latency_ms: 5 })
    ]);
    expect(metrics.routeBreakdown.map((r) => r.route)).toEqual([
      "GET /a",
      "POST /b"
    ]);
    const a = metrics.routeBreakdown[0];
    expect(a.count).toBe(5);
    // Sorted [10, 20, 30, 40, 50] → p50=30, p95=50.
    expect(a.p50).toBe(30);
    expect(a.p95).toBe(50);
    expect(a.max).toBe(50);
  });

  test("routeBreakdown caps at 5 entries with ties broken lexicographically", () => {
    const log = [];
    // 6 distinct routes, all 1 hit each. Sorted by name: a, b, c, d, e, f.
    // The breakdown should drop "f" (alphabetically last).
    for (const name of ["f", "d", "c", "b", "e", "a"]) {
      log.push(entry({ matched_route: `GET /${name}` }));
    }
    const metrics = computeMetrics(log);
    expect(metrics.routeBreakdown.map((r) => r.route)).toEqual([
      "GET /a",
      "GET /b",
      "GET /c",
      "GET /d",
      "GET /e"
    ]);
  });
});

describe("filterRequests", () => {
  const log: RequestLogEntry[] = [
    entry({ status: 200, method: "GET", path: "/a" }),
    entry({ status: 429, method: "POST", path: "/b" }),
    entry({ status: 500, method: "POST", path: "/c" }),
    entry({ status: 204, method: "DELETE", path: "/d" })
  ];

  test("status=all + method=ALL returns everything", () => {
    expect(filterRequests(log, "all", "ALL")).toHaveLength(4);
  });

  test("2xx filter keeps only 2xx responses", () => {
    const out = filterRequests(log, "2xx", "ALL");
    expect(out).toHaveLength(2);
    expect(out.every((e) => e.status >= 200 && e.status < 300)).toBe(true);
  });

  test("method filter narrows by HTTP verb", () => {
    const out = filterRequests(log, "all", "POST");
    expect(out).toHaveLength(2);
    expect(out.every((e) => e.method === "POST")).toBe(true);
  });

  test("combining status + method filters intersects", () => {
    const out = filterRequests(log, "5xx", "POST");
    expect(out).toEqual([log[2]]);
  });

  test("empty filter set returns empty array (not the full log)", () => {
    const out = filterRequests(log, "5xx", "GET");
    expect(out).toHaveLength(0);
  });
});
