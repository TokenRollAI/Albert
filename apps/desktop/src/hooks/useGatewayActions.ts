import { useCallback } from "react";
import { seedRequiredHeadersFromEndpoints } from "../lib/authHints";
import { seedTryItDraft } from "./useTryItDraft";
import type {
  CanonicalEndpoint,
  MockExampleKind,
  RateLimitRule,
  RequestLogEntry,
  RequiredHeader,
  SidebarCollection
} from "../types";
import type { UseToasts } from "./useToasts";

interface UseGatewayActionsArgs {
  mockGateway: {
    start: (args: {
      host: string;
      port: number;
      corsEnabled: boolean;
      defaultLatencyMs?: number | null;
      latencyOverrides?: Record<string, number>;
      errorRate?: number;
      captureBodies?: boolean;
      responseHeaders?: Record<string, Record<string, string>>;
      requiredHeaders?: Record<string, RequiredHeader[]>;
      rateLimits?: Record<string, RateLimitRule>;
      exampleOverrides?: Record<string, MockExampleKind>;
    }) => Promise<{ running: boolean; bind_address?: string | null } | null>;
    update: (args: {
      overrides?: Record<string, MockExampleKind>;
      defaultLatencyMs?: number | null;
      errorRate?: number;
      captureBodies?: boolean;
      rateLimits?: Record<string, RateLimitRule>;
      requiredHeaders?: Record<string, RequiredHeader[]>;
    }) => Promise<unknown>;
    savedPreferences?: {
      default_latency_ms?: number | null;
      latency_overrides?: Record<string, number>;
      error_rate?: number;
      capture_bodies?: boolean;
      response_headers?: Record<string, Record<string, string>>;
      required_headers?: Record<string, RequiredHeader[]>;
      rate_limits?: Record<string, RateLimitRule>;
      example_overrides?: Record<string, MockExampleKind>;
    } | null;
    clearLog?: () => Promise<void>;
  };
  sidebarCollections: SidebarCollection[];
  openTab: (
    collectionId: string,
    collectionName: string,
    endpoint: CanonicalEndpoint
  ) => void;
  setMockPanelOpen: (open: boolean) => void;
  toasts: UseToasts;
}

export interface GatewayActions {
  start: (port: number, host: string, cors: boolean) => Promise<void>;
  applyOverrides: (
    overrides: Record<string, MockExampleKind>
  ) => Promise<void>;
  applyChaos: (defaultLatencyMs: number, errorRate: number) => Promise<void>;
  toggleCaptureBodies: (enabled: boolean) => Promise<void>;
  applyRateLimits: (
    rules: Record<string, RateLimitRule>
  ) => Promise<void>;
  seedRequiredHeadersFromHints: () => Promise<void>;
  clearLog: () => Promise<void>;
  replayRequest: (entry: RequestLogEntry) => void;
}

/**
 * Gateway control surface used by App.tsx. Each method is thin glue around
 * `useMockGateway`'s start/update primitives; keeping them here means the
 * root component only needs to thread the `mockGateway` reference once.
 */
