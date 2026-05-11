import { useMemo, useState } from "react";
import { Icon } from "./Icon";
import { JsonView } from "./JsonView";
import type {
  EndpointTab,
  ExampleKind,
  GenerationContext,
  MockExample,
  ProviderConfigDraft
} from "../types";

interface PromptPreviewPayload {
  system: string;
  user: string;
  endpoint_context?: unknown;
}

interface ResponsePaneProps {
  tab: EndpointTab;
  onSelectExample: (kind: ExampleKind) => void;
  connected: boolean;
  provider: ProviderConfigDraft;
  apiKeyOverride: string;
  onGenerate: (
    tab: EndpointTab,
    intent: ExampleKind,
    persist: boolean
  ) => Promise<MockExample | null>;
  onGenerateWithContext?: (
    tab: EndpointTab,
    intent: ExampleKind,
    persist: boolean,
    context: GenerationContext
  ) => Promise<MockExample | null>;
  onGenerateAll?: (tab: EndpointTab, persist: boolean) => Promise<void>;
  onGenerateAllWithContexts?: (
    tab: EndpointTab,
    persist: boolean,
    contexts: Partial<Record<ExampleKind, GenerationContext>>
  ) => Promise<void>;
  onPreviewPrompt?: (
    tab: EndpointTab,
    intent: ExampleKind,
    generationContext?: GenerationContext | null
  ) => Promise<PromptPreviewPayload>;
  onSaveExample?: (
    tab: EndpointTab,
    example: MockExample
  ) => Promise<MockExample | null>;
  onExampleUpdated?: (tab: EndpointTab, example: MockExample) => void;
}

const KIND_LABEL: Record<ExampleKind, string> = {
  success: "Success",
  empty: "Empty",
  error: "Error"
};

