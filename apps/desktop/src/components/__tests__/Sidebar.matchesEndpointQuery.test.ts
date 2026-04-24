import { describe, expect, test } from "vitest";
import { matchesEndpointQuery } from "../Sidebar";

function ep(
  overrides: Partial<{
    method: string;
    path: string;
    summary: string | null;
    operation_id: string | null;
  }> = {}
) {
  return {
    method: "GET",
    path: "/users",
    summary: null,
    operation_id: null,
    ...overrides
  };
}

describe("matchesEndpointQuery", () => {
  test("empty query matches everything", () => {
    expect(matchesEndpointQuery(ep(), "")).toBe(true);
    expect(matchesEndpointQuery(ep(), "   ")).toBe(true);
  });

  test("case-insensitive substring match on path", () => {
    expect(matchesEndpointQuery(ep({ path: "/Users/me" }), "users")).toBe(true);
    expect(matchesEndpointQuery(ep({ path: "/orders" }), "users")).toBe(false);
  });

  test("matches against summary and operation_id", () => {
    expect(
      matchesEndpointQuery(ep({ summary: "List users" }), "list")
    ).toBe(true);
    expect(
      matchesEndpointQuery(ep({ operation_id: "listUsers" }), "listusers")
    ).toBe(true);
  });

  test("single method token matches by method name (substring)", () => {
    expect(matchesEndpointQuery(ep({ method: "POST" }), "pos")).toBe(true);
    expect(matchesEndpointQuery(ep({ method: "POST" }), "get")).toBe(false);
  });

  test("'get /users' narrows to GET endpoints whose path mentions users", () => {
    expect(
      matchesEndpointQuery(ep({ method: "GET", path: "/users" }), "get /users")
    ).toBe(true);
    // POST /users — method mismatch
    expect(
      matchesEndpointQuery(ep({ method: "POST", path: "/users" }), "get /users")
    ).toBe(false);
    // GET /orders — path mismatch
    expect(
      matchesEndpointQuery(ep({ method: "GET", path: "/orders" }), "get /users")
    ).toBe(false);
  });

  test("'post admin' also checks summary / operation_id for the second token", () => {
    expect(
      matchesEndpointQuery(
        ep({
          method: "POST",
          path: "/users",
          summary: "Create admin user"
        }),
        "post admin"
      )
    ).toBe(true);
  });

  test("two-token query where first token is NOT a verb falls back to substring match on full query", () => {
    // "foo bar" — first token is not an HTTP method; treat as-is.
    // The literal substring "foo bar" is not in any field, so no match.
    expect(
      matchesEndpointQuery(
        ep({ method: "GET", path: "/users", summary: "foo and bar" }),
        "foo bar"
      )
    ).toBe(false);
  });

  test("method-only tokens like 'delete' match DELETE endpoints", () => {
    expect(matchesEndpointQuery(ep({ method: "DELETE" }), "delete")).toBe(true);
    expect(matchesEndpointQuery(ep({ method: "DELETE" }), "del")).toBe(true);
  });
});
