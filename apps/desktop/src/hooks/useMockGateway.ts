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
  defaultLatencyMs?: number | null;
  latencyOverrides?: Record<string, number>;
  errorRate?: number;
  captureBodies?: boolean;
  responseHeaders?: Record<string, Record<string, string>>;
  requiredHeaders?: Record<string, RequiredHeader[]>;
  rateLimits?: Record<string, RateLimitRule>;
  statusOverrides?: Record<string, number>;
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
  statusOverrides?: Record<string, number>;
}

/**
 * Best-effort persistence of the full gateway config so the next session
 * resumes with the same enforcement rules. Failures are swallowed —
 * persistence is a convenience, not a guarantee.
 */
async function persistConfig(
  status: GatewayStatus,
  databaseUrl?: string
): Promise<void> {
  const { config } = status;
  const payload: SavedGatewayPreferences = {
    host: config.host,
    port: config.port,
    cors_enabled: config.cors_enabled,
    example_overrides: config.example_overrides,
    default_latency_ms: config.default_latency_ms ?? null,
    latency_overrides: config.latency_overrides,
    error_rate: config.error_rate,
    capture_bodies: config.capture_bodies,
    response_headers: config.response_headers,
    required_headers: config.required_headers,
    rate_limits: config.rate_limits,
    status_overrides: config.status_overrides
  };
  await invoke("save_gateway_preferences", {
    payload,
    databaseUrl: databaseUrl ?? null
  });
}

export interface SavedGatewayPreferences {
  host?: string;
  port?: number;
  cors_enabled?: boolean;
  example_overrides?: Record<string, MockExampleKind>;
  default_latency_ms?: number | null;
  latency_overrides?: Record<string, number>;
  error_rate?: number;
  capture_bodies?: boolean;
  response_headers?: Record<string, Record<string, string>>;
  required_headers?: Record<string, RequiredHeader[]>;
  rate_limits?: Record<string, RateLimitRule>;
  status_overrides?: Record<string, number>;
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
  clearLog: () => Promise<void>;
  exportBundle: () => Promise<unknown>;
  importBundle: (bundle: unknown) => Promise<GatewayStatus | null>;
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
      defaultLatencyMs,
      latencyOverrides,
      errorRate,
      captureBodies,
      responseHeaders,
      requiredHeaders,
      rateLimits,
      statusOverrides,
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
            default_latency_ms: defaultLatencyMs ?? null,
            latency_overrides: latencyOverrides ?? null,
            error_rate: errorRate ?? null,
            capture_bodies: captureBodies ?? null,
            response_headers: responseHeaders ?? null,
            required_headers: requiredHeaders ?? null,
            rate_limits: rateLimits ?? null,
            status_overrides: statusOverrides ?? null,
            database_url: databaseUrl ?? null
          }
        });
        if (mounted.current) {
          setStatus(next);
        }
        // Persist the full running config so the next session restarts
        // with the same chaos / auth gates / rate-limit enforcement.
        void persistConfig(next, databaseUrl).catch(() => {});
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
            status_overrides: args.statusOverrides ?? null,
            database_url: null
          }
        });
        if (mounted.current) {
          setStatus(next);
        }
        // Keep persisted preferences in sync with the live config so a
        // restart doesn't regress freshly-applied rules.
        void persistConfig(next).catch(() => {});
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

  const clearLog = useCallback(async () => {
    if (!enabled) return;
    try {
      await invoke("mock_server_clear_log");
      if (mounted.current) {
        setRequests([]);
      }
    } catch (err) {
      if (mounted.current) {
        setError(String(err));
      }
    }
  }, [enabled]);

  const exportBundle = useCallback(async () => {
    return invoke<unknown>("export_gateway_config");
  }, []);

  const importBundle = useCallback(
    async (bundle: unknown) => {
      if (!enabled) return null;
      setBusy(true);
      setError(null);
      try {
        const next = await invoke<GatewayStatus>("import_gateway_config", {
          args: { bundle, database_url: null }
        });
        if (mounted.current) {
          setStatus(next);
        }
        return next;
      } catch (err) {
        if (mounted.current) {
          setError(String(err));
        }
        throw err;
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
    update,
    clearLog,
    exportBundle,
    importBundle
  };
}
