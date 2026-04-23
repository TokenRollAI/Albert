import { useEffect, useState } from "react";
import type { ProviderConfigDraft } from "../types";

const STORAGE_KEY = "albert.provider.draft";

const DEFAULT_DRAFT: ProviderConfigDraft = {
  provider_name: "openai",
  base_url: "https://api.openai.com",
  model: "gpt-4o-mini",
  api_key_env: "OPENAI_API_KEY"
};

function load(): ProviderConfigDraft {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_DRAFT;
    const parsed = JSON.parse(raw) as Partial<ProviderConfigDraft>;
    return {
      ...DEFAULT_DRAFT,
      ...parsed
    };
  } catch {
    return DEFAULT_DRAFT;
  }
}

export function useProviderDraft(): {
  draft: ProviderConfigDraft;
  apiKeyOverride: string;
  update: (patch: Partial<ProviderConfigDraft>) => void;
  setApiKeyOverride: (value: string) => void;
  reset: () => void;
} {
  const [draft, setDraft] = useState<ProviderConfigDraft>(() => load());
  const [apiKeyOverride, setApiKeyOverride] = useState<string>("");

  useEffect(() => {
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(draft));
    } catch {
      /* ignore */
    }
  }, [draft]);

  function update(patch: Partial<ProviderConfigDraft>) {
    setDraft((prev) => ({ ...prev, ...patch }));
  }

  function reset() {
    setDraft(DEFAULT_DRAFT);
    setApiKeyOverride("");
  }

  return { draft, apiKeyOverride, update, setApiKeyOverride, reset };
}
