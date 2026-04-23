import { useEffect } from "react";
import { Icon } from "./Icon";
import type { ShortcutBinding } from "../hooks/useKeyboardShortcuts";

interface ShortcutsOverlayProps {
  open: boolean;
  bindings: ShortcutBinding[];
  onClose: () => void;
}

const MOD_LABEL = isMac() ? "⌘" : "Ctrl";

function isMac(): boolean {
  if (typeof navigator === "undefined") return false;
  return /mac|iphone|ipad|ipod/i.test(navigator.platform);
}

function prettifyCombo(combo: string): string {
  return combo
    .split("+")
    .map((token) => token.trim())
    .map((token) => {
      const lower = token.toLowerCase();
      if (lower === "mod") return MOD_LABEL;
      if (lower === "shift") return "⇧";
      if (lower === "alt" || lower === "option") return "⌥";
      if (lower === "ctrl") return "Ctrl";
      if (lower === "meta" || lower === "cmd") return "⌘";
      if (lower === "/") return "/";
      return token.length === 1 ? token.toUpperCase() : token;
    })
    .join(" ");
}

export function ShortcutsOverlay({
  open,
  bindings,
  onClose
}: ShortcutsOverlayProps) {
  useEffect(() => {
    if (!open) return;
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  // Stable section order; bindings without a description are filtered out
  // so we don't show cryptic entries the user can't interpret.
  const rows = bindings.filter((b) => !!b.description);

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
        aria-label="Keyboard shortcuts"
      >
        <header className="modal__head">
          <div>
            <p className="modal__eyebrow">Keyboard</p>
            <h2>Shortcuts</h2>
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
          <ul className="shortcut-list">
            {rows.map((binding) => (
              <li key={binding.combo} className="shortcut-list__row">
                <kbd className="shortcut-list__combo">
                  {prettifyCombo(binding.combo)}
                </kbd>
                <span className="shortcut-list__desc">
                  {binding.description}
                </span>
              </li>
            ))}
            <li className="shortcut-list__row shortcut-list__row--muted">
              <kbd className="shortcut-list__combo">↑ ↓</kbd>
              <span className="shortcut-list__desc">
                Walk endpoint list in the sidebar (after ⌘K)
              </span>
            </li>
            <li className="shortcut-list__row shortcut-list__row--muted">
              <kbd className="shortcut-list__combo">Enter</kbd>
              <span className="shortcut-list__desc">
                Open first match from the sidebar search
              </span>
            </li>
            <li className="shortcut-list__row shortcut-list__row--muted">
              <kbd className="shortcut-list__combo">Esc</kbd>
              <span className="shortcut-list__desc">
                Dismiss modals and drawers
              </span>
            </li>
          </ul>
        </div>
      </div>
    </div>
  );
}
