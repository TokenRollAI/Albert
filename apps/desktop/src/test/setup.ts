import { afterEach, beforeEach, vi } from "vitest";

// jsdom's default localStorage can behave oddly across workers; provide a
// simple in-memory shim so every test gets a clean map.
class MemoryStorage implements Storage {
  private store = new Map<string, string>();
  get length(): number {
    return this.store.size;
  }
  clear(): void {
    this.store.clear();
  }
  getItem(key: string): string | null {
    return this.store.has(key) ? (this.store.get(key) as string) : null;
  }
  key(index: number): string | null {
    return [...this.store.keys()][index] ?? null;
  }
  removeItem(key: string): void {
    this.store.delete(key);
  }
  setItem(key: string, value: string): void {
    this.store.set(key, value);
  }
}

Object.defineProperty(window, "localStorage", {
  value: new MemoryStorage(),
  configurable: true
});
Object.defineProperty(window, "sessionStorage", {
  value: new MemoryStorage(),
  configurable: true
});

// jsdom doesn't have a global `navigator.clipboard`. Provide a no-op so
// components that attempt to copy text don't throw in tests.
if (!("clipboard" in navigator)) {
  Object.defineProperty(navigator, "clipboard", {
    value: { writeText: vi.fn().mockResolvedValue(undefined) },
    configurable: true
  });
}

beforeEach(() => {
  window.localStorage.clear();
  window.sessionStorage.clear();
});

afterEach(() => {
  window.localStorage.clear();
  window.sessionStorage.clear();
});
