import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import {
  workspaceCollectionMeta,
  WorkspacePanel
} from "../WorkspacePanel";
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
    endpoints: [endpoint("GET", "/orders"), endpoint("POST", "/orders")],
    createdAt: "1700000000",
    updatedAt: "1700003600",
    endpointCount: 2,
    ...overrides
  };
}

describe("WorkspacePanel", () => {
  test("renders nothing when closed", () => {
    const { container } = render(
      <WorkspacePanel
        open={false}
        collections={[collection()]}
        connected={true}
        onClose={() => {}}
        onImportClick={() => {}}
        onOpenEndpoint={() => {}}
        onRefresh={() => {}}
      />
    );
    expect(container.textContent).toBe("");
  });

  test("formats collection metadata for workspace cards", () => {
    expect(workspaceCollectionMeta(collection())).toContain("2 endpoints");
    expect(workspaceCollectionMeta(collection({ updatedAt: undefined }))).toMatch(
      /^Imported /
    );
  });

  test("shows imported collection summary and actions", () => {
    const onOpenEndpoint = vi.fn();
    const onRefresh = vi.fn();
    const onRename = vi.fn();
    const onExport = vi.fn();
    const onDelete = vi.fn();
    render(
      <WorkspacePanel
        open={true}
        collections={[collection()]}
        connected={true}
        onClose={() => {}}
        onImportClick={() => {}}
        onOpenEndpoint={onOpenEndpoint}
        onRefresh={onRefresh}
        onRenameCollection={onRename}
        onExportCollection={onExport}
        onDeleteCollection={onDelete}
      />
    );

    expect(screen.getByRole("dialog", { name: "Workspace collections" })).toBeTruthy();
    expect(screen.getByText("Orders")).toBeTruthy();
    expect(screen.getByText("2")).toBeTruthy();
    expect(screen.getByText("GET 1")).toBeTruthy();
    expect(screen.getByText("POST 1")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    expect(onRefresh).toHaveBeenCalledTimes(1);

    const card = screen.getByText("Orders").closest("article");
    expect(card).toBeTruthy();
    fireEvent.click(within(card as HTMLElement).getByRole("button", { name: "Open" }));
    expect(onOpenEndpoint).toHaveBeenCalledTimes(1);
    expect(onOpenEndpoint.mock.calls[0][0].id).toBe("orders");
    expect(onOpenEndpoint.mock.calls[0][1].path).toBe("/orders");

    fireEvent.click(screen.getByRole("button", { name: "Rename Orders" }));
    fireEvent.click(screen.getByRole("button", { name: "Export Orders" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete Orders" }));
    expect(onRename).toHaveBeenCalledTimes(1);
    expect(onExport).toHaveBeenCalledTimes(1);
    expect(onDelete).toHaveBeenCalledTimes(1);
  });

  test("fallback mode shows preview source and disables refresh", () => {
    render(
      <WorkspacePanel
        open={true}
        collections={[collection({ origin: "fallback", endpointCount: undefined })]}
        connected={false}
        onClose={() => {}}
        onImportClick={() => {}}
        onOpenEndpoint={() => {}}
        onRefresh={() => {}}
      />
    );

    expect(screen.getByText("Preview")).toBeTruthy();
    expect(
      (screen.getByRole("button", { name: "Refresh" }) as HTMLButtonElement)
        .disabled
    ).toBe(true);
    expect(screen.queryByRole("button", { name: "Rename Orders" })).toBeNull();
  });

  test("empty state can open import", () => {
    const onImport = vi.fn();
    render(
      <WorkspacePanel
        open={true}
        collections={[]}
        connected={true}
        onClose={() => {}}
        onImportClick={onImport}
        onOpenEndpoint={() => {}}
        onRefresh={() => {}}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "Import source" }));
    expect(onImport).toHaveBeenCalledTimes(1);
  });
});