export function ResponsePane({
  tab,
  onSelectExample,
  connected,
  provider,
  apiKeyOverride,
  onGenerate,
  onGenerateWithContext,
  onGenerateAll,
  onGenerateAllWithContexts,
  onPreviewPrompt,
  onSaveExample,
  onExampleUpdated
}: ResponsePaneProps) {
  const { endpoint, example } = tab;
  const [generating, setGenerating] = useState<ExampleKind | null>(null);
  const [generatingAll, setGeneratingAll] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [persist, setPersist] = useState(true);
  const [copied, setCopied] = useState(false);
  const [editing, setEditing] = useState(false);
  const [editDraft, setEditDraft] = useState("");
  const [editError, setEditError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const available = endpoint.examples.map((item) => item.kind as ExampleKind);
  const selectedResponse =
    endpoint.responses.find((response) =>
      matchExample(example, response.status_code)
    ) ?? endpoint.responses[0];
  const currentExample = endpoint.examples.find((e) => e.kind === example);

  const renderedValue = useMemo(() => {
    if (currentExample?.payload !== undefined && currentExample?.payload !== null) {
      return currentExample.payload;
    }
    return {
      status: selectedResponse?.status_code ?? "200",
      contentType: selectedResponse?.content_type ?? "application/json",
      example,
      schemaRoot: selectedResponse?.schema?.node_type ?? null,
      note:
        "No sample payload persisted yet. Use ✨ Generate to produce one via AI."
    };
  }, [currentExample, selectedResponse, example]);
  const renderedBody = useMemo(
    () => {
      try {
        return JSON.stringify(renderedValue, null, 2);
      } catch {
        return String(renderedValue ?? "");
      }
    },
    [renderedValue]
  );

  const canGenerate =
    connected && !!provider.base_url && !!provider.model;

  function buildCurrentExampleContext(
    intent: ExampleKind
  ): GenerationContext | null {
    const exampleForIntent = endpoint.examples.find((e) => e.kind === intent);
    if (!exampleForIntent || exampleForIntent.payload === undefined) {
      return null;
    }
    return {
      response_snapshot: {
        kind: intent,
        title: exampleForIntent.title,
        body: exampleForIntent.payload,
        note: exampleForIntent.note ?? null
      },
      note: `current mock example ${intent} for ${tab.method} ${tab.endpoint.path}`
    };
  }

  async function handleGenerate(intent: ExampleKind) {
    if (!canGenerate) {
      setError(
        !connected
          ? "AI generation requires the Tauri runtime."
          : "Provider model and base URL are required."
      );
      return;
    }
    setGenerating(intent);
    setError(null);
    try {
      const context = buildCurrentExampleContext(intent);
      const result =
        context && onGenerateWithContext
          ? await onGenerateWithContext(tab, intent, persist, context)
          : await onGenerate(tab, intent, persist);
      if (result && onExampleUpdated) {
        onExampleUpdated(tab, result);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setGenerating(null);
    }
  }

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(renderedBody);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1200);
    } catch {
      /* ignore */
    }
  }

  async function handleGenerateAll() {
    if (!canGenerate || (!onGenerateAll && !onGenerateAllWithContexts)) return;
    setGeneratingAll(true);
    setError(null);
    try {
      if (onGenerateAllWithContexts) {
        const contexts: Partial<Record<ExampleKind, GenerationContext>> = {};
        for (const intent of ["success", "empty", "error"] as ExampleKind[]) {
          const context = buildCurrentExampleContext(intent);
          if (context) {
            contexts[intent] = context;
          }
        }
        await onGenerateAllWithContexts(tab, persist, contexts);
      } else if (onGenerateAll) {
        await onGenerateAll(tab, persist);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setGeneratingAll(false);
    }
  }

  function startEdit() {
    setEditDraft(renderedBody);
    setEditError(null);
    setEditing(true);
  }

  async function handleSaveEdit() {
    if (!onSaveExample) return;
    let parsed: unknown;
    try {
      parsed = JSON.parse(editDraft);
    } catch (err) {
      setEditError(`Invalid JSON: ${String(err)}`);
      return;
    }
    setSaving(true);
    setEditError(null);
    try {
      const saved = await onSaveExample(tab, {
        kind: example,
        title: currentExample?.title ?? KIND_LABEL[example],
        payload: parsed,
        note: "Hand-edited"
      });
      if (saved && onExampleUpdated) {
        onExampleUpdated(tab, saved);
      }
      setEditing(false);
    } catch (err) {
      setEditError(String(err));
    } finally {
      setSaving(false);
    }
  }

  const missingKey =
    connected && !apiKeyOverride && !hasProcessEnv(provider.api_key_env);

  return (
    <section className="response">
      <header className="response__head">
        <h3>Mock Response</h3>
        <div className="response__segments" role="tablist">
          {(["success", "empty", "error"] as ExampleKind[]).map((kind) => {
            const enabled = available.includes(kind);
            const active = kind === example;
            return (
              <button
                key={kind}
                type="button"
                role="tab"
                className={
                  active
                    ? `segment segment--active segment--${kind}`
                    : `segment segment--${kind}`
                }
                aria-selected={active}
                disabled={!enabled}
                onClick={() => onSelectExample(kind)}
              >
                {KIND_LABEL[kind]}
              </button>
            );
          })}
        </div>
      </header>

      <div className="response__meta">
        <span className="response__status">
          {selectedResponse?.status_code ?? "—"}
        </span>
        <span className="response__ctype">
          {selectedResponse?.content_type ?? "application/json"}
        </span>
        {selectedResponse?.description ? (
          <span className="response__desc">{selectedResponse.description}</span>
        ) : null}
        {currentExample?.note ? (
          <span className="response__note" title={currentExample.note}>
            <Icon name="info" size={12} /> {currentExample.note}
          </span>
        ) : null}
      </div>

      <div className="response__toolbar">
        <div className="response__toolbar-left">
          <button
            type="button"
            className="btn btn--ghost btn--sm"
            onClick={handleCopy}
            title="Copy payload"
            disabled={editing}
          >
            <Icon name="copy" size={12} />
            <span>{copied ? "Copied" : "Copy payload"}</span>
          </button>
          {onSaveExample ? (
            !editing ? (
              <button
                type="button"
                className="btn btn--ghost btn--sm"
                onClick={startEdit}
                disabled={!connected}
                title={
                  connected
                    ? "Edit this mock payload"
                    : "Tauri runtime required to persist edits"
                }
              >
                <Icon name="settings" size={12} />
                <span>Edit</span>
              </button>
            ) : (
              <>
                <button
                  type="button"
                  className="btn btn--primary btn--sm"
                  onClick={handleSaveEdit}
                  disabled={saving}
                >
                  <Icon name="save" size={12} />
                  <span>{saving ? "Saving…" : "Save"}</span>
                </button>
                <button
                  type="button"
                  className="btn btn--ghost btn--sm"
                  onClick={() => {
                    setEditing(false);
                    setEditError(null);
                  }}
                  disabled={saving}
                >
                  Cancel
                </button>
              </>
            )
          ) : null}
          <label className="toggle">
            <input
              type="checkbox"
              checked={persist}
              onChange={(event) => setPersist(event.target.checked)}
            />
            <span>Save generated results</span>
          </label>
        </div>
        <div className="response__toolbar-right">
          {onPreviewPrompt ? (
            <button
              type="button"
              className="btn btn--ghost btn--sm"
              onClick={() =>
                onPreviewPrompt(tab, example, buildCurrentExampleContext(example))
              }
              disabled={!connected}
              title="Show the system + user prompt"
            >
              <Icon name="info" size={12} />
              <span>Preview prompt</span>
            </button>
          ) : null}
          {onGenerateAll || onGenerateAllWithContexts ? (
            <button
              type="button"
              className="btn btn--ghost btn--sm"
              onClick={handleGenerateAll}
              disabled={!canGenerate || generatingAll || generating !== null}
              title="Generate success, empty, and error examples in one go"
            >
              <Icon name="zap" size={12} />
              <span>
                {generatingAll ? "Generating all…" : "Generate all"}
              </span>
            </button>
          ) : null}
          <button
            type="button"
            className="btn btn--primary btn--sm"
            onClick={() => handleGenerate(example)}
            disabled={!canGenerate || generating !== null || generatingAll}
            title={
              canGenerate
                ? `Generate ${example} example via ${provider.provider_name}`
                : "Tauri runtime required"
            }
          >
            <Icon name="sparkles" size={12} />
            <span>
              {generating === example
                ? "Generating…"
                : `Generate ${KIND_LABEL[example]}`}
            </span>
          </button>
        </div>
      </div>

      {missingKey ? (
        <div className="banner banner--warn">
          <Icon name="info" size={12} /> No API key entered. Paste a key in
          the Providers panel before generating AI mocks.
        </div>
      ) : null}

      {error ? (
        <div className="banner banner--error" role="status">
          {error}
        </div>
      ) : null}

      <div className="response__body">
        {editing ? (
          <>
            <textarea
              className="response__editor"
              value={editDraft}
              onChange={(event) => setEditDraft(event.target.value)}
              spellCheck={false}
              autoFocus
            />
            {editError ? (
              <div className="banner banner--error" role="status">
                {editError}
              </div>
            ) : null}
          </>
        ) : (
          <JsonView value={renderedValue} />
        )}
      </div>
    </section>
  );
}

function matchExample(example: ExampleKind, status: string): boolean {
  if (example === "success") {
    return status.startsWith("2");
  }
  if (example === "error") {
    return status.startsWith("4") || status.startsWith("5");
  }
  return status === "204";
}

function hasProcessEnv(_key: string): boolean {
  // Browser-side we can't read server-side env; surface as hint.
  return false;
}
