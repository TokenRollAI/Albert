import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Icon } from "./Icon";
import type { ProviderConfigDraft } from "../types";

interface TestConnectionResult {
  ok: boolean;
  message: string;
  status?: number | null;
}

interface ProviderEnvStatus {
  env_var: string;
  env_present: boolean;
  override_present: boolean;
  usable: boolean;
  message: string;
}

type ProviderProfile = ProviderConfigDraft;

interface ProvidersPanelProps {
  open: boolean;
  onClose: () => void;
  draft: ProviderConfigDraft;
  apiKeyOverride: string;
  connected: boolean;
  onUpdateDraft: (patch: Partial<ProviderConfigDraft>) => void;
  onUpdateApiKey: (value: string) => void;
}

const PRESETS: Array<{ name: string; value: ProviderConfigDraft }> = [
  {
    name: "OpenAI",
    value: {
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
    }
  },
  {
    name: "OpenAI Responses",
    value: {
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
    }
  },
  {
    name: "Azure OpenAI",
    value: {
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
    }
  },
  {
    name: "Azure Responses",
    value: {
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
    }
  },
  {
    name: "Qwen-compatible",
    value: {
      provider_name: "qwen",
      environment: "local",
      base_url: "https://new-api.fantacy.live",
      model: "qwen3.5-plus-02-15",
      api_key_env: "OPENAI_API_KEY",
      api_type: "openai_compatible",
      azure_deployment: null,
      azure_api_version: null,
      temperature: 0.7,
      max_output_tokens: null,
      reasoning_effort: null,
      schema_repair_attempts: 2
    }
  }
];

const DEFAULT_TEMPERATURE = 0.7;
const DEFAULT_SCHEMA_REPAIR_ATTEMPTS = 2;
const MAX_SCHEMA_REPAIR_ATTEMPTS = 5;
const ALL_ENVIRONMENTS = "__all__";
type ProviderReasoningEffort = Exclude<
  ProviderConfigDraft["reasoning_effort"],
  null | undefined
>;

const REASONING_EFFORT_OPTIONS: Array<{
  label: string;
  value: ProviderReasoningEffort | "";
}> = [
  { label: "Default", value: "" },
  { label: "None", value: "none" },
  { label: "Minimal", value: "minimal" },
  { label: "Low", value: "low" },
  { label: "Medium", value: "medium" },
  { label: "High", value: "high" },
  { label: "Xhigh", value: "xhigh" }
];

function normalizeTemperature(value: number): number {
  if (!Number.isFinite(value)) return DEFAULT_TEMPERATURE;
  return Math.min(2, Math.max(0, value));
}

function parseTemperatureInput(value: string): number {
  if (!value.trim()) return DEFAULT_TEMPERATURE;
  return normalizeTemperature(Number(value));
}

function parseMaxOutputTokensInput(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Math.floor(Number(trimmed));
  if (!Number.isFinite(parsed) || parsed <= 0) return null;
  return parsed;
}

function normalizeSchemaRepairAttempts(value: number): number {
  if (!Number.isFinite(value)) return DEFAULT_SCHEMA_REPAIR_ATTEMPTS;
  return Math.min(MAX_SCHEMA_REPAIR_ATTEMPTS, Math.max(0, Math.floor(value)));
}

function parseSchemaRepairAttemptsInput(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  return normalizeSchemaRepairAttempts(Number(trimmed));
}

function profileEnvironment(profile: ProviderProfile): string {
  const value = profile.environment?.trim();
  return value || "default";
}

function providerEnvironmentOptions(profiles: ProviderProfile[]): string[] {
  return Array.from(new Set(profiles.map(profileEnvironment))).sort((a, b) =>
    a.localeCompare(b)
  );
}