export function useGatewayActions({
  mockGateway,
  sidebarCollections,
  openTab,
  setMockPanelOpen,
  toasts
}: UseGatewayActionsArgs): GatewayActions {
  const start = useCallback<GatewayActions["start"]>(
    async (port, host, cors) => {
      // Replay the persisted chaos / auth-gate / rate-limit set on start
      // so restarts feel like a resume, not a reset. The persisted map
      // may be stale if the user deleted collections — the gateway
      // silently ignores unknown route keys, which is the behavior we want.
      const saved = mockGateway.savedPreferences ?? null;
      const result = await mockGateway.start({
        port,
        host,
        corsEnabled: cors,
        defaultLatencyMs: saved?.default_latency_ms ?? null,
        latencyOverrides: saved?.latency_overrides,
        errorRate: saved?.error_rate,
        captureBodies: saved?.capture_bodies,
        responseHeaders: saved?.response_headers,
        requiredHeaders: saved?.required_headers,
        rateLimits: saved?.rate_limits,
        exampleOverrides: saved?.example_overrides
      });
      if (result?.running && result.bind_address) {
        toasts.success(
          `Mock server listening at http://${result.bind_address}`
        );
      }
    },
    [mockGateway, toasts]
  );

  const applyOverrides = useCallback<GatewayActions["applyOverrides"]>(
    async (overrides) => {
      const result = await mockGateway.update({ overrides });
      if (result) {
        toasts.info(
          `Applied overrides for ${Object.keys(overrides).length} route(s).`
        );
      }
    },
    [mockGateway, toasts]
  );

  const applyChaos = useCallback<GatewayActions["applyChaos"]>(
    async (defaultLatencyMs, errorRate) => {
      const result = await mockGateway.update({
        defaultLatencyMs,
        errorRate
      });
      if (result) {
        toasts.info(
          errorRate > 0
            ? `Chaos: ${defaultLatencyMs}ms latency, ${Math.round(errorRate * 100)}% errors.`
            : `Latency floor set to ${defaultLatencyMs}ms.`
        );
      }
    },
    [mockGateway, toasts]
  );

  const toggleCaptureBodies = useCallback<
    GatewayActions["toggleCaptureBodies"]
  >(
    async (enabled) => {
      await mockGateway.update({ captureBodies: enabled });
      toasts.info(
        enabled ? "Request body capture on." : "Request body capture off."
      );
    },
    [mockGateway, toasts]
  );

  const applyRateLimits = useCallback<GatewayActions["applyRateLimits"]>(
    async (rules) => {
      const result = await mockGateway.update({ rateLimits: rules });
      if (!result) return;
      const count = Object.keys(rules).length;
      toasts.info(
        count === 0
          ? "Rate limits cleared."
          : `Rate limits applied to ${count} route${count === 1 ? "" : "s"}.`
      );
    },
    [mockGateway, toasts]
  );

  const seedRequiredHeadersFromHints = useCallback<
    GatewayActions["seedRequiredHeadersFromHints"]
  >(async () => {
    const allEndpoints = sidebarCollections.flatMap((c) => c.endpoints);
    const seeded = seedRequiredHeadersFromEndpoints(allEndpoints);
    const count = Object.keys(seeded).length;
    if (count === 0) {
      toasts.warn(
        "No seedable auth hints found in imported endpoints."
      );
      return;
    }
    const result = await mockGateway.update({ requiredHeaders: seeded });
    if (result) {
      toasts.success(
        `Seeded auth gates for ${count} route${count === 1 ? "" : "s"} from OpenAPI security.`
      );
    }
  }, [mockGateway, sidebarCollections, toasts]);

  const replayRequest = useCallback<GatewayActions["replayRequest"]>(
    (entry) => {
      if (!entry.matched_route) return;
      // matched_route format is "METHOD /path"
      const [methodRaw, ...pathParts] = entry.matched_route.split(" ");
      const path = pathParts.join(" ");
      const method = methodRaw.toUpperCase();

      for (const collection of sidebarCollections) {
        const match = collection.endpoints.find(
          (e: CanonicalEndpoint) =>
            e.path === path && e.method.toUpperCase() === method
        );
        if (match) {
          openTab(collection.id, collection.name, match);
          const routeKey = `${method} ${path}`;
          seedTryItDraft(routeKey, {
            query: entry.query ?? "",
            body: entry.request_body ?? "",
            params: undefined,
            headers: undefined
          });
          setMockPanelOpen(false);
          toasts.info(`Loaded ${method} ${path} into Try-it.`);
          return;
        }
      }
      toasts.warn(
        `Could not find a local definition for ${method} ${path}.`
      );
    },
    [openTab, setMockPanelOpen, sidebarCollections, toasts]
  );

  const clearLog = useCallback<GatewayActions["clearLog"]>(async () => {
    if (!mockGateway.clearLog) return;
    await mockGateway.clearLog();
    toasts.info("Request log cleared.");
  }, [mockGateway, toasts]);

  return {
    start,
    applyOverrides,
    applyChaos,
    toggleCaptureBodies,
    applyRateLimits,
    seedRequiredHeadersFromHints,
    clearLog,
    replayRequest
  };
}
