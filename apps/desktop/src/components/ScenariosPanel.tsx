import { useCallback, useEffect, useState } from "react";
import { Icon } from "./Icon";
import type { StoredScenarioSummary } from "../types";

export interface ScenariosPanelProps {
  running: boolean;
  listScenarios: () => Promise<StoredScenarioSummary[]>;
  onSave: (name: string) => Promise<void>;
  onLoad: (name: string) => Promise<void>;
  onDelete: (name: string) => Promise<void>;
  onRename: (oldName: string, newName: string) => Promise<void>;
}

/**
 * A named gateway config preset. Saving captures the live `GatewayConfigBundle`,
 * loading applies it back via the same pipeline as `import_gateway_config`.
 * Useful for defining reusable "broken backend", "rate limited", or "slow"
 * scenarios that can be flipped between with one click.
 */
export function ScenariosPanel({
  running,
  listScenarios,
  onSave,
  onLoad,
  onDelete,
  onRename
}: ScenariosPanelProps) {
  const [scenarios, setScenarios] = useState<StoredScenarioSummary[]>([]);
  const [draftName, setDraftName] = useState("");
  const [busy, setBusy] = useState(false);
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameDraft, setRenameDraft] = useState("");

  const refresh = useCallback(async () => {
    try {
      const list = await listScenarios();
      setScenarios(list);
    } catch {
      /* surfaced via toast in the underlying hook */
    }
  }, [listScenarios]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function handleSave() {
    const name = draftName.trim();
    if (!name) return;
    setBusy(true);
    try {
      await onSave(name);
      setDraftName("");
      await refresh();
    } finally {
      setBusy(false);
    }
  }

  async function handleLoad(name: string) {
    setBusy(true);
    try {
      await onLoad(name);
    } finally {
      setBusy(false);
    }
  }

  async function handleDelete(name: string) {
    setBusy(true);
    try {
      await onDelete(name);
      await refresh();
    } finally {
      setBusy(false);
    }
  }

  async function commitRename(oldName: string) {
    const next = renameDraft.trim();
    if (!next || next === oldName) {
      setRenamingId(null);
      setRenameDraft("");
      return;
    }
    setBusy(true);
    try {
      await onRename(oldName, next);
      setRenamingId(null);
      setRenameDraft("");
      await refresh();
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel">
      <div className="panel__title panel__title--row">
        <h3>Scenarios</h3>
        <span className="panel__meta">
          {scenarios.length} saved preset
          {scenarios.length === 1 ? "" : "s"}
        </span>
      </div>
      <p className="hint">
        Capture the live config as a named preset (latency, error rate,
        rate limits, status overrides, auth rules, schema enforcement).
        Activate any saved scenario to snap the running server to that
        state in one call.
      </p>
      <div className="formgrid">
        <label className="field">
          <span className="field__label">Save current as</span>
          <input
            type="text"
            value={draftName}
            onChange={(event) => setDraftName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                void handleSave();
              }
            }}
            placeholder="e.g. broken backend"
            spellCheck={false}
            disabled={!running || busy}
          />
        </label>
        <div className="field field--check" style={{ alignItems: "end" }}>
          <button
            type="button"
            className="btn btn--primary btn--sm"
            onClick={() => void handleSave()}
            disabled={!running || busy || !draftName.trim()}
          >
            <Icon name="save" size={12} />
            <span>Save</span>
          </button>
        </div>
      </div>
      {scenarios.length === 0 ? (
        <div className="empty">No scenarios yet.</div>
      ) : (
        <ul className="routelist">
          {scenarios.map((s) => {
            const isRenaming = renamingId === s.id;
            return (
              <li key={s.id} className="routelist__item routelist__item--wide">
                {isRenaming ? (
                  <input
                    type="text"
                    value={renameDraft}
                    onChange={(event) => setRenameDraft(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") {
                        event.preventDefault();
                        void commitRename(s.name);
                      } else if (event.key === "Escape") {
                        setRenamingId(null);
                        setRenameDraft("");
                      }
                    }}
                    autoFocus
                    spellCheck={false}
                  />
                ) : (
                  <span
                    className="routelist__path"
                    title={`Updated ${s.updated_at}`}
                  >
                    {s.name}
                  </span>
                )}
                <div className="row-actions">
                  {isRenaming ? (
                    <>
                      <button
                        type="button"
                        className="btn btn--primary btn--sm"
                        onClick={() => void commitRename(s.name)}
                        disabled={busy}
                      >
                        Save
                      </button>
                      <button
                        type="button"
                        className="btn btn--ghost btn--sm"
                        onClick={() => {
                          setRenamingId(null);
                          setRenameDraft("");
                        }}
                      >
                        Cancel
                      </button>
                    </>
                  ) : (
                    <>
                      <button
                        type="button"
                        className="btn btn--primary btn--sm"
                        onClick={() => void handleLoad(s.name)}
                        disabled={!running || busy}
                        title="Activate this scenario"
                      >
                        <Icon name="play" size={12} />
                        <span>Load</span>
                      </button>
                      <button
                        type="button"
                        className="btn btn--ghost btn--sm"
                        onClick={() => {
                          setRenamingId(s.id);
                          setRenameDraft(s.name);
                        }}
                        disabled={busy}
                      >
                        Rename
                      </button>
                      <button
                        type="button"
                        className="btn btn--danger btn--sm"
                        onClick={() => void handleDelete(s.name)}
                        disabled={busy}
                      >
                        Delete
                      </button>
                    </>
                  )}
                </div>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
