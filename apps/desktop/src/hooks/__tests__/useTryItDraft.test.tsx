import { act, renderHook } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { seedTryItDraft, useTryItDraft } from "../useTryItDraft";

describe("useTryItDraft", () => {
  test("persists updates to localStorage", () => {
    const { result } = renderHook(() => useTryItDraft("GET /users"));
    act(() => {
      result.current.updateQuery("status=active");
      result.current.updateBody('{"name":"Ada"}');
      result.current.updateHeaders([{ key: "X-Trace", value: "abc" }]);
    });

    // Remount with the same key — the hook should rehydrate from storage
    // rather than start from an empty draft.
    const { result: remount } = renderHook(() => useTryItDraft("GET /users"));
    expect(remount.current.draft.query).toBe("status=active");
    expect(remount.current.draft.body).toBe('{"name":"Ada"}');
    expect(remount.current.draft.headers[0].key).toBe("X-Trace");
  });

  test("isolates drafts across different routes", () => {
    const { result: a } = renderHook(() => useTryItDraft("GET /a"));
    const { result: b } = renderHook(() => useTryItDraft("GET /b"));
    act(() => {
      a.current.updateQuery("for=a");
      b.current.updateQuery("for=b");
    });

    const { result: aRemount } = renderHook(() => useTryItDraft("GET /a"));
    const { result: bRemount } = renderHook(() => useTryItDraft("GET /b"));
    expect(aRemount.current.draft.query).toBe("for=a");
    expect(bRemount.current.draft.query).toBe("for=b");
  });

  test("reset wipes the stored draft", () => {
    const { result } = renderHook(() => useTryItDraft("GET /wipe"));
    act(() => {
      result.current.updateQuery("first=true");
    });
    act(() => {
      result.current.reset();
    });

    const { result: remount } = renderHook(() => useTryItDraft("GET /wipe"));
    expect(remount.current.draft.query).toBe("");
  });

  test("seedTryItDraft applies a partial update and reaches mounted hooks", () => {
    const { result } = renderHook(() => useTryItDraft("POST /seed"));

    act(() => {
      seedTryItDraft("POST /seed", { query: "via=seed", body: "hi" });
    });

    expect(result.current.draft.query).toBe("via=seed");
    expect(result.current.draft.body).toBe("hi");
  });
});
