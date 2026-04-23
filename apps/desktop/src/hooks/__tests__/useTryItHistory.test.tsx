import { act, renderHook } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { useTryItHistory } from "../useTryItHistory";

describe("useTryItHistory", () => {
  test("records entries at the front and caps at 5", () => {
    const { result } = renderHook(() => useTryItHistory("GET /users"));
    act(() => {
      for (let i = 0; i < 7; i += 1) {
        result.current.record({
          status: 200,
          elapsedMs: i * 10,
          method: "GET",
          url: `http://x/users?n=${i}`
        });
      }
    });
    expect(result.current.history).toHaveLength(5);
    // Most recent first
    expect(result.current.history[0].url).toContain("n=6");
    expect(result.current.history[4].url).toContain("n=2");
  });

  test("rehydrates from localStorage on remount", () => {
    const { result } = renderHook(() => useTryItHistory("POST /orders"));
    act(() => {
      result.current.record({
        status: 201,
        elapsedMs: 42,
        method: "POST",
        url: "http://x/orders"
      });
    });
    const { result: remount } = renderHook(() =>
      useTryItHistory("POST /orders")
    );
    expect(remount.current.history).toHaveLength(1);
    expect(remount.current.history[0].status).toBe(201);
  });

  test("clear removes both in-memory and persisted state", () => {
    const { result } = renderHook(() => useTryItHistory("DELETE /thing/1"));
    act(() => {
      result.current.record({
        status: 204,
        elapsedMs: 10,
        method: "DELETE",
        url: "http://x/thing/1"
      });
    });
    act(() => {
      result.current.clear();
    });
    const { result: remount } = renderHook(() =>
      useTryItHistory("DELETE /thing/1")
    );
    expect(remount.current.history).toHaveLength(0);
  });
});
