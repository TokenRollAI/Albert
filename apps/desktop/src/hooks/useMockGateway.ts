import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import type {
  GatewayStatus,
  MockExampleKind,
  RequestLogEntry
} from "../types";

const EMPTY_STATUS: GatewayStatus = {
  running: false,
  bind_address: null,
  route_count: 0,
  started_at_epoch_ms: null,
  config: {
    host: "127.0.0.1",
    port: 4317,
    cors_enabled: true,
    example_overrides: {}
  },
  routes: []
};

interface UseMockGatewayOptions {
  enabled: boolean;
  pollMs?: number;
}

interface StartArgs {
  host: string;
  port: number;
  corsEnabled: boolean;
  collectionIds?: string[];
  exampleOverrides?: Record<string, MockExampleKind>;
  databaseUrl?: string;
}

interface UseMockGatewayResult {
  status: GatewayStatus;
  busy: boolean;
  error: string | null;
  requests: RequestLogEntry[];
  start: (args: StartArgs) => Promise<GatewayStatus | null>;
  stop: () => Promise<void>;
  refresh: () => Promise<void>;
  update: (
    overrides?: Record<string, MockExampleKind>,
    collectionIds?: string[]
  ) => Promise<GatewayStatus | null>;
}

export function useMockGateway({
  enabled,
  pollMs = 3000
}: UseMockGatewayOptions): UseMockGatewayResult {
  const [status, setStatus] = useState<GatewayStatus>(EMPTY_STATUS);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [requests, setRequests] = useState<RequestLogEntry[]>([]);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const refresh = useCallback(async () => {
    if (!enabled) {
      setStatus(EMPTY_STATUS);
      setRequests([]);
      return;
    }
    try {
      const next = await invoke<GatewayStatus>("mock_server_status");
      if (!mounted.current) return;
      setStatus(next ?? EMPTY_STATUS);
      if (next?.running) {
        try {
          const log = await invoke<RequestLogEntry[]>("mock_server_requests", {
            limit: 50
          });
          if (mounted.current) {
            setRequests(log);
          }
        } catch {
          /* ignore log fetch */
        }
      } else if (mounted.current) {
        setRequests([]);
      }
    } catch (err) {
      if (mounted.current) {
        setError(String(err));
      }
    }
  }, [enabled]);

  useEffect(() => {
    if (!enabled) {
      return;
    }
    void refresh();
    const handle = window.setInterval(() => {
      void refresh();
    }, pollMs);
    return () => {
      window.clearInterval(handle);
    };
  }, [enabled, pollMs, refresh]);

  const start = useCallback(
    async ({
      host,
      port,
      corsEnabled,
      collectionIds,
      exampleOverrides,
      databaseUrl
    }: StartArgs) => {
      if (!enabled) {
        setError("Mock server needs the Tauri runtime.");
        return null;
      }
      setBusy(true);
      setError(null);
      try {
        const next = await invoke<GatewayStatus>("start_mock_server", {
          args: {
            host,
            port,
            cors_enabled: corsEnabled,
            collection_ids: collectionIds ?? null,
            example_overrides: exampleOverrides ?? null,
            database_url: databaseUrl ?? null
          }
        });
        if (mounted.current) {
          setStatus(next);
        }
        return next;
      } catch (err) {
        if (mounted.current) {
          setError(String(err));
        }
        return null;
      } finally {
        if (mounted.current) {
          setBusy(false);
        }
      }
    },
    [enabled]
  );

  const stop = useCallback(async () => {
    if (!enabled) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const next = await invoke<GatewayStatus>("stop_mock_server");
      if (mounted.current) {
        setStatus(next ?? EMPTY_STATUS);
      }
    } catch (err) {
      if (mounted.current) {
        setError(String(err));
      }
    } finally {
      if (mounted.current) {
        setBusy(false);
      }
    }
  }, [enabled]);

  const update = useCallback(
    async (
      overrides?: Record<string, MockExampleKind>,
      collectionIds?: string[]
    ) => {
      if (!enabled) {
        return null;
      }
      setBusy(true);
      setError(null);
      try {
        const next = await invoke<GatewayStatus>("update_mock_server", {
          args: {
            example_overrides: overrides ?? null,
            collection_ids: collectionIds ?? null,
            database_url: null
          }
        });
        if (mounted.current) {
          setStatus(next);
        }
        return next;
      } catch (err) {
        if (mounted.current) {
          setError(String(err));
        }
        return null;
      } finally {
        if (mounted.current) {
          setBusy(false);
        }
      }
    },
    [enabled]
  );

  return { status, busy, error, requests, start, stop, refresh, update };
}
