import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  ProvidersPanel,
  nextDuplicateProfileName
} from "../ProvidersPanel";
import type { ProviderConfigDraft } from "../../types";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

const draft: ProviderConfigDraft = {
  provider_name: "openai",
  environment: "local",
  base_url: "https://api.openai.com",
  model: "gpt-4o-mini",
  api_key_env: "OPENAI_API_KEY",
  api_type: "openai_compatible",
  azure_deployment: null,
  azure_api_version: null,
  temperature: 0.7,
  max_output_tokens: null,
  reasoning_effort: null,
  schema_repair_attempts: 2
};

function providerProfile(
  patch: Partial<ProviderConfigDraft> = {}
): ProviderConfigDraft {
  return {
    ...draft,
    ...patch
  };
}

function renderPanel(options?: {
  connected?: boolean;
  apiKeyOverride?: string;
  onUpdateApiKey?: (value: string) => void;
  onUpdateDraft?: (patch: Partial<ProviderConfigDraft>) => void;
}) {
  return render(
    <ProvidersPanel
      open={true}
      onClose={vi.fn()}
      draft={draft}
      apiKeyOverride={options?.apiKeyOverride ?? ""}
      connected={options?.connected ?? true}
      onUpdateDraft={options?.onUpdateDraft ?? vi.fn()}
      onUpdateApiKey={options?.onUpdateApiKey ?? vi.fn()}
    />
  );
}

function mockInvoke(options?: {
  env?: {
    env_var: string;
    env_present: boolean;
    override_present: boolean;
    usable: boolean;
    message: string;
  };
  profiles?: ProviderConfigDraft[];
}) {
  vi.mocked(invoke).mockImplementation((command: string, args?: unknown) => {
    if (command === "provider_env_status") {
      const provider = (args as { args?: { provider?: ProviderConfigDraft } })
        .args?.provider;
      const envVar = provider?.api_key_env ?? "OPENAI_API_KEY";
      return Promise.resolve(
        options?.env ?? {
          env_var: envVar,
          env_present: true,
          override_present: false,
          usable: true,
          message: `${envVar} is present in the Tauri backend environment.`
        }
      );
    }
    if (command === "list_provider_configs") {
      return Promise.resolve(options?.profiles ?? []);
    }
    if (command === "save_provider_config") {
      return Promise.resolve((args as { provider: ProviderConfigDraft }).provider);
    }
    if (command === "delete_provider_config") {
      return Promise.resolve(true);
    }
    return Promise.resolve(null);
  });
}

afterEach(() => {
  vi.clearAllMocks();
});

