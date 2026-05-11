import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { ResponsePane } from "../ResponsePane";
import type {
  EndpointTab,
  MockExample,
  ProviderConfigDraft
} from "../../types";

const provider: ProviderConfigDraft = {
  provider_name: "openai",
  base_url: "https://api.openai.com",
  model: "gpt-4o-mini",
  api_key_env: "OPENAI_API_KEY"
};

function makeTab(example?: Partial<MockExample>): EndpointTab {
  return {
    id: "tab-1",
    collectionId: "fallback:demo",
    collectionName: "Demo",
    method: "GET",
    path: "/api/orders",
    inspector: "params",
    example: "success",
    endpoint: {
      method: "GET",
      path: "/api/orders",
      summary: "List orders",
      description: null,
      tags: [],
      parameters: [],
      request_body: null,
      responses: [
        {
          status_code: "200",
          description: "OK",
          content_type: "application/json",
          schema: null
        }
      ],
      examples: [
        {
          kind: "success",
          title: "Success",
          payload: { data: [{ id: "ord_1" }] },
          note: "Fallback preview",
          ...example
        },
        {
          kind: "empty",
          title: "Empty",
          payload: [],
          note: "No orders"
        },
        {
          kind: "error",
          title: "Error",
          payload: { error: "unauthorized" },
          note: "Auth error"
        }
      ],
      auth: null
    }
  };
}

function renderPane(options: { connected: boolean; apiKeyOverride?: string }) {
  return render(
    <ResponsePane
      tab={makeTab()}
      onSelectExample={vi.fn()}
      connected={options.connected}
      provider={provider}
      apiKeyOverride={options.apiKeyOverride ?? ""}
      onGenerate={vi.fn()}
    />
  );
}

describe("ResponsePane", () => {
  test("does not show API key warning when Tauri runtime is unavailable", () => {
    renderPane({ connected: false });
    expect(screen.queryByText(/No API key entered/i)).toBeNull();
  });

  test("shows API key warning only when runtime is connected and no key is present", () => {
    renderPane({ connected: true });
    expect(screen.getByText(/No API key entered/i)).not.toBeNull();
  });

  test("renders persisted mock payload instead of the placeholder fallback", () => {
    const { container } = renderPane({ connected: false });
    expect(container.textContent).toContain("ord_1");
    expect(container.textContent).not.toContain("No sample payload persisted yet");
  });

  test("passes the current mock payload as context when generating", async () => {
    const onGenerate = vi.fn().mockResolvedValue(null);
    const onGenerateWithContext = vi.fn().mockResolvedValue(null);
    render(
      <ResponsePane
        tab={makeTab()}
        onSelectExample={vi.fn()}
        connected={true}
        provider={provider}
        apiKeyOverride="session-key"
        onGenerate={onGenerate}
        onGenerateWithContext={onGenerateWithContext}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: /Generate Success/i }));

    await waitFor(() =>
      expect(onGenerateWithContext).toHaveBeenCalledWith(
        expect.objectContaining({ id: "tab-1" }),
        "success",
        true,
        {
          response_snapshot: {
            kind: "success",
            title: "Success",
            body: { data: [{ id: "ord_1" }] },
            note: "Fallback preview"
          },
          note: "current mock example success for GET /api/orders"
        }
      )
    );
    expect(onGenerate).not.toHaveBeenCalled();
  });

  test("passes each current mock payload as context when generating all", async () => {
    const onGenerateAll = vi.fn().mockResolvedValue(undefined);
    const onGenerateAllWithContexts = vi.fn().mockResolvedValue(undefined);
    render(
      <ResponsePane
        tab={makeTab()}
        onSelectExample={vi.fn()}
        connected={true}
        provider={provider}
        apiKeyOverride="session-key"
        onGenerate={vi.fn()}
        onGenerateAll={onGenerateAll}
        onGenerateAllWithContexts={onGenerateAllWithContexts}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: /Generate all/i }));

    await waitFor(() =>
      expect(onGenerateAllWithContexts).toHaveBeenCalledWith(
        expect.objectContaining({ id: "tab-1" }),
        true,
        {
          success: {
            response_snapshot: {
              kind: "success",
              title: "Success",
              body: { data: [{ id: "ord_1" }] },
              note: "Fallback preview"
            },
            note: "current mock example success for GET /api/orders"
          },
          empty: {
            response_snapshot: {
              kind: "empty",
              title: "Empty",
              body: [],
              note: "No orders"
            },
            note: "current mock example empty for GET /api/orders"
          },
          error: {
            response_snapshot: {
              kind: "error",
              title: "Error",
              body: { error: "unauthorized" },
              note: "Auth error"
            },
            note: "current mock example error for GET /api/orders"
          }
        }
      )
    );
    expect(onGenerateAll).not.toHaveBeenCalled();
  });

  test("passes the current mock payload as context when previewing a prompt", async () => {
    const onPreviewPrompt = vi.fn().mockResolvedValue({
      system: "system",
      user: "user"
    });
    render(
      <ResponsePane
        tab={makeTab()}
        onSelectExample={vi.fn()}
        connected={true}
        provider={provider}
        apiKeyOverride="session-key"
        onGenerate={vi.fn()}
        onPreviewPrompt={onPreviewPrompt}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: /Preview prompt/i }));

    await waitFor(() =>
      expect(onPreviewPrompt).toHaveBeenCalledWith(
        expect.objectContaining({ id: "tab-1" }),
        "success",
        {
          response_snapshot: {
            kind: "success",
            title: "Success",
            body: { data: [{ id: "ord_1" }] },
            note: "Fallback preview"
          },
          note: "current mock example success for GET /api/orders"
        }
      )
    );
  });
});
