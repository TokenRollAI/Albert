import { act, renderHook } from "@testing-library/react";
import { describe, expect, test } from "vitest";
import { useEndpointTabs } from "../useEndpointTabs";
import type { CanonicalApiCollection, CanonicalEndpoint } from "../../types";

function endpoint(path: string, method = "GET"): CanonicalEndpoint {
  return {
    operation_id: null,
    method,
    path,
    summary: null,
    description: null,
    tags: [],
    parameters: [],
    request_body: null,
    responses: [],
    examples: [
      { kind: "success", title: "s" },
      { kind: "empty", title: "e" },
      { kind: "error", title: "x" }
    ]
  };
}

function collection(
  id: string,
  endpoints: CanonicalEndpoint[]
): CanonicalApiCollection {
  return {
    id,
    name: `col-${id}`,
    source: "openapi",
    description: null,
    endpoints
  };
}

describe("useEndpointTabs", () => {
  test("opens a tab + persists it to localStorage", () => {
    const col = collection("c1", [endpoint("/users")]);
    const { result } = renderHook(() => useEndpointTabs());
    act(() => {
      result.current.openTab(col.id, col.name, col.endpoints[0]);
    });
    expect(result.current.tabs).toHaveLength(1);
    expect(window.localStorage.getItem("albert.tabs.v1")).toContain("/users");
  });

  test("restoreTabs reopens persisted tabs", () => {
    const col = collection("c1", [endpoint("/users"), endpoint("/orders")]);

    // First hook instance opens the tabs.
    const first = renderHook(() => useEndpointTabs());
    act(() => {
      first.result.current.openTab(col.id, col.name, col.endpoints[0]);
      first.result.current.openTab(col.id, col.name, col.endpoints[1]);
    });
    expect(first.result.current.tabs).toHaveLength(2);

    // Second instance (simulating app restart) sees no tabs until
    // restoreTabs is called.
    const second = renderHook(() => useEndpointTabs());
    expect(second.result.current.tabs).toHaveLength(0);
    act(() => {
      second.result.current.restoreTabs([col]);
    });
    expect(second.result.current.tabs).toHaveLength(2);
    expect(second.result.current.activeId).not.toBeNull();
  });

  test("restoreTabs skips tabs whose endpoint no longer exists", () => {
    const col = collection("c1", [endpoint("/users"), endpoint("/orders")]);
    const first = renderHook(() => useEndpointTabs());
    act(() => {
      first.result.current.openTab(col.id, col.name, col.endpoints[0]);
      first.result.current.openTab(col.id, col.name, col.endpoints[1]);
    });

    // Reopen with a collection that lost /orders.
    const reduced = collection("c1", [endpoint("/users")]);
    const second = renderHook(() => useEndpointTabs());
    act(() => {
      second.result.current.restoreTabs([reduced]);
    });
    expect(second.result.current.tabs).toHaveLength(1);
    expect(second.result.current.tabs[0].path).toBe("/users");
  });

  test("restoreTabs is idempotent when tabs already exist", () => {
    const col = collection("c1", [endpoint("/users")]);
    const { result } = renderHook(() => useEndpointTabs());
    act(() => {
      result.current.openTab(col.id, col.name, col.endpoints[0]);
    });
    const before = result.current.tabs;
    act(() => {
      // Another collection snapshot arrives; no tabs should be added.
      result.current.restoreTabs([collection("c1", [endpoint("/other")])]);
    });
    expect(result.current.tabs).toBe(before);
  });
});