describe("ProvidersPanel", () => {
  test("shows API key status when a key source is available", async () => {
    mockInvoke();

    renderPanel();

    expect(await screen.findByText("API key active")).toBeDefined();
    expect(invoke).toHaveBeenCalledWith("provider_env_status", {
      args: {
        provider: draft,
        api_key_override: null
      }
    });
  });

  test("refreshes key status when an API key is entered", async () => {
    vi.mocked(invoke).mockImplementation((command: string, args?: unknown) => {
      if (command === "list_provider_configs") return Promise.resolve([]);
      if (command === "provider_env_status") {
        const override = (args as {
          args: { api_key_override: string | null };
        }).args.api_key_override;
        return Promise.resolve({
          env_var: "OPENAI_API_KEY",
          env_present: false,
          override_present: !!override,
          usable: !!override,
          message: override
            ? "Session API key is active."
            : "OPENAI_API_KEY is not set in the Tauri backend environment."
        });
      }
      return Promise.resolve(null);
    });
    const onUpdateApiKey = vi.fn();
    const { rerender } = renderPanel({ onUpdateApiKey });

    expect(await screen.findByText("API key required")).toBeDefined();
    fireEvent.change(screen.getByLabelText("API key"), {
      target: { value: "sk-test" }
    });
    expect(onUpdateApiKey).toHaveBeenCalledWith("sk-test");

    rerender(
      <ProvidersPanel
        open={true}
        onClose={vi.fn()}
        draft={draft}
        apiKeyOverride="sk-test"
        connected={true}
        onUpdateDraft={vi.fn()}
        onUpdateApiKey={onUpdateApiKey}
      />
    );

    expect(await screen.findByText("API key active")).toBeDefined();
    expect(invoke).toHaveBeenLastCalledWith("provider_env_status", {
      args: {
        provider: draft,
        api_key_override: "sk-test"
      }
    });
  });

  test("does not invoke backend commands when Tauri runtime is unavailable", () => {
    renderPanel({ connected: false });

    expect(invoke).not.toHaveBeenCalled();
    expect(screen.getByText("Tauri required")).toBeDefined();
    expect(
      (screen.getByRole("button", {
        name: /Test connection/i
      }) as HTMLButtonElement).disabled
    ).toBe(true);
  });

  test("loads, selects, saves, and deletes persisted provider profiles", async () => {
    const qwen = providerProfile({
      provider_name: "qwen",
      environment: "staging",
      base_url: "https://new-api.fantacy.live",
      model: "qwen3.5-plus-02-15",
      api_key_env: "OPENAI_API_KEY",
      temperature: 0.2,
      max_output_tokens: 2048,
      reasoning_effort: "low"
    });
    mockInvoke({ profiles: [qwen] });
    const onUpdateDraft = vi.fn();
    renderPanel({ onUpdateDraft });

    expect(await screen.findByText("qwen")).toBeDefined();
    fireEvent.click(screen.getByTitle("Use qwen"));
    expect(onUpdateDraft).toHaveBeenCalledWith(qwen);

    fireEvent.click(screen.getByRole("button", { name: /Save profile/i }));
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("save_provider_config", {
        provider: draft
      })
    );

    fireEvent.click(
      screen.getByRole("button", { name: /Delete qwen profile/i })
    );
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("delete_provider_config", {
        providerName: "qwen"
      })
    );
  });

  test("duplicates a saved provider profile into a new active draft name", async () => {
    const qwen = providerProfile({
      provider_name: "qwen",
      environment: "staging",
      base_url: "https://new-api.fantacy.live",
      model: "qwen3.5-plus-02-15",
      api_key_env: "OPENAI_API_KEY",
      temperature: 0.2,
      max_output_tokens: 2048,
      reasoning_effort: "low"
    });
    mockInvoke({ profiles: [qwen] });
    const onUpdateDraft = vi.fn();
    renderPanel({ onUpdateDraft });

    expect(await screen.findByText("qwen")).toBeDefined();
    fireEvent.click(
      screen.getByRole("button", { name: /Duplicate qwen profile/i })
    );

    expect(onUpdateDraft).toHaveBeenCalledWith({
      ...qwen,
      provider_name: "qwen-copy"
    });
  });

  test("filters saved provider profiles by environment", async () => {
    const local = providerProfile({
      provider_name: "openai-local",
      environment: "local"
    });
    const staging = providerProfile({
      provider_name: "azure-staging",
      environment: "staging",
      api_type: "azure_openai",
      api_key_env: "AZURE_OPENAI_API_KEY"
    });
    mockInvoke({ profiles: [local, staging] });
    renderPanel();

    expect(await screen.findByText("openai-local")).toBeDefined();
    expect(screen.getByText("azure-staging")).toBeDefined();

    fireEvent.change(
      screen.getByLabelText("Filter provider profiles by environment"),
      {
        target: { value: "staging" }
      }
    );

    expect(screen.queryByText("openai-local")).toBeNull();
    expect(screen.getByText("azure-staging")).toBeDefined();
    expect(screen.getByText("1/2 saved")).toBeDefined();
  });

  test("nextDuplicateProfileName avoids existing duplicate names", () => {
    expect(
      nextDuplicateProfileName("openai", [
        "openai",
        "openai-copy",
        "openai-copy-2"
      ])
    ).toBe("openai-copy-3");
  });

  test("surfaces Azure fields and saves provider-specific settings", async () => {
    mockInvoke();
    const onUpdateDraft = vi.fn();
    const { rerender } = renderPanel({ onUpdateDraft });

    fireEvent.click(screen.getByRole("button", { name: "Azure OpenAI" }));
    expect(onUpdateDraft).toHaveBeenCalledWith({
      provider_name: "azure",
      environment: "staging",
      base_url: "https://<resource>.openai.azure.com",
      model: "gpt-4o-mini",
      api_key_env: "AZURE_OPENAI_API_KEY",
      api_type: "azure_openai",
      azure_deployment: "gpt-4o-mini",
      azure_api_version: "2024-10-21",
      temperature: 0.7,
      max_output_tokens: null,
      reasoning_effort: null,
      schema_repair_attempts: 2
    });

    const azureDraft = providerProfile({
      provider_name: "azure",
      environment: "staging",
      base_url: "https://example.openai.azure.com",
      model: "gpt-4o-mini",
      api_key_env: "AZURE_OPENAI_API_KEY",
      api_type: "azure_openai",
      azure_deployment: "orders-deployment",
      azure_api_version: "2024-10-21",
      temperature: 0.3,
      max_output_tokens: 1024,
      reasoning_effort: null
    });
    rerender(
      <ProvidersPanel
        open={true}
        onClose={vi.fn()}
        draft={azureDraft}
        apiKeyOverride=""
        connected={true}
        onUpdateDraft={onUpdateDraft}
        onUpdateApiKey={vi.fn()}
      />
    );

    await waitFor(() =>
      expect(screen.getByText("API key active")).toBeDefined()
    );
    expect(screen.getByLabelText(/Azure deployment/i)).toBeDefined();
    expect(screen.getByLabelText(/Azure API version/i)).toBeDefined();

    const saveButton = screen.getByRole("button", {
      name: /Save profile/i
    }) as HTMLButtonElement;
    await waitFor(() => expect(saveButton.disabled).toBe(false));
    fireEvent.click(saveButton);
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("save_provider_config", {
        provider: azureDraft
      })
    );
  });

  test("selects OpenAI Responses without showing Azure fields", async () => {
    mockInvoke();
    const onUpdateDraft = vi.fn();
    const { rerender } = renderPanel({ onUpdateDraft });

    fireEvent.click(screen.getByRole("button", { name: "OpenAI Responses" }));
    expect(onUpdateDraft).toHaveBeenCalledWith({
      provider_name: "openai-responses",
      environment: "local",
      base_url: "https://api.openai.com",
      model: "gpt-4o-mini",
      api_key_env: "OPENAI_API_KEY",
      api_type: "openai_responses",
      azure_deployment: null,
      azure_api_version: null,
      temperature: 0.7,
      max_output_tokens: null,
      reasoning_effort: null,
      schema_repair_attempts: 2
    });

    const responsesDraft = providerProfile({
      provider_name: "openai-responses",
      base_url: "https://api.openai.com",
      model: "gpt-4o-mini",
      api_key_env: "OPENAI_API_KEY",
      api_type: "openai_responses",
      azure_deployment: "stale-deployment",
      azure_api_version: "2024-10-21",
      temperature: 0.7,
      max_output_tokens: null,
      reasoning_effort: "high"
    });
    rerender(
      <ProvidersPanel
        open={true}
        onClose={vi.fn()}
        draft={responsesDraft}
        apiKeyOverride=""
        connected={true}
        onUpdateDraft={onUpdateDraft}
        onUpdateApiKey={vi.fn()}
      />
    );

    expect(screen.queryByLabelText(/Azure deployment/i)).toBeNull();
    expect(screen.queryByLabelText(/Azure API version/i)).toBeNull();
    await waitFor(() => expect(screen.getByText("API key active")).toBeDefined());

    fireEvent.change(screen.getByLabelText(/API type/i), {
      target: { value: "openai_compatible" }
    });
    expect(onUpdateDraft).toHaveBeenLastCalledWith({
      api_type: "openai_compatible",
      azure_deployment: null,
      azure_api_version: null
    });
  });

  test("selects Azure Responses and preserves Azure deployment fields", async () => {
    mockInvoke();
    const onUpdateDraft = vi.fn();
    const { rerender } = renderPanel({ connected: false, onUpdateDraft });

    fireEvent.click(screen.getByRole("button", { name: "Azure Responses" }));
    expect(onUpdateDraft).toHaveBeenCalledWith({
      provider_name: "azure-responses",
      environment: "staging",
      base_url: "https://<resource>.openai.azure.com",
      model: "gpt-4o-mini",
      api_key_env: "AZURE_OPENAI_API_KEY",
      api_type: "azure_openai_responses",
      azure_deployment: "gpt-4o-mini",
      azure_api_version: null,
      temperature: 0.7,
      max_output_tokens: null,
      reasoning_effort: null,
      schema_repair_attempts: 2
    });

    const azureResponsesDraft = providerProfile({
      provider_name: "azure-responses",
      environment: "staging",
      base_url: "https://example.openai.azure.com",
      model: "fallback-deployment",
      api_key_env: "AZURE_OPENAI_API_KEY",
      api_type: "azure_openai_responses",
      azure_deployment: "orders-responses",
      azure_api_version: null,
      temperature: 0.7,
      max_output_tokens: null,
      reasoning_effort: "high",
      schema_repair_attempts: 2
    });
    rerender(
      <ProvidersPanel
        open={true}
        onClose={vi.fn()}
        draft={azureResponsesDraft}
        apiKeyOverride=""
        connected={false}
        onUpdateDraft={onUpdateDraft}
        onUpdateApiKey={vi.fn()}
      />
    );

    expect(screen.getByLabelText(/Azure deployment/i)).toBeDefined();
    expect(screen.getByLabelText(/Azure API version/i)).toBeDefined();

    fireEvent.change(screen.getByLabelText(/API type/i), {
      target: { value: "openai_responses" }
    });
    expect(onUpdateDraft).toHaveBeenLastCalledWith({
      api_type: "openai_responses",
      azure_deployment: null,
      azure_api_version: null
    });
  });

  test("edits generation controls and saves them with the active profile", async () => {
    mockInvoke();
    const onUpdateDraft = vi.fn();
    const { rerender } = renderPanel({ onUpdateDraft });

    fireEvent.change(screen.getByLabelText("Temperature value"), {
      target: { value: "1.25" }
    });
    expect(onUpdateDraft).toHaveBeenCalledWith({ temperature: 1.25 });

    fireEvent.change(screen.getByLabelText("Max output tokens"), {
      target: { value: "2048" }
    });
    expect(onUpdateDraft).toHaveBeenCalledWith({ max_output_tokens: 2048 });

    fireEvent.change(screen.getByLabelText("Reasoning effort"), {
      target: { value: "high" }
    });
    expect(onUpdateDraft).toHaveBeenCalledWith({ reasoning_effort: "high" });

    fireEvent.change(screen.getByLabelText("Environment"), {
      target: { value: "prod" }
    });
    expect(onUpdateDraft).toHaveBeenCalledWith({ environment: "prod" });

    fireEvent.change(screen.getByLabelText("Schema repair retries"), {
      target: { value: "4" }
    });
    expect(onUpdateDraft).toHaveBeenCalledWith({
      schema_repair_attempts: 4
    });

    const tunedDraft: ProviderConfigDraft = {
      ...draft,
      temperature: 1.25,
      max_output_tokens: 2048,
      reasoning_effort: "high",
      schema_repair_attempts: 4
    };
    rerender(
      <ProvidersPanel
        open={true}
        onClose={vi.fn()}
        draft={tunedDraft}
        apiKeyOverride=""
        connected={true}
        onUpdateDraft={onUpdateDraft}
        onUpdateApiKey={vi.fn()}
      />
    );

    const saveButton = screen.getByRole("button", {
      name: /Save profile/i
    }) as HTMLButtonElement;
    await waitFor(() => expect(saveButton.disabled).toBe(false));
    fireEvent.click(saveButton);
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("save_provider_config", {
        provider: tunedDraft
      })
    );
  });
});
