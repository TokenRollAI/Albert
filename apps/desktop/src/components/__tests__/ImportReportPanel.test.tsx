import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { ImportReportPanel, type ImportReport } from "../ImportReportPanel";
import type { CanonicalApiCollection, CanonicalEndpoint } from "../../types";

function endpoint(method: string, path: string, summary?: string): CanonicalEndpoint {
  return {
    method,
    path,
    summary,
    tags: [],
    parameters: [],
    responses: [],
    examples: [],
    auth: null
  };
}

function collection(): CanonicalApiCollection {
  return {
    id: "orders",
    name: "Orders",
    source: "openapi",
    description: null,
    endpoints: [
      endpoint("GET", "/orders", "List orders"),
      endpoint("GET", "/orders/{id}", "Fetch order"),
      endpoint("POST", "/orders", "Create order")
    ]
  };
}

function report(): ImportReport {
  return {
    collection: collection(),
    result: {
      collection_id: "orders",
      collection_name: "Orders",
      endpoint_count: 3,
      database_url: "/tmp/albert.sqlite",
      diff: {
        added: [{ method: "POST", path: "/orders", summary: "Create order" }],
        changed: [
          {
            method: "GET",
            path: "/orders/{id}",
            summary: "Fetch order",
            reasons: ["metadata changed", "responses changed"],
            details: [
              "summary changed: Get order -> Fetch order",
              "response changed: 200 (schema)"
            ]
          }
        ],
        removed: [
          { method: "DELETE", path: "/orders/{id}", summary: "Delete order" }
        ],
        unchanged: 1
      }
    }
  };
}

function reportWithTwoChangedEndpoints(): ImportReport {
  const base = report();
  base.result.diff.changed = [
    ...base.result.diff.changed,
    {
      method: "GET",
      path: "/orders",
      summary: "List orders",
      reasons: ["parameters changed"],
      details: ["parameter added: query status"]
    }
  ];
  return base;
}

