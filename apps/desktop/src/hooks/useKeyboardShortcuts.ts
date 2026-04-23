import { useEffect } from "react";

export interface ShortcutBinding {
  combo: string;
  handler: (event: KeyboardEvent) => void;
  description?: string;
}

/**
 * Register global keyboard shortcuts. `combo` syntax is a plus-delimited
 * string like `Mod+K` (Mod = Cmd on macOS, Ctrl elsewhere), `Shift+/`, etc.
 *
 * The handler is ignored when focus is inside an editable element unless the
 * combo uses Mod or contains a function key.
 */
export function useKeyboardShortcuts(bindings: ShortcutBinding[]): void {
  useEffect(() => {
    function listener(event: KeyboardEvent) {
      if (isTypingTarget(event.target) && !hasModifier(event)) {
        return;
      }
      for (const binding of bindings) {
        if (matches(event, binding.combo)) {
          event.preventDefault();
          binding.handler(event);
          return;
        }
      }
    }
    window.addEventListener("keydown", listener);
    return () => window.removeEventListener("keydown", listener);
  }, [bindings]);
}

function isTypingTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  const tag = target.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
}

function hasModifier(event: KeyboardEvent): boolean {
  return event.metaKey || event.ctrlKey;
}

function isMac(): boolean {
  if (typeof navigator === "undefined") return false;
  return /mac|iphone|ipad|ipod/i.test(navigator.platform);
}

function matches(event: KeyboardEvent, combo: string): boolean {
  const parts = combo.split("+").map((p) => p.trim().toLowerCase());
  const key = parts[parts.length - 1];
  const mods = parts.slice(0, -1);

  const needMod = mods.includes("mod");
  const needShift = mods.includes("shift");
  const needAlt = mods.includes("alt") || mods.includes("option");
  const needCtrl = mods.includes("ctrl");
  const needMeta = mods.includes("meta") || mods.includes("cmd");

  const modPressed = isMac() ? event.metaKey : event.ctrlKey;
  if (needMod && !modPressed) return false;
  if (needShift !== event.shiftKey) return false;
  if (needAlt !== event.altKey) return false;
  if (needCtrl && !event.ctrlKey) return false;
  if (needMeta && !event.metaKey) return false;

  const eventKey = event.key.toLowerCase();
  if (eventKey === key) return true;
  // fallback for "?" / "/" that requires shift on US keyboards
  if (key === "/" && eventKey === "/" && event.shiftKey === needShift) {
    return true;
  }
  return false;
}
