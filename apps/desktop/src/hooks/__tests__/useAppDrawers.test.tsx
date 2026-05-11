import { describe, expect, test } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useAppDrawers } from "../useAppDrawers";

describe("useAppDrawers", () => {
  test("every slot starts closed", () => {
    const { result } = renderHook(() => useAppDrawers());
    expect(result.current.workspace.open).toBe(false);
    expect(result.current.importReport.open).toBe(false);
    expect(result.current.import.open).toBe(false);
    expect(result.current.mockServer.open).toBe(false);
    expect(result.current.providers.open).toBe(false);
    expect(result.current.shortcuts.open).toBe(false);
  });

  test("open$ / close / toggle update only the targeted slot", () => {
    const { result } = renderHook(() => useAppDrawers());
    act(() => result.current.workspace.open$());
    expect(result.current.workspace.open).toBe(true);
    expect(result.current.providers.open).toBe(false);

    act(() => result.current.workspace.close());
    expect(result.current.workspace.open).toBe(false);

    act(() => result.current.providers.toggle());
    expect(result.current.providers.open).toBe(true);
    act(() => result.current.providers.toggle());
    expect(result.current.providers.open).toBe(false);
  });

  test("set accepts an explicit boolean and can be passed as a prop", () => {
    const { result } = renderHook(() => useAppDrawers());
    act(() => result.current.mockServer.set(true));
    expect(result.current.mockServer.open).toBe(true);
    act(() => result.current.mockServer.set(false));
    expect(result.current.mockServer.open).toBe(false);
  });
});
