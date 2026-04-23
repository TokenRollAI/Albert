import { useMemo, useState } from "react";
import { Icon } from "./Icon";
import type {
  EndpointTab,
  ExampleKind,
  MockExample,
  ProviderConfigDraft
} from "../types";

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
  onExampleUpdated
}: ResponsePaneProps) {
  const { endpoint, example } = tab;
  const [generating, setGenerating] = useState<ExampleKind | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [persist, setPersist] = useState(true);
  const [copied, setCopied] = useState(false);

  const available = endpoint.examples.map((item) => item.kind as ExampleKind);
  const selectedResponse =
    endpoint.responses.find((response) =>
      matchExample(example, response.status_code)
    ) ?? endpoint.responses[0];
  const currentExample = endpoint.examples.find((e) => e.kind === example);

  const renderedBody = useMemo(() => {
    if (currentExample?.payload !== undefined && currentExample?.payload !== null) {
      try {
        return JSON.stringify(currentExample.payload, null, 2);
      } catch {
        /* fall through */
      }
    }
    return JSON.stringify(
      {
        status: selectedResponse?.status_code ?? "200",
        contentType: selectedResponse?.content_type ?? "application/json",
        example,
        schemaRoot: selectedResponse?.schema?.node_type ?? null,
        note:
          "No sample payload persisted yet. Use ✨ Generate to produce one via AI."
      },
      null,
      2
    );
  }, [currentExample, selectedResponse, example]);

  const canGenerate =
    connected && !!provider.base_url && !!provider.model;

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
      const result = await onGenerate(tab, intent, persist);
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

  const missingKey =
    !apiKeyOverride && !hasProcessEnv(provider.api_key_env);

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
          >
            <Icon name="copy" size={12} />
            <span>{copied ? "Copied" : "Copy payload"}</span>
          </button>
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
          <button
            type="button"
            className="btn btn--primary btn--sm"
            onClick={() => handleGenerate(example)}
            disabled={!canGenerate || generating !== null}
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
          <Icon name="info" size={12} /> No API key override entered. Ensure{" "}
          <code>{provider.api_key_env}</code> is exported where the backend
          started, or paste a key in the Providers panel.
        </div>
      ) : null}

      {error ? (
        <div className="banner banner--error" role="status">
          {error}
        </div>
      ) : null}

      <div className="response__body">
        <pre className="code-block">{renderedBody}</pre>
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
