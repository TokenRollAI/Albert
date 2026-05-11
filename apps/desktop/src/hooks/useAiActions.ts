import { invoke } from "@tauri-apps/api/core";
import { useCallback } from "react";
import type {
  EndpointTab,
  ExampleKind,
  GenerationContext,
  MockExample,
  ProviderConfigDraft
} from "../types";
import type { UseToasts } from "./useToasts";

export interface PromptPreview {
  system: string;
  user: string;
  endpoint_context?: unknown;
}

interface UseAiActionsArgs {
  isTauriRuntime: boolean;
  providerDraft: ProviderConfigDraft;
  apiKeyOverride: string;
  toasts: UseToasts;
  setStatusMessage: (msg: string) => void;
  refreshStoredCollections: () => Promise<void>;
  updateEndpointExample: (id: string, example: MockExample) => void;
  promptPreviewSetters: {
    setPreview: (preview: PromptPreview | null) => void;
    setOpen: (open: boolean) => void;
    setLoading: (loading: boolean) => void;
    setError: (error: string | null) => void;
  };
}

export interface AiActions {
  generate: (
    tab: EndpointTab,
    intent: ExampleKind,
    persist: boolean
  ) => Promise<MockExample | null>;
  generateWithContext: (
    tab: EndpointTab,
    intent: ExampleKind,
    persist: boolean,
    generationContext: GenerationContext
  ) => Promise<MockExample | null>;
  generateAll: (tab: EndpointTab, persist: boolean) => Promise<void>;
  generateAllWithContexts: (
    tab: EndpointTab,
    persist: boolean,
    generationContexts: Partial<Record<ExampleKind, GenerationContext>>
  ) => Promise<void>;
  previewPrompt: (
    tab: EndpointTab,
    intent: ExampleKind,
    generationContext?: GenerationContext | null
  ) => Promise<PromptPreview>;
  saveExample: (
    tab: EndpointTab,
    example: MockExample
  ) => Promise<MockExample | null>;
}

/**
 * All AI + mock-example mutations that cross the Tauri boundary.
 * Extracted from App.tsx to keep the root component focused on layout
 * and state wiring.
 */
