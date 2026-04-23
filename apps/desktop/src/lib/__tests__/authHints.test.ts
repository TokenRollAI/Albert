import { describe, expect, test } from "vitest";
import {
  authHintToRequiredHeaders,
  seedRequiredHeadersFromEndpoints
} from "../authHints";
import type { CanonicalEndpoint } from "../../types";

describe("authHintToRequiredHeaders", () => {
  test("bearer → Authorization with Bearer prefix", () => {
    expect(
      authHintToRequiredHeaders({
        scheme: "http_bearer",
        header_name: "Authorization",
        value_prefix: "Bearer ",
        description: null
      })
    ).toEqual([
      { name: "Authorization", value_prefix: "Bearer ", value_equals: null }
    ]);
  });

  test("api_key_header → named header without prefix", () => {
    expect(
      authHintToRequiredHeaders({
        scheme: "api_key_header",
        header_name: "X-Api-Key",
        value_prefix: null,
        description: null
      })
    ).toEqual([
      { name: "X-Api-Key", value_prefix: null, value_equals: null }
    ]);
  });

  test("oauth2 normalizes to a Bearer header gate", () => {
    expect(
      authHintToRequiredHeaders({
        scheme: "oauth2",
        header_name: "Authorization",
        value_prefix: "Bearer ",
        description: null
      })
    ).toEqual([
      { name: "Authorization", value_prefix: "Bearer ", value_equals: null }
    ]);
  });

  test("other schemes return no rules", () => {
    expect(
      authHintToRequiredHeaders({
        scheme: "other",
        header_name: "Authorization",
        value_prefix: null,
        description: "mTLS"
      })
    ).toEqual([]);
  });

  test("null hint returns no rules", () => {
    expect(authHintToRequiredHeaders(null)).toEqual([]);
    expect(authHintToRequiredHeaders(undefined)).toEqual([]);
  });
});

describe("seedRequiredHeadersFromEndpoints", () => {
  function endpoint(
    method: string,
    path: string,
    auth: CanonicalEndpoint["auth"] = null
  ): CanonicalEndpoint {
    return {
      method,
      path,
      tags: [],
      parameters: [],
      responses: [],
      examples: [],
      auth: auth ?? null
    };
  }

  test("builds METHOD /path → rules map", () => {
    const result = seedRequiredHeadersFromEndpoints([
      endpoint("GET", "/secret", {
        scheme: "http_bearer",
        header_name: "Authorization",
        value_prefix: "Bearer ",
        description: null
      }),
      endpoint("GET", "/public"),
      endpoint("POST", "/orders", {
        scheme: "api_key_header",
        header_name: "X-Api-Key",
        value_prefix: null,
        description: null
      })
    ]);
    expect(Object.keys(result).sort()).toEqual([
      "GET /secret",
      "POST /orders"
    ]);
    expect(result["GET /secret"][0].value_prefix).toBe("Bearer ");
    expect(result["POST /orders"][0].name).toBe("X-Api-Key");
  });

  test("drops non-seedable schemes", () => {
    const result = seedRequiredHeadersFromEndpoints([
      endpoint("GET", "/mtls", {
        scheme: "other",
        header_name: "X",
        value_prefix: null,
        description: null
      })
    ]);
    expect(result).toEqual({});
  });
});