describe("ImportReportPanel", () => {
  test("renders nothing when closed", () => {
    const { container } = render(
      <ImportReportPanel
        open={false}
        report={report()}
        onClose={() => {}}
        onImportClick={() => {}}
        onOpenEndpoint={() => {}}
      />
    );

    expect(container.textContent).toBe("");
  });

  test("shows grouped diff counts and endpoint rows", () => {
    render(
      <ImportReportPanel
        open={true}
        report={report()}
        onClose={() => {}}
        onImportClick={() => {}}
        onOpenEndpoint={() => {}}
      />
    );

    expect(screen.getByRole("dialog", { name: "Import report" })).toBeTruthy();
    expect(screen.getByText("1 added · 1 changed · 1 removed · 1 unchanged")).toBeTruthy();
    expect(within(screen.getByLabelText("Added")).getByText("/orders")).toBeTruthy();
    expect(
      within(screen.getByLabelText("Changed")).getByText("/orders/{id}")
    ).toBeTruthy();
    expect(
      within(screen.getByLabelText("Changed")).getByText("metadata changed")
    ).toBeTruthy();
    expect(
      within(screen.getByLabelText("Changed")).getByText("responses changed")
    ).toBeTruthy();
    expect(
      within(screen.getByLabelText("Changed")).getByText(
        "summary changed: Get order -> Fetch order"
      )
    ).toBeTruthy();
    expect(
      within(screen.getByLabelText("Changed")).getByText(
        "response changed: 200 (schema)"
      )
    ).toBeTruthy();
    expect(within(screen.getByLabelText("Removed")).getByText("Delete order")).toBeTruthy();
  });

  test("opens and previews only endpoints that still exist after import", () => {
    const onOpenEndpoint = vi.fn();
    const onPreviewEndpointPrompt = vi.fn();
    const onRefreshEndpointMock = vi.fn();
    render(
      <ImportReportPanel
        open={true}
        report={report()}
        onClose={() => {}}
        onImportClick={() => {}}
        onOpenEndpoint={onOpenEndpoint}
        onPreviewEndpointPrompt={onPreviewEndpointPrompt}
        onRefreshEndpointMock={onRefreshEndpointMock}
      />
    );

    const added = screen.getByLabelText("Added");
    fireEvent.click(within(added).getByRole("button", { name: "Open" }));
    expect(onOpenEndpoint).toHaveBeenCalledTimes(1);
    expect(onOpenEndpoint.mock.calls[0][0].id).toBe("orders");
    expect(onOpenEndpoint.mock.calls[0][1].method).toBe("POST");
    expect(onOpenEndpoint.mock.calls[0][1].path).toBe("/orders");
    fireEvent.click(within(added).getByRole("button", { name: "Prompt" }));
    expect(onPreviewEndpointPrompt).toHaveBeenCalledTimes(1);
    expect(onPreviewEndpointPrompt.mock.calls[0][0].id).toBe("orders");
    expect(onPreviewEndpointPrompt.mock.calls[0][1].method).toBe("POST");
    expect(onPreviewEndpointPrompt.mock.calls[0][2]).toMatchObject({
      method: "POST",
      path: "/orders"
    });
    expect(within(added).queryByRole("button", { name: "Refresh" })).toBeNull();

    const changed = screen.getByLabelText("Changed");
    fireEvent.click(within(changed).getByRole("button", { name: "Prompt" }));
    expect(onPreviewEndpointPrompt).toHaveBeenCalledTimes(2);
    expect(onPreviewEndpointPrompt.mock.calls[1][2]).toMatchObject({
      method: "GET",
      path: "/orders/{id}",
      reasons: ["metadata changed", "responses changed"],
      details: [
        "summary changed: Get order -> Fetch order",
        "response changed: 200 (schema)"
      ]
    });
    fireEvent.click(within(changed).getByRole("button", { name: "Refresh" }));
    expect(onRefreshEndpointMock).toHaveBeenCalledTimes(1);
    expect(onRefreshEndpointMock.mock.calls[0][0].id).toBe("orders");
    expect(onRefreshEndpointMock.mock.calls[0][1].method).toBe("GET");
    expect(onRefreshEndpointMock.mock.calls[0][2]).toMatchObject({
      path: "/orders/{id}",
      reasons: ["metadata changed", "responses changed"],
      details: [
        "summary changed: Get order -> Fetch order",
        "response changed: 200 (schema)"
      ]
    });

    const removed = screen.getByLabelText("Removed");
    expect(within(removed).queryByRole("button", { name: "Open" })).toBeNull();
    expect(within(removed).queryByRole("button", { name: "Prompt" })).toBeNull();
    expect(within(removed).queryByRole("button", { name: "Refresh" })).toBeNull();
  });

  test("batch refreshes refreshable changed endpoints", () => {
    const onRefreshChangedMocks = vi.fn();
    render(
      <ImportReportPanel
        open={true}
        report={reportWithTwoChangedEndpoints()}
        onClose={() => {}}
        onImportClick={() => {}}
        onOpenEndpoint={() => {}}
        onRefreshChangedMocks={onRefreshChangedMocks}
      />
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Refresh changed (2)" })
    );

    expect(onRefreshChangedMocks).toHaveBeenCalledTimes(1);
    expect(onRefreshChangedMocks.mock.calls[0][0].id).toBe("orders");
    expect(onRefreshChangedMocks.mock.calls[0][1]).toHaveLength(2);
    expect(onRefreshChangedMocks.mock.calls[0][1][0]).toMatchObject({
      endpoint: { method: "GET", path: "/orders/{id}" },
      change: {
        path: "/orders/{id}",
        details: [
          "summary changed: Get order -> Fetch order",
          "response changed: 200 (schema)"
        ]
      }
    });
    expect(onRefreshChangedMocks.mock.calls[0][1][1]).toMatchObject({
      endpoint: { method: "GET", path: "/orders" },
      change: {
        path: "/orders",
        details: ["parameter added: query status"]
      }
    });
  });

  test("empty report state can open import", () => {
    const onImport = vi.fn();
    render(
      <ImportReportPanel
        open={true}
        report={null}
        onClose={() => {}}
        onImportClick={onImport}
        onOpenEndpoint={() => {}}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: "Import source" }));
    expect(onImport).toHaveBeenCalledTimes(1);
  });

  test("no-change report shows no endpoint changes", () => {
    const noChange = report();
    noChange.result.diff = {
      added: [],
      changed: [],
      removed: [],
      unchanged: 3
    };
    render(
      <ImportReportPanel
        open={true}
        report={noChange}
        onClose={() => {}}
        onImportClick={() => {}}
        onOpenEndpoint={() => {}}
      />
    );

    expect(screen.getByText("no changes")).toBeTruthy();
    expect(screen.getByText("No endpoint changes")).toBeTruthy();
  });
});
