import { useEffect, useRef, useState } from "react";
import { Icon } from "./Icon";

interface ImportDialogProps {
  open: boolean;
  onClose: () => void;
  onParse: (name: string, body: string) => Promise<void>;
  onImport: (name: string, body: string) => Promise<void>;
  canImport: boolean;
  busy: "parse" | "import" | null;
  message: string | null;
  initialName?: string;
  initialBody?: string;
}

export function ImportDialog({
  open,
  onClose,
  onParse,
  onImport,
  canImport,
  busy,
  message,
  initialName = "",
  initialBody = ""
}: ImportDialogProps) {
  const [name, setName] = useState(initialName);
  const [body, setBody] = useState(initialBody);
  const surfaceRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open) {
      setName(initialName);
      setBody(initialBody);
    }
  }, [open, initialName, initialBody]);

  useEffect(() => {
    if (!open) return;
    function handleKey(event: KeyboardEvent) {
      if (event.key === "Escape" && !busy) {
        onClose();
      }
    }
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, [open, busy, onClose]);

  if (!open) {
    return null;
  }

  const disabled = busy !== null || body.trim().length === 0;

  return (
    <div
      className="modal__overlay"
      onClick={(event) => {
        if (!busy && event.target === event.currentTarget) {
          onClose();
        }
      }}
    >
      <div className="modal" role="dialog" aria-modal="true" ref={surfaceRef}>
        <header className="modal__head">
          <div>
            <p className="modal__eyebrow">Import</p>
            <h2>OpenAPI or cURL</h2>
          </div>
          <button
            type="button"
            className="btn btn--icon"
            onClick={onClose}
            disabled={busy !== null}
            aria-label="Close"
          >
            <Icon name="close" size={14} />
          </button>
        </header>

        <div className="modal__body">
          <label className="field">
            <span>Collection name</span>
            <input
              type="text"
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="Orders API"
              disabled={busy !== null}
            />
          </label>

          <label className="field field--grow">
            <span>Paste OpenAPI JSON/YAML or a cURL command</span>
            <textarea
              value={body}
              onChange={(event) => setBody(event.target.value)}
              placeholder={'{\n  "openapi": "3.0.3",\n  ...\n}'}
              spellCheck={false}
              disabled={busy !== null}
            />
          </label>

          {message ? <p className="modal__message">{message}</p> : null}
        </div>

        <footer className="modal__foot">
          <div className="modal__hint">
            {canImport
              ? "Import writes to SQLite; Parse Preview only normalizes locally."
              : "Tauri runtime unavailable — only Parse Preview works in the browser."}
          </div>
          <div className="modal__actions">
            <button
              type="button"
              className="btn"
              onClick={() => onParse(name, body)}
              disabled={disabled}
            >
              {busy === "parse" ? "Parsing…" : "Parse Preview"}
            </button>
            <button
              type="button"
              className="btn btn--primary"
              onClick={() => onImport(name, body)}
              disabled={disabled || !canImport}
              title={
                canImport
                  ? "Import to SQLite"
                  : "Requires Tauri runtime"
              }
            >
              {busy === "import" ? "Importing…" : "Import To SQLite"}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}
