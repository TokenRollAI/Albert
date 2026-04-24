import { beforeEach, describe, expect, test } from "vitest";
import { act, renderHook } from "@testing-library/react";
import {
  hasDraftContent,
  seedTryItDraft,
  useDirtyRoutes
} from "../useTryItDraft";

beforeEach(() => {
  window.localStorage.clear();
});

describe("hasDraftContent", () => {
  test("reports false for an untouched route", () => {
    expect(hasDraftContent("GET /x")).toBe(false);
  });

  test("reports true after seeding any field", () => {
    seedTryItDraft("GET /x", { query: "q=1" });
    expect(hasDraftContent("GET /x")).toBe(true);
  });

  test("treats all-blank fields as empty", () => {
    // Store a draft that has structure but no actual content.
    window.localStorage.setItem(
      "albert.tryit.1:GET /x",
      JSON.stringify({
        params: { id: "  " },
        query: "",
        body: "\n",
        headers: [{ key: "", value: "noise" }]
      })
    );
    expect(hasDraftContent("GET /x")).toBe(false);
  });
});

describe("useDirtyRoutes", () => {
  test("initial render marks routes with content dirty", () => {
    seedTryItDraft("POST /orders", { body: '{"x":1}' });
    const { result } = renderHook(() =>
      useDirtyRoutes(["GET /users", "POST /orders"])
    );
    expect(result.current.has("POST /orders")).toBe(true);
    expect(result.current.has("GET /users")).toBe(false);
  });

  test("reacts to a seed event on a tracked route", () => {
    const { result } = renderHook(() =>
      useDirtyRoutes(["GET /users"])
    );
    expect(result.current.has("GET /users")).toBe(false);
    act(() => {
      seedTryItDraft("GET /users", { query: "limit=10" });
    });
    expect(result.current.has("GET /users")).toBe(true);
  });

  test("ignores events for untracked routes", () => {
    const { result } = renderHook(() =>
      useDirtyRoutes(["GET /users"])
    );
    act(() => {
      seedTryItDraft("POST /orders", { body: '{"noise":true}' });
    });
    expect(result.current.has("POST /orders")).toBe(false);
    expect(result.current.has("GET /users")).toBe(false);
  });
});
