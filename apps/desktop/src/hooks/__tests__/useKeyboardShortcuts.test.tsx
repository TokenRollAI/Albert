import { renderHook } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";
import { useKeyboardShortcuts } from "../useKeyboardShortcuts";

function fireKey(init: KeyboardEventInit & { key: string }) {
  // Normalize to the modifier the matcher checks (meta on Mac, ctrl elsewhere).
  // Tests run in jsdom which reports platform "linux" by default, so `ctrlKey`
  // is the mod gate.
  window.dispatchEvent(new KeyboardEvent("keydown", init));
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("useKeyboardShortcuts", () => {
  test("fires handler on matching Mod+K", () => {
    const handler = vi.fn();
    renderHook(() =>
      useKeyboardShortcuts([{ combo: "Mod+k", handler, description: "focus" }])
    );
    fireKey({ key: "k", ctrlKey: true });
    expect(handler).toHaveBeenCalledOnce();
  });

  test("suppresses non-Mod shortcuts while the event target is an input", () => {
    const input = document.createElement("input");
    document.body.appendChild(input);
    const handler = vi.fn();
    renderHook(() =>
      useKeyboardShortcuts([{ combo: "Enter", handler }])
    );
    // Dispatch from the input so `event.target` is the INPUT element,
    // mirroring what real typing would produce.
    input.dispatchEvent(
      new KeyboardEvent("keydown", { key: "Enter", bubbles: true })
    );
    expect(handler).not.toHaveBeenCalled();
    input.remove();
  });

  test("Mod+alt+ArrowRight fires independently of shift / meta combos", () => {
    const handler = vi.fn();
    renderHook(() =>
      useKeyboardShortcuts([
        { combo: "Mod+alt+arrowright", handler, description: "next tab" }
      ])
    );
    // Right mods, right key — should fire.
    fireKey({ key: "ArrowRight", ctrlKey: true, altKey: true });
    expect(handler).toHaveBeenCalledOnce();
    // Missing alt — should NOT fire.
    fireKey({ key: "ArrowRight", ctrlKey: true });
    expect(handler).toHaveBeenCalledOnce();
  });

  test("Mod+1 dispatches only when digit 1 is pressed with mod", () => {
    const handler = vi.fn();
    renderHook(() =>
      useKeyboardShortcuts([{ combo: "Mod+1", handler, description: "tab 1" }])
    );
    fireKey({ key: "1", ctrlKey: true });
    fireKey({ key: "2", ctrlKey: true }); // mismatched digit
    expect(handler).toHaveBeenCalledOnce();
  });

  test("shift modifier must match exactly — Mod+Shift+P vs Mod+P don't cross-fire", () => {
    const plainHandler = vi.fn();
    const shiftHandler = vi.fn();
    renderHook(() =>
      useKeyboardShortcuts([
        { combo: "Mod+p", handler: plainHandler },
        { combo: "Mod+Shift+p", handler: shiftHandler }
      ])
    );
    fireKey({ key: "p", ctrlKey: true });
    expect(plainHandler).toHaveBeenCalledOnce();
    expect(shiftHandler).not.toHaveBeenCalled();
    fireKey({ key: "P", ctrlKey: true, shiftKey: true });
    expect(shiftHandler).toHaveBeenCalledOnce();
  });
});
