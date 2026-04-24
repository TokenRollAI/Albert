import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { Icon } from "./Icon";
import {
  friendlyFetchError,
  validateFetchUrl
} from "../lib/fetchErrors";

interface FetchedSource {
  url: string;
  content_type?: string | null;
  body: string;
  suggested_name?: string | null;
}

function basenameWithoutExtension(filename: string): string {
  const base = filename.split(/[\\/]/).pop() ?? filename;
  const idx = base.lastIndexOf(".");
  return idx > 0 ? base.slice(0, idx) : base;
}

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
  canFetch: boolean;
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
  initialBody = "",
  canFetch
}: ImportDialogProps) {
  const [name, setName] = useState(initialName);
  const [body, setBody] = useState(initialBody);
  const [dragging, setDragging] = useState(false);
  const [fetchUrl, setFetchUrl] = useState("");
  const [fetchBusy, setFetchBusy] = useState(false);
  const [fetchError, setFetchError] = useState<string | null>(null);
  const surfaceRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  async function handleFetchUrl() {
    const validationError = validateFetchUrl(fetchUrl);
    if (validationError) {
      setFetchError(validationError);
      return;
    }
    setFetchBusy(true);
    setFetchError(null);
    try {
      const result = await invoke<FetchedSource>("fetch_remote_source", {
        args: { url: fetchUrl.trim() }
      });
      setBody(result.body);
      if (!name.trim() && result.suggested_name) {
        setName(result.suggested_name);
      }
      // Success feedback lives as a transient banner below the input —
      // setting a neutral "loaded N bytes" message.
      setFetchError(null);
    } catch (err) {
      setFetchError(friendlyFetchError(err));
    } finally {
      setFetchBusy(false);
    }
  }

  async function ingestFile(file: File) {
    const text = await file.text();
    setBody(text);
    if (!name.trim()) {
      setName(basenameWithoutExtension(file.name));
    }
  }

  async function handleDrop(event: React.DragEvent<HTMLDivElement>) {
    event.preventDefault();
    event.stopPropagation();
    setDragging(false);
    if (busy !== null) return;
    const file = event.dataTransfer.files?.[0];
    if (!file) return;
    try {
      await ingestFile(file);
    } catch {
      /* ignore read failures */
    }
  }

  function handleDragOver(event: React.DragEvent<HTMLDivElement>) {
    if (busy !== null) return;
    event.preventDefault();
    event.stopPropagation();
    setDragging(true);
  }

  function handleDragLeave(event: React.DragEvent<HTMLDivElement>) {
    if (event.currentTarget === event.target) {
      setDragging(false);
    }
  }

  async function handleFileInput(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    if (!file) return;
    try {
      await ingestFile(file);
    } catch {
      /* ignore */
    } finally {
      event.target.value = "";
    }
  }

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

          {canFetch ? (
            <div className="field">
              <span className="field__label-row">
                <span>Fetch from URL</span>
                {fetchError ? (
                  <span className="modal__error" title={fetchError}>
                    {fetchError}
                  </span>
                ) : null}
              </span>
              <div className="import__fetch">
                <input
                  type="text"
                  value={fetchUrl}
                  onChange={(event) => setFetchUrl(event.target.value)}
                  placeholder="https://example.com/openapi.json"
                  disabled={busy !== null || fetchBusy}
                  spellCheck={false}
                />
                <button
                  type="button"
                  className="btn btn--secondary btn--sm"
                  onClick={handleFetchUrl}
                  disabled={busy !== null || fetchBusy || !fetchUrl.trim()}
                >
                  <Icon name="link" size={12} />
                  <span>{fetchBusy ? "Fetching…" : "Fetch"}</span>
                </button>
              </div>
            </div>
          ) : null}

          <label className="field field--grow">
            <span className="field__label-row">
              <span>Paste OpenAPI JSON/YAML or a cURL command</span>
              <button
                type="button"
                className="btn btn--ghost btn--sm"
                onClick={() => fileInputRef.current?.click()}
                disabled={busy !== null}
              >
                <Icon name="import" size={12} />
                <span>Choose file…</span>
              </button>
            </span>
            <div
              className={dragging ? "dropzone dropzone--active" : "dropzone"}
              onDragOver={handleDragOver}
              onDragLeave={handleDragLeave}
              onDrop={handleDrop}
            >
              <textarea
                value={body}
                onChange={(event) => setBody(event.target.value)}
                placeholder={
                  'Drop a .json / .yaml / .txt file here, or paste content…\n\n{\n  "openapi": "3.0.3",\n  ...\n}'
                }
                spellCheck={false}
                disabled={busy !== null}
              />
              {dragging ? (
                <div className="dropzone__hint">Release to load file…</div>
              ) : null}
            </div>
            <input
              ref={fileInputRef}
              type="file"
              hidden
              accept=".json,.yaml,.yml,.txt,application/json,text/yaml,text/plain"
              onChange={handleFileInput}
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
