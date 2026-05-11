import { act, renderHook } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, test, vi } from "vitest";
import {
  describeImportDiff,
  useImportActions
} from "../useImportActions";
import type { CanonicalApiCollection } from "../../types";
import type { UseToasts } from "../useToasts";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

function makeToasts(): UseToasts {
  return {
    toasts: [],
    push: vi.fn(() => "toast"),
    dismiss: vi.fn(),
    info: vi.fn(() => "toast"),
    success: vi.fn(() => "toast"),
    warn: vi.fn(() => "toast"),
    error: vi.fn(() => "toast")
  };
}

function collection(): CanonicalApiCollection {
  return {
    id: "orders",
    name: "Orders",
    source: "openapi",
    description: null,
    endpoints: [
      {
        method: "GET",
        path: "/orders",
        summary: "List orders",
        description: null,
        tags: [],
        parameters: [],
        request_body: null,
        responses: [],
        examples: [],
        auth: null
      }
    ]
  };
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("describeImportDiff", () => {
  test("summarizes changed endpoint counts", () => {
    expect(
      describeImportDiff({
        added: [{ method: "POST", path: "/orders" }],
        changed: [{ method: "GET", path: "/orders/{id}" }],
        removed: [{ method: "DELETE", path: "/orders/{id}" }],
        unchanged: 2
      })
    ).toBe("1 added · 1 changed · 1 removed · 2 unchanged");
  });

  test("summarizes no-op imports", () => {
    expect(
      describeImportDiff({
        added: [],
        changed: [],
        removed: [],
        unchanged: 3
      })
    ).toBe("3 unchanged endpoint(s)");
  });
});

describe("useImportActions", () => {
  test("includes import diff summary in status and toast", async () => {
    const toasts = makeToasts();
    const setPreviewCollection = vi.fn();
    const setStatusMessage = vi.fn();
    const refreshStoredCollections = vi.fn().mockResolvedValue(undefined);
    const onImportComplete = vi.fn();
    const openTab = vi.fn();
    const onClose = vi.fn();
    vi.mocked(invoke)
      .mockResolvedValueOnce({
        collection_id: "orders",
        collection_name: "Orders",
        endpoint_count: 4,
        database_url: "/tmp/albert.sqlite",
        diff: {
          added: [{ method: "POST", path: "/orders" }],
          changed: [{ method: "GET", path: "/orders/{id}" }],
          removed: [],
          unchanged: 2
        }
      })
      .mockResolvedValueOnce(collection());

    const { result } = renderHook(() =>
      useImportActions({
        isTauriRuntime: true,
        toasts,
        setPreviewCollection,
        setStatusMessage,
        refreshStoredCollections,
        onImportComplete,
        openTab,
        onClose
      })
    );

    await act(async () => {
      await result.current.runImport("Orders", "{}");
    });

    expect(setStatusMessage).toHaveBeenCalledWith(
      "Imported 4 endpoint(s) into /tmp/albert.sqlite. 1 added · 1 changed · 2 unchanged."
    );
    expect(toasts.success).toHaveBeenCalledWith(
      'Imported 4 endpoint(s) as "Orders" · 1 added · 1 changed · 2 unchanged.'
    );
    expect(onImportComplete).toHaveBeenCalledWith(
      {
        collection_id: "orders",
        collection_name: "Orders",
        endpoint_count: 4,
        database_url: "/tmp/albert.sqlite",
        diff: {
          added: [{ method: "POST", path: "/orders" }],
          changed: [{ method: "GET", path: "/orders/{id}" }],
          removed: [],
          unchanged: 2
        }
      },
      collection()
    );
    expect(refreshStoredCollections).toHaveBeenCalledOnce();
    expect(openTab).toHaveBeenCalledWith(
      "orders",
      "Orders",
      collection().endpoints[0]
    );
    expect(onClose).toHaveBeenCalledOnce();
  });
});
