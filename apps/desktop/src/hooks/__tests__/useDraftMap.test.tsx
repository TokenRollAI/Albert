import { describe, expect, test, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useDraftMap } from "../useDraftMap";

describe("useDraftMap", () => {
  test("starts with the passed-in current value and is not dirty", () => {
    const { result } = renderHook(() =>
      useDraftMap<Record<string, number>>({ a: 1 }, vi.fn())
    );
    expect(result.current.draft).toEqual({ a: 1 });
    expect(result.current.dirty).toBe(false);
    expect(result.current.busy).toBe(false);
  });

  test("editing the draft flips dirty", () => {
    const { result } = renderHook(() =>
      useDraftMap<Record<string, number>>({ a: 1 }, vi.fn())
    );
    act(() => result.current.setDraft({ a: 1, b: 2 }));
    expect(result.current.dirty).toBe(true);
  });

  test("reset restores the server value and clears dirty", () => {
    const { result } = renderHook(() =>
      useDraftMap<Record<string, number>>({ a: 1 }, vi.fn())
    );
    act(() => result.current.setDraft({ a: 9 }));
    expect(result.current.dirty).toBe(true);
    act(() => result.current.reset());
    expect(result.current.draft).toEqual({ a: 1 });
    expect(result.current.dirty).toBe(false);
  });

  test("apply flips busy around the onApply promise", async () => {
    let resolve: (() => void) | null = null;
    const onApply = vi.fn().mockImplementation(
      () =>
        new Promise<void>((r) => {
          resolve = r;
        })
    );
    const { result } = renderHook(() =>
      useDraftMap<Record<string, number>>({ a: 1 }, onApply)
    );
    act(() => result.current.setDraft({ a: 2 }));
    let pending: Promise<void>;
    act(() => {
      pending = result.current.apply();
    });
    expect(result.current.busy).toBe(true);
    await act(async () => {
      resolve!();
      await pending;
    });
    expect(result.current.busy).toBe(false);
    expect(onApply).toHaveBeenCalledWith({ a: 2 });
  });

  test("dirty is value-based so a reassignment to the same object is clean", () => {
    // Same reference + same contents ⇒ not dirty, regardless of whether
    // setDraft was called. Insertion order matters for the JSON-based
    // comparison, which is fine for our Record<string, …> shapes that
    // are constructed by cloning rather than by key re-ordering.
    const initial = { a: 1, b: 2 };
    const { result } = renderHook(() =>
      useDraftMap<Record<string, number>>(initial, vi.fn())
    );
    act(() => result.current.setDraft({ ...initial }));
    expect(result.current.dirty).toBe(false);
  });
});
