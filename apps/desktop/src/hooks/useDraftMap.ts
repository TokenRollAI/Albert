import { useCallback, useMemo, useState } from "react";

/**
 * Shared state machine used by the three per-route editors in the Mock
 * Server drawer (RateLimits / StatusOverrides / ResponseHeaders). Each
 * editor has the same shape:
 *
 *   - Start with the current-on-server value.
 *   - Keep a local draft as the user edits.
 *   - Enable an "Apply" button only when the draft differs from the
 *     server's copy (the dirty check).
 *   - On Apply, flip a busy flag, call the update verb, and stay dirty
 *     until the server value matches the draft again.
 *   - "Reset" pulls the server value back into the draft.
 *
 * Extracting this here removes ~40 duplicated lines per editor and
 * centralizes the one subtle bit: the dirty comparison uses
 * `JSON.stringify`, which is correct for the plain Record<string, V>
 * shapes these maps use but would break on Sets / Dates / cyclical
 * values (none of which these editors ever carry).
 */
export interface DraftMap<T> {
  readonly draft: T;
  setDraft: (next: T | ((prev: T) => T)) => void;
  readonly dirty: boolean;
  readonly busy: boolean;
  apply: () => Promise<void>;
  reset: () => void;
}

export function useDraftMap<T>(
  current: T,
  onApply: (next: T) => Promise<void>
): DraftMap<T> {
  const [draft, setDraft] = useState<T>(current);
  const [busy, setBusy] = useState(false);

  const dirty = useMemo(
    () => JSON.stringify(draft) !== JSON.stringify(current),
    [draft, current]
  );

  const apply = useCallback(async () => {
    setBusy(true);
    try {
      await onApply(draft);
    } finally {
      setBusy(false);
    }
  }, [draft, onApply]);

  const reset = useCallback(() => {
    setDraft(current);
  }, [current]);

  return {
    draft,
    setDraft,
    dirty,
    busy,
    apply,
    reset
  };
}
