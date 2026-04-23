import { describe, expect, test } from "vitest";
import { computeMetrics } from "../MockRequestsTab";
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
});
