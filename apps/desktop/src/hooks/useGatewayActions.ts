import { useCallback } from "react";
import { seedTryItDraft } from "./useTryItDraft";
import type {
  CanonicalEndpoint,
  MockExampleKind,
  RateLimitRule,
  RequestLogEntry,
  SidebarCollection
} from "../types";
import type { UseToasts } from "./useToasts";

interface UseGatewayActionsArgs {
  mockGateway: {
    start: (args: {
      host: string;
      port: number;
      corsEnabled: boolean;
    }) => Promise<{ running: boolean; bind_address?: string | null } | null>;
    update: (args: {
      overrides?: Record<string, MockExampleKind>;
      defaultLatencyMs?: number | null;
      errorRate?: number;
      captureBodies?: boolean;
      rateLimits?: Record<string, RateLimitRule>;
    }) => Promise<unknown>;
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
      const result = await mockGateway.start({
        port,
        host,
        corsEnabled: cors
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

  return {
    start,
    applyOverrides,
    applyChaos,
    toggleCaptureBodies,
    applyRateLimits,
    replayRequest
  };
}
