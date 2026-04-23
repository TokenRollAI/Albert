import { Icon } from "./Icon";
import type { ProviderConfigDraft } from "../types";

interface ProvidersPanelProps {
  open: boolean;
  onClose: () => void;
  draft: ProviderConfigDraft;
  apiKeyOverride: string;
  onUpdateDraft: (patch: Partial<ProviderConfigDraft>) => void;
  onUpdateApiKey: (value: string) => void;
}

const PRESETS = [
  {
    name: "OpenAI",
    value: {
      provider_name: "openai",
      base_url: "https://api.openai.com",
      model: "gpt-4o-mini",
      api_key_env: "OPENAI_API_KEY"
    }
  },
  {
    name: "Azure OpenAI",
    value: {
      provider_name: "azure",
      base_url: "https://<resource>.openai.azure.com",
      model: "gpt-4o-mini",
      api_key_env: "AZURE_OPENAI_API_KEY"
    }
  },
  {
    name: "Qwen-compatible",
    value: {
      provider_name: "qwen",
      base_url: "https://new-api.fantacy.live",
      model: "qwen3.5-plus-02-15",
      api_key_env: "OPENAI_API_KEY"
    }
  }
];

export function ProvidersPanel({
  open,
  onClose,
  draft,
  apiKeyOverride,
  onUpdateDraft,
  onUpdateApiKey
}: ProvidersPanelProps) {
  if (!open) return null;
  return (
    <div className="drawer" role="dialog" aria-label="Provider configuration">
      <div className="drawer__backdrop" onClick={onClose} />
      <div className="drawer__panel">
        <header className="drawer__head">
          <div className="drawer__title">
            <Icon name="sparkles" size={16} />
            <h2>Providers</h2>
          </div>
          <button
            type="button"
            className="btn btn--icon"
            onClick={onClose}
            aria-label="Close providers panel"
          >
            <Icon name="close" size={16} />
          </button>
        </header>

        <div className="drawer__body">
          <section className="panel">
            <h3 className="panel__title">Presets</h3>
            <div className="chipset">
              {PRESETS.map((preset) => (
                <button
                  key={preset.name}
                  type="button"
                  className="chip"
                  onClick={() => onUpdateDraft(preset.value)}
                >
                  {preset.name}
                </button>
              ))}
            </div>
          </section>

          <section className="panel">
            <h3 className="panel__title">Active provider</h3>
            <div className="formgrid formgrid--stack">
              <label className="field">
                <span className="field__label">Name</span>
                <input
                  type="text"
                  value={draft.provider_name}
                  onChange={(event) =>
                    onUpdateDraft({ provider_name: event.target.value })
                  }
                  spellCheck={false}
                />
              </label>
              <label className="field">
                <span className="field__label">Base URL</span>
                <input
                  type="text"
                  value={draft.base_url}
                  onChange={(event) =>
                    onUpdateDraft({ base_url: event.target.value })
                  }
                  spellCheck={false}
                />
              </label>
              <label className="field">
                <span className="field__label">Model</span>
                <input
                  type="text"
                  value={draft.model}
                  onChange={(event) =>
                    onUpdateDraft({ model: event.target.value })
                  }
                  spellCheck={false}
                />
              </label>
              <label className="field">
                <span className="field__label">API key env var</span>
                <input
                  type="text"
                  value={draft.api_key_env}
                  onChange={(event) =>
                    onUpdateDraft({ api_key_env: event.target.value })
                  }
                  spellCheck={false}
                />
              </label>
              <label className="field">
                <span className="field__label">
                  API key override (session only)
                </span>
                <input
                  type="password"
                  value={apiKeyOverride}
                  onChange={(event) => onUpdateApiKey(event.target.value)}
                  autoComplete="off"
                  placeholder="Paste key to override the env variable"
                />
              </label>
            </div>
            <p className="hint">
              The override stays in memory for this session only. For persistent
              credentials, export <code>{draft.api_key_env}</code> in the
              environment where the Tauri backend runs (e.g. via{" "}
              <code>.env</code>).
            </p>
          </section>
        </div>
      </div>
    </div>
  );
}