export function ProvidersPanel({
  open,
  onClose,
  draft,
  apiKeyOverride,
  connected,
  onUpdateDraft,
  onUpdateApiKey
}: ProvidersPanelProps) {
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<TestConnectionResult | null>(null);
  const [envStatus, setEnvStatus] = useState<ProviderEnvStatus | null>(null);
  const [profiles, setProfiles] = useState<ProviderProfile[]>([]);
  const [profilesBusy, setProfilesBusy] = useState(false);
  const [profileError, setProfileError] = useState<string | null>(null);
  const [profileEnvironmentFilter, setProfileEnvironmentFilter] =
    useState<string>(ALL_ENVIRONMENTS);

  useEffect(() => {
    if (!open) return;
    setTestResult(null);
  }, [
    open,
    draft.provider_name,
    draft.environment,
    draft.base_url,
    draft.model,
    draft.api_key_env,
    draft.api_type,
    draft.azure_deployment,
    draft.azure_api_version,
    draft.schema_repair_attempts,
    apiKeyOverride
  ]);

  useEffect(() => {
    let cancelled = false;

    if (!open || !connected) {
      setEnvStatus(
        open
          ? {
              env_var: draft.api_key_env,
              env_present: false,
              override_present: apiKeyOverride.trim().length > 0,
              usable: false,
              message: "Tauri runtime required to inspect backend environment."
            }
          : null
      );
      return () => {
        cancelled = true;
      };
    }

    invoke<ProviderEnvStatus>("provider_env_status", {
      args: {
        provider: draft,
        api_key_override: apiKeyOverride || null
      }
    })
      .then((status) => {
        if (!cancelled) setEnvStatus(status);
      })
      .catch((err) => {
        if (!cancelled) {
          setEnvStatus({
            env_var: draft.api_key_env,
            env_present: false,
            override_present: apiKeyOverride.trim().length > 0,
            usable: false,
            message: String(err)
          });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [
    open,
    connected,
    draft.provider_name,
    draft.environment,
    draft.base_url,
    draft.model,
    draft.api_key_env,
    draft.api_type,
    draft.azure_deployment,
    draft.azure_api_version,
    apiKeyOverride
  ]);

  useEffect(() => {
    if (!open) return;
    void refreshProfiles();
  }, [open, connected]);

  if (!open) return null;

  const envStatusLabel = !connected
    ? "Tauri required"
    : envStatus
      ? envStatus.override_present
        ? "override active"
        : envStatus.env_present
          ? `${envStatus.env_var} found`
          : envStatus.env_var
            ? `${envStatus.env_var} missing`
            : "env var missing"
      : "checking key source";
  const envStatusClass = envStatus?.usable
    ? "provider-env-status provider-env-status--ok"
    : connected && !envStatus
      ? "provider-env-status provider-env-status--idle"
      : "provider-env-status provider-env-status--warn";
  const apiType = draft.api_type ?? "openai_compatible";
  const temperature = normalizeTemperature(
    draft.temperature ?? DEFAULT_TEMPERATURE
  );
  const maxOutputTokens = draft.max_output_tokens ?? null;
  const schemaRepairAttempts =
    draft.schema_repair_attempts ?? DEFAULT_SCHEMA_REPAIR_ATTEMPTS;
  const environmentOptions = providerEnvironmentOptions(profiles);
  const visibleProfiles =
    profileEnvironmentFilter === ALL_ENVIRONMENTS
      ? profiles
      : profiles.filter(
          (profile) => profileEnvironment(profile) === profileEnvironmentFilter
        );

  async function refreshProfiles() {
    if (!connected) {
      setProfiles([]);
      setProfileError(null);
      return;
    }
    setProfilesBusy(true);
    setProfileError(null);
    try {
      const result = await invoke<ProviderProfile[]>("list_provider_configs");
      setProfiles(result);
    } catch (err) {
      setProfileError(String(err));
    } finally {
      setProfilesBusy(false);
    }
  }

  async function saveProfile() {
    if (!connected) return;
    setProfilesBusy(true);
    setProfileError(null);
    try {
      const saved = await invoke<ProviderProfile>("save_provider_config", {
        provider: draft
      });
      onUpdateDraft(saved);
      await refreshProfiles();
    } catch (err) {
      setProfileError(String(err));
    } finally {
      setProfilesBusy(false);
    }
  }

  async function deleteProfile(name: string) {
    if (!connected) return;
    setProfilesBusy(true);
    setProfileError(null);
    try {
      await invoke<boolean>("delete_provider_config", { providerName: name });
      await refreshProfiles();
    } catch (err) {
      setProfileError(String(err));
    } finally {
      setProfilesBusy(false);
    }
  }

  function duplicateProfile(profile: ProviderProfile) {
    onUpdateDraft({
      ...profile,
      provider_name: nextDuplicateProfileName(
        profile.provider_name,
        profiles.map((item) => item.provider_name)
      )
    });
  }

  async function runTest() {
    setTesting(true);
    setTestResult(null);
    try {
      const result = await invoke<TestConnectionResult>(
        "test_provider_connection",
        {
          args: {
            provider: draft,
            api_key_override: apiKeyOverride || null
          }
        }
      );
      setTestResult(result);
    } catch (err) {
      setTestResult({ ok: false, message: String(err) });
    } finally {
      setTesting(false);
    }
  }
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
            <div className="panel__title panel__title--row">
              <h3 className="panel__title">Saved profiles</h3>
              <span className="panel__meta">
                {profilesBusy
                  ? "syncing"
                  : profileEnvironmentFilter === ALL_ENVIRONMENTS
                    ? `${profiles.length} saved`
                    : `${visibleProfiles.length}/${profiles.length} saved`}
              </span>
            </div>
            {profiles.length > 0 ? (
              <label className="field provider-profiles__filter">
                <span className="field__label">Environment</span>
                <select
                  value={profileEnvironmentFilter}
                  onChange={(event) =>
                    setProfileEnvironmentFilter(event.target.value)
                  }
                  aria-label="Filter provider profiles by environment"
                >
                  <option value={ALL_ENVIRONMENTS}>All environments</option>
                  {environmentOptions.map((environment) => (
                    <option key={environment} value={environment}>
                      {environment}
                    </option>
                  ))}
                </select>
              </label>
            ) : null}
            {profileError ? (
              <div className="banner banner--error" role="status">
                {profileError}
              </div>
            ) : null}
            {profiles.length === 0 ? (
              <div className="empty provider-profiles__empty">
                {connected
                  ? "No saved provider profiles."
                  : "Tauri runtime required to persist profiles."}
              </div>
            ) : (
              <ul className="provider-profiles">
                {visibleProfiles.map((profile) => (
                  <li
                    key={profile.provider_name}
                    className="provider-profiles__row"
                  >
                    <button
                      type="button"
                      className="provider-profiles__main"
                      onClick={() => onUpdateDraft(profile)}
                      title={`Use ${profile.provider_name}`}
                    >
                      <span className="provider-profiles__name">
                        {profile.provider_name}
                      </span>
                      <span className="provider-profiles__meta">
                        {profileEnvironment(profile)} · {profile.model} ·{" "}
                        {profile.api_key_env}
                      </span>
                    </button>
                    <button
                      type="button"
                      className="btn btn--icon"
                      onClick={() => duplicateProfile(profile)}
                      disabled={profilesBusy}
                      aria-label={`Duplicate ${profile.provider_name} profile`}
                      title="Duplicate profile into the active draft"
                    >
                      <Icon name="copy" size={12} />
                    </button>
                    <button
                      type="button"
                      className="btn btn--icon"
                      onClick={() => deleteProfile(profile.provider_name)}
                      disabled={profilesBusy}
                      aria-label={`Delete ${profile.provider_name} profile`}
                      title="Delete saved profile"
                    >
                      <Icon name="close" size={12} />
                    </button>
                  </li>
                ))}
              </ul>
            )}
            <div className="row-actions">
              <button
                type="button"
                className="btn btn--sm"
                onClick={refreshProfiles}
                disabled={!connected || profilesBusy}
              >
                <Icon name="refresh" size={12} />
                <span>Refresh</span>
              </button>
              <button
                type="button"
                className="btn btn--primary btn--sm"
                onClick={saveProfile}
                disabled={
                  !connected || profilesBusy || !draft.provider_name.trim()
                }
              >
                <Icon name="save" size={12} />
                <span>Save profile</span>
              </button>
            </div>
          </section>

          <section className="panel">
            <div className="panel__title panel__title--row">
              <h3 className="panel__title">Active provider</h3>
              <span
                className={envStatusClass}
                title={
                  envStatus?.message ??
                  "Inspecting the Tauri backend environment."
                }
              >
                {envStatusLabel}
              </span>
            </div>
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
                <span className="field__label">Environment</span>
                <input
                  type="text"
                  value={draft.environment ?? ""}
                  onChange={(event) =>
                    onUpdateDraft({ environment: event.target.value })
                  }
                  placeholder="local, staging, prod"
                  spellCheck={false}
                />
              </label>
              <label className="field">
                <span className="field__label">API type</span>
                <select
                  value={apiType}
                  onChange={(event) => {
                    const nextType = event.target
                      .value as ProviderConfigDraft["api_type"];
                    onUpdateDraft({
                      api_type: nextType,
                      ...(nextType !== "azure_openai" &&
                      nextType !== "azure_openai_responses"
                        ? {
                            azure_deployment: null,
                            azure_api_version: null
                          }
                        : {})
                    });
                  }}
                >
                  <option value="openai_compatible">OpenAI-compatible</option>
                  <option value="openai_responses">OpenAI Responses</option>
                  <option value="azure_openai">Azure OpenAI</option>
                  <option value="azure_openai_responses">
                    Azure Responses
                  </option>
                </select>
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
              {apiType === "azure_openai" ||
              apiType === "azure_openai_responses" ? (
                <>
                  <label className="field">
                    <span className="field__label">Azure deployment</span>
                    <input
                      type="text"
                      value={draft.azure_deployment ?? ""}
                      onChange={(event) =>
                        onUpdateDraft({ azure_deployment: event.target.value })
                      }
                      spellCheck={false}
                    />
                  </label>
                  <label className="field">
                    <span className="field__label">Azure API version</span>
                    <input
                      type="text"
                      value={draft.azure_api_version ?? ""}
                      onChange={(event) =>
                        onUpdateDraft({ azure_api_version: event.target.value })
                      }
                      spellCheck={false}
                    />
                  </label>
                </>
              ) : null}
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
              <div className="provider-generation-controls">
                <div className="field">
                  <span className="field__label-row">
                    <span className="field__label">Temperature</span>
                    <span className="provider-generation-controls__value">
                      {temperature.toFixed(2)}
                    </span>
                  </span>
                  <input
                    type="range"
                    min="0"
                    max="2"
                    step="0.05"
                    value={temperature}
                    onChange={(event) =>
                      onUpdateDraft({
                        temperature: normalizeTemperature(
                          Number(event.target.value)
                        )
                      })
                    }
                    aria-label="Temperature"
                  />
                  <input
                    type="number"
                    min="0"
                    max="2"
                    step="0.05"
                    value={temperature}
                    onChange={(event) =>
                      onUpdateDraft({
                        temperature: parseTemperatureInput(event.target.value)
                      })
                    }
                    aria-label="Temperature value"
                  />
                </div>
                <label className="field">
                  <span className="field__label">Max output tokens</span>
                  <input
                    type="number"
                    min="1"
                    step="1"
                    value={maxOutputTokens ?? ""}
                    onChange={(event) =>
                      onUpdateDraft({
                        max_output_tokens: parseMaxOutputTokensInput(
                          event.target.value
                        )
                      })
                    }
                    placeholder="provider default"
                    aria-label="Max output tokens"
                  />
                </label>
                <label className="field">
                  <span className="field__label">Reasoning effort</span>
                  <select
                    value={draft.reasoning_effort ?? ""}
                    onChange={(event) =>
                      onUpdateDraft({
                        reasoning_effort:
                          event.target.value === ""
                            ? null
                            : (event.target.value as ProviderReasoningEffort)
                      })
                    }
                    aria-label="Reasoning effort"
                  >
                    {REASONING_EFFORT_OPTIONS.map((option) => (
                      <option key={option.label} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="field">
                  <span className="field__label">Schema repair retries</span>
                  <input
                    type="number"
                    min="0"
                    max={MAX_SCHEMA_REPAIR_ATTEMPTS}
                    step="1"
                    value={schemaRepairAttempts}
                    onChange={(event) =>
                      onUpdateDraft({
                        schema_repair_attempts:
                          parseSchemaRepairAttemptsInput(event.target.value)
                      })
                    }
                    aria-label="Schema repair retries"
                  />
                </label>
              </div>
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

            <div className="row-actions">
              <button
                type="button"
                className="btn btn--primary btn--sm"
                onClick={runTest}
                disabled={!connected || testing}
                title={
                  connected
                    ? "Send a minimal chat request to verify auth + reachability"
                    : "Tauri runtime required"
                }
              >
                <Icon name="zap" size={12} />
                <span>{testing ? "Testing…" : "Test connection"}</span>
              </button>
              {testResult ? (
                <span
                  className={
                    testResult.ok
                      ? "provider-test-result provider-test-result--ok"
                      : "provider-test-result provider-test-result--err"
                  }
                  title={testResult.message}
                >
                  {testResult.ok ? "✓ connected" : "✗ failed"}
                </span>
              ) : null}
            </div>
            {testResult && !testResult.ok ? (
              <div className="banner banner--error" role="status">
                {testResult.message}
              </div>
            ) : null}
          </section>
        </div>
      </div>
    </div>
  );
}

export function nextDuplicateProfileName(
  sourceName: string,
  existingNames: string[]
): string {
  const base = `${sourceName.trim() || "provider"}-copy`;
  const existing = new Set(existingNames);
  if (!existing.has(base)) return base;
  for (let idx = 2; idx < 1000; idx += 1) {
    const candidate = `${base}-${idx}`;
    if (!existing.has(candidate)) return candidate;
  }
  return `${base}-${Date.now()}`;
}
