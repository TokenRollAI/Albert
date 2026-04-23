import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import type {
  GatewayStatus,
  MockExampleKind,
  RateLimitRule,
  RequestLogEntry,
  RequiredHeader
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
    example_overrides: {},
    default_latency_ms: null,
    latency_overrides: {},
    error_rate: 0
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

interface UpdateArgs {
  overrides?: Record<string, MockExampleKind>;
  collectionIds?: string[];
  defaultLatencyMs?: number | null;
  latencyOverrides?: Record<string, number>;
  errorRate?: number;
  captureBodies?: boolean;
  responseHeaders?: Record<string, Record<string, string>>;
  requiredHeaders?: Record<string, RequiredHeader[]>;
  rateLimits?: Record<string, RateLimitRule>;
}

export interface SavedGatewayPreferences {
  host?: string;
  port?: number;
  cors_enabled?: boolean;
}

interface UseMockGatewayResult {
  status: GatewayStatus;
  busy: boolean;
  error: string | null;
  requests: RequestLogEntry[];
  savedPreferences: SavedGatewayPreferences | null;
  start: (args: StartArgs) => Promise<GatewayStatus | null>;
  stop: () => Promise<void>;
  refresh: () => Promise<void>;
  update: (args: UpdateArgs) => Promise<GatewayStatus | null>;
}

export function useMockGateway({
  enabled,
  pollMs = 3000
}: UseMockGatewayOptions): UseMockGatewayResult {
  const [status, setStatus] = useState<GatewayStatus>(EMPTY_STATUS);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [requests, setRequests] = useState<RequestLogEntry[]>([]);
  const [savedPreferences, setSavedPreferences] = useState<SavedGatewayPreferences | null>(
    null
  );
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
    // Best-effort preference load on mount so the Mock Server panel can
    // seed its form with the last-used host/port/cors combo.
    void invoke<SavedGatewayPreferences | null>("load_gateway_preferences")
      .then((prefs) => {
        if (mounted.current && prefs) {
          setSavedPreferences(prefs);
        }
      })
      .catch(() => {});
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
        // Best-effort save: the next session can offer the same host/port
        // as defaults. Failures are intentionally swallowed.
        void invoke("save_gateway_preferences", {
          payload: {
            host,
            port,
            cors_enabled: corsEnabled
          },
          databaseUrl: databaseUrl ?? null
        }).catch(() => {});
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
    async (args: UpdateArgs) => {
      if (!enabled) {
        return null;
      }
      setBusy(true);
      setError(null);
      try {
        const next = await invoke<GatewayStatus>("update_mock_server", {
          args: {
            example_overrides: args.overrides ?? null,
            collection_ids: args.collectionIds ?? null,
            default_latency_ms:
              args.defaultLatencyMs === null || args.defaultLatencyMs === 0
                ? 0
                : args.defaultLatencyMs ?? null,
            latency_overrides: args.latencyOverrides ?? null,
            error_rate: args.errorRate ?? null,
            capture_bodies: args.captureBodies ?? null,
            response_headers: args.responseHeaders ?? null,
            required_headers: args.requiredHeaders ?? null,
            rate_limits: args.rateLimits ?? null,
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

  return {
    status,
    busy,
    error,
    requests,
    savedPreferences,
    start,
    stop,
    refresh,
    update
  };
}
