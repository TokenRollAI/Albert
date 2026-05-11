import { act, renderHook } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, test, vi } from "vitest";
import { useAiActions } from "../useAiActions";
import type { EndpointTab, MockExample, ProviderConfigDraft } from "../../types";
import type { UseToasts } from "../useToasts";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

const providerDraft: ProviderConfigDraft = {
  provider_name: "openai",
  base_url: "https://api.openai.com",
  model: "gpt-4o-mini",
  api_key_env: "OPENAI_API_KEY"
};

const tab: EndpointTab = {
  id: "tab-1",
  collectionId: "col",
  collectionName: "Demo",
  method: "GET",
  path: "/orders",
  inspector: "params",
  example: "success",
  endpoint: {
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
};

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

function renderActions() {
  const toasts = makeToasts();
  const refreshStoredCollections = vi.fn().mockResolvedValue(undefined);
  const updateEndpointExample = vi.fn();
  const setStatusMessage = vi.fn();
  const promptPreviewSetters = {
    setPreview: vi.fn(),
    setOpen: vi.fn(),
    setLoading: vi.fn(),
    setError: vi.fn()
  };
  const hook = renderHook(() =>
    useAiActions({
      isTauriRuntime: true,
      providerDraft,
      apiKeyOverride: "session-key",
      toasts,
      setStatusMessage,
      refreshStoredCollections,
      updateEndpointExample,
      promptPreviewSetters
    })
  );
  return { ...hook, refreshStoredCollections, updateEndpointExample };
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

describe("useAiActions", () => {
  test("passes optional generation context through generate_mock_example", async () => {
    const example: MockExample = {
      kind: "success",
      title: "AI Success",
      payload: { ok: true },
      note: "Generated"
    };
    vi.mocked(invoke).mockResolvedValue(example);
    const { result } = renderActions();

    await act(async () => {
      await result.current.generateWithContext(tab, "success", true, {
        request_snapshot: { query: "status=paid" },
        response_snapshot: { status: 200, body: { ok: true } },
        note: "cache abc"
      });
    });

    expect(invoke).toHaveBeenCalledWith("generate_mock_example", {
      request: {
        endpoint: tab.endpoint,
        intent: "success",
        provider: providerDraft,
        collection_id: "col",
        persist: true,
        database_url: null,
        api_key_override: "session-key",
        generation_context: {
          request_snapshot: { query: "status=paid" },
          response_snapshot: { status: 200, body: { ok: true } },
          note: "cache abc"
        }
      }
    });
  });

  test("passes optional generation context through prompt preview", async () => {
    vi.mocked(invoke).mockResolvedValue({
      system: "system",
      user: "user",
      endpoint_context: {}
    });
    const { result } = renderActions();

    await act(async () => {
      await result.current.previewPrompt(tab, "error", {
        request_snapshot: { query: "status=failed" },
        response_snapshot: { status: 500, body: { error: "upstream" } },
        note: "cache xyz"
      });
    });

    expect(invoke).toHaveBeenCalledWith("preview_generation_prompt", {
      endpoint: tab.endpoint,
      intent: "error",
      generation_context: {
        request_snapshot: { query: "status=failed" },
        response_snapshot: { status: 500, body: { error: "upstream" } },
        note: "cache xyz"
      }
    });
  });

  test("passes per-kind generation contexts through generate all", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce({
        kind: "success",
        title: "AI Success",
        payload: { ok: true },
        note: "Generated"
      } satisfies MockExample)
      .mockResolvedValueOnce({
        kind: "empty",
        title: "AI Empty",
        payload: [],
        note: "Generated"
      } satisfies MockExample)
      .mockResolvedValueOnce({
        kind: "error",
        title: "AI Error",
        payload: { error: "bad" },
        note: "Generated"
      } satisfies MockExample);
    const { result } = renderActions();

    await act(async () => {
      await result.current.generateAllWithContexts(tab, true, {
        success: {
          response_snapshot: {
            kind: "success",
            body: { previous: "success" }
          },
          note: "current success"
        },
        empty: {
          response_snapshot: {
            kind: "empty",
            body: []
          },
          note: "current empty"
        },
        error: {
          response_snapshot: {
            kind: "error",
            body: { previous: "error" }
          },
          note: "current error"
        }
      });
    });

    expect(invoke).toHaveBeenNthCalledWith(1, "generate_mock_example", {
      request: {
        endpoint: tab.endpoint,
        intent: "success",
        provider: providerDraft,
        collection_id: "col",
        persist: true,
        database_url: null,
        api_key_override: "session-key",
        generation_context: {
          response_snapshot: {
            kind: "success",
            body: { previous: "success" }
          },
          note: "current success"
        }
      }
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "generate_mock_example", {
      request: {
        endpoint: tab.endpoint,
        intent: "empty",
        provider: providerDraft,
        collection_id: "col",
        persist: true,
        database_url: null,
        api_key_override: "session-key",
        generation_context: {
          response_snapshot: {
            kind: "empty",
            body: []
          },
          note: "current empty"
        }
      }
    });
    expect(invoke).toHaveBeenNthCalledWith(3, "generate_mock_example", {
      request: {
        endpoint: tab.endpoint,
        intent: "error",
        provider: providerDraft,
        collection_id: "col",
        persist: true,
        database_url: null,
        api_key_override: "session-key",
        generation_context: {
          response_snapshot: {
            kind: "error",
            body: { previous: "error" }
          },
          note: "current error"
        }
      }
    });
  });
});
