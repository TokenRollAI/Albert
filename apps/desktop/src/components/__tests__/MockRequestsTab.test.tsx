import { describe, expect, test } from "vitest";
import {
  computeMetrics,
  filterRequests,
  prettifyRequestBody,
  toCsvRows
} from "../MockRequestsTab";
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

  test("search matches against path", () => {
    const out = filterRequests(log, "all", "ALL", "/b");
    expect(out.map((e) => e.path)).toEqual(["/b"]);
  });

  test("search matches against status as a string", () => {
    const out = filterRequests(log, "all", "ALL", "429");
    expect(out.map((e) => e.status)).toEqual([429]);
  });

  test("search matches request_id when provided", () => {
    const out = filterRequests(
      [
        entry({ path: "/x", request_id: "trace-abc-123" }),
        entry({ path: "/y", request_id: "other-xyz" })
      ],
      "all",
      "ALL",
      "abc-123"
    );
    expect(out).toHaveLength(1);
    expect(out[0].path).toBe("/x");
  });

  test("search is case-insensitive and ignores surrounding whitespace", () => {
    const out = filterRequests(
      [entry({ path: "/Users/Me" })],
      "all",
      "ALL",
      "  users  "
    );
    expect(out).toHaveLength(1);
  });

  test("search composes with status + method filters", () => {
    const out = filterRequests(
      [
        entry({ method: "POST", status: 500, path: "/a" }),
        entry({ method: "POST", status: 500, path: "/b" })
      ],
      "5xx",
      "POST",
      "/b"
    );
    expect(out).toHaveLength(1);
    expect(out[0].path).toBe("/b");
  });
});

describe("prettifyRequestBody", () => {
  test("valid compact JSON becomes 2-space indented", () => {
    const out = prettifyRequestBody('{"a":1,"b":[2,3]}');
    expect(out).toBe('{\n  "a": 1,\n  "b": [\n    2,\n    3\n  ]\n}');
  });

  test("non-JSON text passes through unchanged", () => {
    expect(prettifyRequestBody("plain text body")).toBe("plain text body");
  });

  test("preserves the truncation sentinel on oversized bodies", () => {
    const out = prettifyRequestBody('{"a":1}…[truncated]');
    expect(out).toBe('{\n  "a": 1\n}\n…[truncated]');
  });

  test("passes the capture-failed sentinel through unchanged", () => {
    expect(
      prettifyRequestBody("<capture failed: some io error>")
    ).toBe("<capture failed: some io error>");
  });

  test("empty input returns empty string", () => {
    expect(prettifyRequestBody("")).toBe("");
  });

  test("malformed JSON that starts with { still falls back cleanly", () => {
    const raw = '{"bad: true';
    expect(prettifyRequestBody(raw)).toBe(raw);
  });
});

describe("toCsvRows", () => {
  test("empty log still emits the header row", () => {
    const csv = toCsvRows([]);
    expect(csv).toBe(
      "timestamp_ms,method,path,matched_route,status,kind,source,latency_ms,collection_name,request_id,query"
    );
  });

  test("numeric + string columns are stringified with a stable order", () => {
    const csv = toCsvRows([
      entry({
        at_epoch_ms: 1700000000,
        method: "GET",
        path: "/users",
        matched_route: "GET /users",
        status: 200,
        kind: "success",
        source: "default",
        latency_ms: 42,
        collection_name: "api",
        request_id: "abc-123",
        query: "q=1"
      })
    ]);
    const [_, row] = csv.split("\n");
    expect(row).toBe(
      "1700000000,GET,/users,GET /users,200,success,default,42,api,abc-123,q=1"
    );
  });

  test("RFC 4180: fields with commas / quotes / newlines are quoted", () => {
    const csv = toCsvRows([
      entry({
        path: "/users/1",
        query: "name=Doe, John",
        request_id: 'he said "hi"',
        matched_route: "line1\nline2"
      })
    ]);
    // Don't split on \n here — the embedded newline is intentional. Check
    // the raw string for each quoted token instead.
    expect(csv).toContain('"name=Doe, John"');
    expect(csv).toContain('"he said ""hi"""');
    expect(csv).toContain('"line1\nline2"');
  });

  test("null / undefined optional fields render as empty strings", () => {
    const csv = toCsvRows([
      entry({
        matched_route: null,
        kind: null,
        collection_name: null,
        request_id: undefined,
        query: null
      })
    ]);
    // fields 4 (matched_route), 6 (kind), 9 (collection_name), 10 (request_id), 11 (query).
    const [_, row] = csv.split("\n");
    const cells = row.split(",");
    expect(cells[3]).toBe("");
    expect(cells[5]).toBe("");
    expect(cells[8]).toBe("");
    expect(cells[9]).toBe("");
    expect(cells[10]).toBe("");
  });
});
