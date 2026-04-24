import { describe, expect, test } from "vitest";
import { buildCurlFromLogEntry } from "../UrlBar";
import type { RequestLogEntry } from "../../types";

function entry(overrides: Partial<RequestLogEntry> = {}): RequestLogEntry {
  return {
    at_epoch_ms: 0,
    method: "GET",
    path: "/users",
    query: null,
    matched_route: null,
    collection_name: null,
    status: 200,
    kind: null,
    source: "default",
    latency_ms: 0,
    request_id: null,
    request_body: null,
    ...overrides
  };
}

describe("buildCurlFromLogEntry", () => {
  test("GET with no body, no headers, no base URL → falls back to api.example.com", () => {
    const cmd = buildCurlFromLogEntry(entry({ path: "/users" }), null);
    expect(cmd).toContain('curl -X GET');
    expect(cmd).toContain('"https://api.example.com/users"');
    expect(cmd).not.toContain("-d");
    expect(cmd).not.toContain("x-request-id");
  });

  test("trailing slash in base URL is trimmed before concatenating the path", () => {
    const cmd = buildCurlFromLogEntry(
      entry({ path: "/users" }),
      "http://localhost:4317/"
    );
    expect(cmd).toContain('"http://localhost:4317/users"');
  });

  test("query string appended with leading ? even when missing in the stored form", () => {
    const cmdBare = buildCurlFromLogEntry(
      entry({ path: "/users", query: "name=jane" }),
      "http://localhost:4317"
    );
    expect(cmdBare).toContain('"http://localhost:4317/users?name=jane"');
    const cmdPrefixed = buildCurlFromLogEntry(
      entry({ path: "/users", query: "?name=jane" }),
      "http://localhost:4317"
    );
    expect(cmdPrefixed).toContain('"http://localhost:4317/users?name=jane"');
  });

  test("request_id becomes an x-request-id header so replay preserves the trace key", () => {
    const cmd = buildCurlFromLogEntry(
      entry({ request_id: "abc-123" }),
      null
    );
    expect(cmd).toContain('-H "x-request-id: abc-123"');
  });

  test("captured body becomes a -d flag with JSON content-type", () => {
    const cmd = buildCurlFromLogEntry(
      entry({
        method: "POST",
        path: "/posts",
        request_body: '{"title":"hi"}'
      }),
      "http://localhost:4317"
    );
    expect(cmd).toContain('-H "Content-Type: application/json"');
    expect(cmd).toContain(`-d '{"title":"hi"}'`);
  });

  test("truncation sentinel is stripped before embedding the body", () => {
    const cmd = buildCurlFromLogEntry(
      entry({
        method: "POST",
        path: "/posts",
        request_body: '{"long":true}…[truncated]'
      }),
      null
    );
    expect(cmd).toContain(`-d '{"long":true}'`);
    expect(cmd).not.toContain("truncated");
  });

  test("single quotes inside the body are escaped via the '\\'' trick", () => {
    const cmd = buildCurlFromLogEntry(
      entry({
        method: "POST",
        path: "/x",
        request_body: `{"msg":"don't"}`
      }),
      null
    );
    expect(cmd).toContain(`-d '{"msg":"don'\\''t"}'`);
  });

  test("<capture failed: …> sentinel is preserved (not copied as a body)", () => {
    const cmd = buildCurlFromLogEntry(
      entry({
        method: "POST",
        path: "/x",
        request_body: "<capture failed: io error>"
      }),
      null
    );
    expect(cmd).not.toContain("-d");
    expect(cmd).not.toContain("capture failed");
  });
});
