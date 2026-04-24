import { describe, expect, test } from "vitest";
import { countMethods } from "../Sidebar";
import type { CanonicalEndpoint } from "../../types";

function endpoint(method: string, path = "/x"): CanonicalEndpoint {
  return {
    method,
    path,
    tags: [],
    parameters: [],
    responses: [],
    examples: [],
    auth: null
  };
}

describe("countMethods", () => {
  test("empty list returns empty array", () => {
    expect(countMethods([])).toEqual([]);
  });

  test("counts per method", () => {
    const result = countMethods([
      endpoint("GET"),
      endpoint("GET"),
      endpoint("POST")
    ]);
    expect(result).toEqual([
      { method: "GET", count: 2 },
      { method: "POST", count: 1 }
    ]);
  });

  test("keeps the canonical GET→HEAD ordering regardless of input order", () => {
    const result = countMethods([
      endpoint("DELETE"),
      endpoint("POST"),
      endpoint("GET"),
      endpoint("PATCH")
    ]);
    expect(result.map((r) => r.method)).toEqual([
      "GET",
      "POST",
      "PATCH",
      "DELETE"
    ]);
  });

  test("unknown methods trail after canonical ones, sorted alphabetically", () => {
    const result = countMethods([
      endpoint("LINK"),
      endpoint("GET"),
      endpoint("CONNECT")
    ]);
    expect(result.map((r) => r.method)).toEqual(["GET", "CONNECT", "LINK"]);
  });

  test("is case-insensitive on the method name", () => {
    const result = countMethods([endpoint("get"), endpoint("Get"), endpoint("GET")]);
    expect(result).toEqual([{ method: "GET", count: 3 }]);
  });
});
