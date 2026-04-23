import { useCallback, useEffect, useRef, useState } from "react";

export type ToastLevel = "info" | "success" | "warn" | "error";

export interface Toast {
  id: string;
  level: ToastLevel;
  message: string;
  ttlMs: number;
}

export interface UseToasts {
  toasts: Toast[];
  push: (level: ToastLevel, message: string, ttlMs?: number) => string;
  dismiss: (id: string) => void;
  info: (message: string, ttlMs?: number) => string;
  success: (message: string, ttlMs?: number) => string;
  warn: (message: string, ttlMs?: number) => string;
  error: (message: string, ttlMs?: number) => string;
}

const DEFAULT_TTL_MS = 3500;

export function useToasts(): UseToasts {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const timers = useRef<Map<string, number>>(new Map());

  const dismiss = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
    const handle = timers.current.get(id);
    if (handle !== undefined) {
      window.clearTimeout(handle);
      timers.current.delete(id);
    }
  }, []);

  const push = useCallback(
    (level: ToastLevel, message: string, ttlMs: number = DEFAULT_TTL_MS) => {
      const id = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
      setToasts((prev) => [...prev, { id, level, message, ttlMs }]);
      const handle = window.setTimeout(() => dismiss(id), ttlMs);
      timers.current.set(id, handle);
      return id;
    },
    [dismiss]
  );

  useEffect(() => {
    const refs = timers.current;
    return () => {
      refs.forEach((handle) => window.clearTimeout(handle));
      refs.clear();
    };
  }, []);

  const info = useCallback(
    (message: string, ttlMs?: number) => push("info", message, ttlMs),
    [push]
  );
  const success = useCallback(
    (message: string, ttlMs?: number) => push("success", message, ttlMs),
    [push]
  );
  const warn = useCallback(
    (message: string, ttlMs?: number) => push("warn", message, ttlMs),
    [push]
  );
  const error = useCallback(
    (message: string, ttlMs?: number) => push("error", message, ttlMs ?? 6000),
    [push]
  );

  return { toasts, push, dismiss, info, success, warn, error };
}