export function useAiActions({
  isTauriRuntime,
  providerDraft,
  apiKeyOverride,
  toasts,
  setStatusMessage,
  refreshStoredCollections,
  updateEndpointExample,
  promptPreviewSetters
}: UseAiActionsArgs): AiActions {
  const generateExample = useCallback(
    async (
      tab: EndpointTab,
      intent: ExampleKind,
      persist: boolean,
      generationContext: GenerationContext | null
    ) => {
      if (!isTauriRuntime) {
        throw new Error("AI generation requires the Tauri runtime.");
      }
      try {
        const example = await invoke<MockExample>("generate_mock_example", {
          request: {
            endpoint: tab.endpoint,
            intent,
            provider: providerDraft,
            collection_id: tab.collectionId,
            persist,
            database_url: null,
            api_key_override: apiKeyOverride || null,
            generation_context: generationContext
          }
        });
        updateEndpointExample(tab.id, example);
        if (persist) {
          await refreshStoredCollections();
        }
        setStatusMessage(
          `AI ${intent} example ready for ${tab.method} ${tab.path}.`
        );
        toasts.success(
          `${intent} mock generated for ${tab.method} ${tab.path}.`
        );
        return example;
      } catch (error) {
        toasts.error(`Generation failed: ${String(error)}`);
        throw error;
      }
    },
    [
      isTauriRuntime,
      providerDraft,
      apiKeyOverride,
      toasts,
      setStatusMessage,
      refreshStoredCollections,
      updateEndpointExample
    ]
  );

  const generate = useCallback<AiActions["generate"]>(
    async (tab, intent, persist) => {
      return generateExample(tab, intent, persist, null);
    },
    [generateExample]
  );

  const generateWithContext = useCallback<AiActions["generateWithContext"]>(
    async (tab, intent, persist, generationContext) => {
      return generateExample(tab, intent, persist, generationContext);
    },
    [generateExample]
  );

  const generateAll = useCallback<AiActions["generateAll"]>(
    async (tab, persist) => {
      if (!isTauriRuntime) {
        toasts.warn("AI generation requires the Tauri runtime.");
        return;
      }
      const intents: ExampleKind[] = ["success", "empty", "error"];
      const results: Array<{ kind: ExampleKind; ok: boolean }> = [];
      for (const intent of intents) {
        try {
          const example = await invoke<MockExample>("generate_mock_example", {
            request: {
              endpoint: tab.endpoint,
              intent,
              provider: providerDraft,
              collection_id: tab.collectionId,
              persist,
              database_url: null,
              api_key_override: apiKeyOverride || null,
              generation_context: null
            }
          });
          updateEndpointExample(tab.id, example);
          results.push({ kind: intent, ok: true });
        } catch (error) {
          results.push({ kind: intent, ok: false });
          toasts.error(`Generate ${intent} failed: ${String(error)}`);
        }
      }
      if (persist) {
        await refreshStoredCollections();
      }
      const okCount = results.filter((r) => r.ok).length;
      toasts.success(
        `Generated ${okCount}/${intents.length} variant${okCount === 1 ? "" : "s"}.`
      );
    },
    [
      isTauriRuntime,
      providerDraft,
      apiKeyOverride,
      toasts,
      refreshStoredCollections,
      updateEndpointExample
    ]
  );

  const generateAllWithContexts = useCallback<AiActions["generateAllWithContexts"]>(
    async (tab, persist, generationContexts) => {
      if (!isTauriRuntime) {
        toasts.warn("AI generation requires the Tauri runtime.");
        return;
      }
      const intents: ExampleKind[] = ["success", "empty", "error"];
      const results: Array<{ kind: ExampleKind; ok: boolean }> = [];
      for (const intent of intents) {
        try {
          const example = await invoke<MockExample>("generate_mock_example", {
            request: {
              endpoint: tab.endpoint,
              intent,
              provider: providerDraft,
              collection_id: tab.collectionId,
              persist,
              database_url: null,
              api_key_override: apiKeyOverride || null,
              generation_context: generationContexts[intent] ?? null
            }
          });
          updateEndpointExample(tab.id, example);
          results.push({ kind: intent, ok: true });
        } catch (error) {
          results.push({ kind: intent, ok: false });
          toasts.error(`Generate ${intent} failed: ${String(error)}`);
        }
      }
      if (persist) {
        await refreshStoredCollections();
      }
      const okCount = results.filter((r) => r.ok).length;
      toasts.success(
        `Generated ${okCount}/${intents.length} variant${okCount === 1 ? "" : "s"}.`
      );
    },
    [
      isTauriRuntime,
      providerDraft,
      apiKeyOverride,
      toasts,
      refreshStoredCollections,
      updateEndpointExample
    ]
  );

  const previewPrompt = useCallback<AiActions["previewPrompt"]>(
    async (tab, intent, generationContext = null) => {
      promptPreviewSetters.setOpen(true);
      promptPreviewSetters.setLoading(true);
      promptPreviewSetters.setError(null);
      try {
        const preview = await invoke<PromptPreview>(
          "preview_generation_prompt",
          {
            endpoint: tab.endpoint,
            intent,
            generation_context: generationContext
          }
        );
        promptPreviewSetters.setPreview(preview);
        return preview;
      } catch (error) {
        const message = String(error);
        promptPreviewSetters.setError(message);
        throw error;
      } finally {
        promptPreviewSetters.setLoading(false);
      }
    },
    [promptPreviewSetters]
  );

  const saveExample = useCallback<AiActions["saveExample"]>(
    async (tab, example) => {
      if (!isTauriRuntime) {
        throw new Error("Edit requires the Tauri runtime.");
      }
      if (
        !tab.collectionId ||
        tab.collectionId.startsWith("preview:") ||
        tab.collectionId.startsWith("fallback:")
      ) {
        toasts.warn("Import this collection first to save edits.");
        throw new Error("Collection is not persisted yet.");
      }
      try {
        const saved = await invoke<MockExample>("save_mock_example", {
          args: {
            collection_id: tab.collectionId,
            method: tab.method,
            path: tab.endpoint.path,
            kind: example.kind,
            title: example.title,
            payload: example.payload,
            note: example.note ?? null,
            database_url: null
          }
        });
        updateEndpointExample(tab.id, saved);
        await refreshStoredCollections();
        toasts.success(
          `Saved ${example.kind} payload for ${tab.method} ${tab.path}.`
        );
        return saved;
      } catch (error) {
        toasts.error(`Save failed: ${String(error)}`);
        throw error;
      }
    },
    [
      isTauriRuntime,
      toasts,
      refreshStoredCollections,
      updateEndpointExample
    ]
  );

  return {
    generate,
    generateWithContext,
    generateAll,
    generateAllWithContexts,
    previewPrompt,
    saveExample
  };
}
