import { useEffect, useState } from "react";
import { Icon } from "./Icon";

export interface PromptPreview {
  system: string;
  user: string;
  endpoint_context?: unknown;
}

interface PromptPreviewModalProps {
  open: boolean;
  preview: PromptPreview | null;
  loading: boolean;
  error: string | null;
  onClose: () => void;
}

export function PromptPreviewModal({
  open,
  preview,
  loading,
  error,
  onClose
}: PromptPreviewModalProps) {
  const [copied, setCopied] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    function handleKey(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, [open, onClose]);

  if (!open) return null;

  async function copy(value: string, label: string) {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(label);
      window.setTimeout(() => setCopied(null), 1200);
    } catch {
      /* ignore */
    }
  }

  return (
    <div
      className="modal__overlay"
      onClick={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        className="modal modal--wide"
        role="dialog"
        aria-modal="true"
        aria-label="Generation prompt preview"
      >
        <header className="modal__head">
          <div>
            <p className="modal__eyebrow">AI generation</p>
            <h2>Prompt preview</h2>
          </div>
          <button
            type="button"
            className="btn btn--icon"
            onClick={onClose}
            aria-label="Close"
          >
            <Icon name="close" size={14} />
          </button>
        </header>

        <div className="modal__body">
          {loading ? (
            <div className="empty">Building prompt…</div>
          ) : error ? (
            <div className="banner banner--error" role="status">
              {error}
            </div>
          ) : preview ? (
            <>
              <section className="panel">
                <div className="panel__title panel__title--row">
                  <h3>System</h3>
                  <button
                    type="button"
                    className="btn btn--ghost btn--sm"
                    onClick={() => copy(preview.system, "system")}
                  >
                    <Icon name="copy" size={12} />
                    <span>{copied === "system" ? "Copied" : "Copy"}</span>
                  </button>
                </div>
                <pre className="code-block code-block--wrap">
                  {preview.system}
                </pre>
              </section>
              <section className="panel">
                <div className="panel__title panel__title--row">
                  <h3>User</h3>
                  <button
                    type="button"
                    className="btn btn--ghost btn--sm"
                    onClick={() => copy(preview.user, "user")}
                  >
                    <Icon name="copy" size={12} />
                    <span>{copied === "user" ? "Copied" : "Copy"}</span>
                  </button>
                </div>
                <pre className="code-block code-block--wrap">{preview.user}</pre>
              </section>
            </>
          ) : null}
        </div>
      </div>
    </div>
  );
}
