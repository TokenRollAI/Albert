import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import {
  collectionMetaLabel,
  formatCollectionTimestamp,
  Sidebar
} from "../Sidebar";
import type { CanonicalEndpoint, SidebarCollection } from "../../types";

function endpoint(method: string, path: string): CanonicalEndpoint {
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

function collection(
  overrides: Partial<SidebarCollection> = {}
): SidebarCollection {
  return {
    id: "orders",
    name: "Orders",
    origin: "imported",
    source: "openapi",
    endpoints: [endpoint("GET", "/orders")],
    createdAt: "1700000000",
    updatedAt: "1700003600",
    endpointCount: 1,
    ...overrides
  };
}

describe("Sidebar collection metadata", () => {
  test("formats unix-second timestamps for display", () => {
    expect(formatCollectionTimestamp("1700000000")).toMatch(/\d/);
    expect(formatCollectionTimestamp("not-a-date")).toBeNull();
  });

  test("labels imported collections with update time and endpoint count", () => {
    const label = collectionMetaLabel(collection({ endpointCount: 2 }));
    expect(label).toMatch(/^Updated /);
    expect(label).toContain("2 endpoints");
  });

  test("does not label preview or fallback collections", () => {
    expect(collectionMetaLabel(collection({ origin: "fallback" }))).toBeNull();
    expect(collectionMetaLabel(collection({ origin: "preview" }))).toBeNull();
  });

  test("renders imported metadata in the collection row", () => {
    render(
      <Sidebar
        collections={[collection()]}
        activeTabId={null}
        onOpenEndpoint={vi.fn()}
        onImportClick={vi.fn()}
        onRefresh={vi.fn()}
        busy={false}
      />
    );

    expect(screen.getByText(/Updated /)).toBeTruthy();
    expect(screen.getByText(/1 endpoint/)).toBeTruthy();
  });

  test("fallback collection rows omit import metadata", () => {
    render(
      <Sidebar
        collections={[
          collection({
            origin: "fallback",
            createdAt: undefined,
            updatedAt: undefined,
            endpointCount: undefined
          })
        ]}
        activeTabId={null}
        onOpenEndpoint={vi.fn()}
        onImportClick={vi.fn()}
        onRefresh={vi.fn()}
        busy={false}
      />
    );

    expect(screen.queryByText(/Updated /)).toBeNull();
    expect(screen.queryByText(/Imported /)).toBeNull();
  });

  test("opened collection keeps endpoint navigation usable", () => {
    const onOpenEndpoint = vi.fn();
    render(
      <Sidebar
        collections={[collection()]}
        activeTabId={null}
        onOpenEndpoint={onOpenEndpoint}
        onImportClick={vi.fn()}
        onRefresh={vi.fn()}
        busy={false}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: /GET/i }));
    expect(onOpenEndpoint).toHaveBeenCalledTimes(1);
  });
});
