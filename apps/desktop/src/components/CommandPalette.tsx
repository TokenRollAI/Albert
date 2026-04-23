import { useEffect, useMemo, useRef, useState } from "react";
import { fuzzyFilter } from "../lib/fuzzy";

/**
 * A command palette entry can either open an endpoint (jumps to that
 * tab, creating it if necessary) or fire an arbitrary action (toggle a
 * drawer, run a gateway command, etc.). Kept as a discriminated union
 * so the caller can dispatch without a separate payload lookup.
 */
export type CommandItem =
  | {
      kind: "endpoint";
      id: string;
      label: string;
      subtitle?: string;
      collectionId: string;
      endpointMethod: string;
      endpointPath: string;
    }
  | {
      kind: "action";
      id: string;
      label: string;
      subtitle?: string;
      run: () => void;
    };

interface CommandPaletteProps {
  open: boolean;
  items: CommandItem[];
  onClose: () => void;
  onRun: (item: CommandItem) => void;
}

export function CommandPalette({
  open,
  items,
  onClose,
  onRun
}: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const listRef = useRef<HTMLUListElement | null>(null);

  // Reset state whenever the palette opens; pre-populating a stale query
  // or selection would fight muscle memory.
  useEffect(() => {
    if (open) {
      setQuery("");
      setSelectedIndex(0);
      // Focus on next frame so the dialog transition doesn't eat the
      // focus attempt.
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  const results = useMemo(
    () => fuzzyFilter(query, items, (item) => item.label),
    [query, items]
  );

  // Keep the selection index within bounds when results change.
  useEffect(() => {
    if (selectedIndex >= results.length) {
      setSelectedIndex(results.length === 0 ? 0 : results.length - 1);
    }
  }, [results.length, selectedIndex]);

  // Scroll the active option into view when arrow keys move past the
  // rendered viewport.
  useEffect(() => {
    if (!listRef.current) return;
    const node = listRef.current.querySelector<HTMLLIElement>(
      `[data-index="${selectedIndex}"]`
    );
    if (node && typeof node.scrollIntoView === "function") {
      node.scrollIntoView({ block: "nearest" });
    }
  }, [selectedIndex]);

  if (!open) return null;

  function handleKey(event: React.KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelectedIndex((idx) =>
        results.length === 0 ? 0 : (idx + 1) % results.length
      );
      return;
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelectedIndex((idx) =>
        results.length === 0 ? 0 : (idx - 1 + results.length) % results.length
      );
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      const picked = results[selectedIndex];
      if (picked) {
        onRun(picked.item);
      }
    }
  }

  return (
    <div
      className="cmdpalette"
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
    >
      <div className="cmdpalette__backdrop" onClick={onClose} />
      <div className="cmdpalette__shell">
        <input
          ref={inputRef}
          type="text"
          className="cmdpalette__input"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setSelectedIndex(0);
          }}
          onKeyDown={handleKey}
          placeholder="Jump to endpoint or action… (↑↓ to move, ⏎ to run)"
          spellCheck={false}
          aria-label="Command palette query"
        />
        {results.length === 0 ? (
          <div className="cmdpalette__empty">
            No matches for <code>{query || "…"}</code>
          </div>
        ) : (
          <ul
            ref={listRef}
            className="cmdpalette__list"
            role="listbox"
            aria-label="Palette results"
          >
            {results.map(({ item }, index) => {
              const isActive = index === selectedIndex;
              return (
                <li
                  key={`${item.kind}:${item.id}`}
                  data-index={index}
                  className={
                    isActive
                      ? "cmdpalette__row cmdpalette__row--active"
                      : "cmdpalette__row"
                  }
                  role="option"
                  aria-selected={isActive}
                  onMouseEnter={() => setSelectedIndex(index)}
                  onClick={() => onRun(item)}
                >
                  <span
                    className={
                      item.kind === "endpoint"
                        ? "cmdpalette__kind cmdpalette__kind--endpoint"
                        : "cmdpalette__kind cmdpalette__kind--action"
                    }
                  >
                    {item.kind === "endpoint" ? "→" : "▶"}
                  </span>
                  <span className="cmdpalette__label">{item.label}</span>
                  {item.subtitle ? (
                    <span className="cmdpalette__subtitle">
                      {item.subtitle}
                    </span>
                  ) : null}
                </li>
              );
            })}
          </ul>
        )}
        <footer className="cmdpalette__footer">
          <kbd>↑</kbd>
          <kbd>↓</kbd> navigate
          <span className="cmdpalette__sep">·</span>
          <kbd>⏎</kbd> run
          <span className="cmdpalette__sep">·</span>
          <kbd>Esc</kbd> close
        </footer>
      </div>
    </div>
  );
}
