import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { fallbackSummary } from "../data/fallback";
import type {
  AppBootstrapSummary,
  CanonicalApiCollection,
  ImportedApiCollection,
  StoredCollectionSummary
} from "../types";

export interface UseCollectionData {
  storedCollections: ImportedApiCollection[];
  summary: AppBootstrapSummary;
  runtime: string;
  statusMessage: string;
  setStatusMessage: (msg: string) => void;
  refreshBusy: boolean;
  refreshStoredCollections: () => Promise<void>;
}

/**
 * Own the collection-list state: bootstrap the runtime on mount, load
 * canonical snapshots, and expose a `refresh` callback the rest of the UI
 * can fire after a mutation. Splitting this out leaves App.tsx focused on
 * layout + hook wiring.
 */
export function useCollectionData(): UseCollectionData {
  const [storedCollections, setStoredCollections] = useState<
    ImportedApiCollection[]
  >([]);
  const [summary, setSummary] =
    useState<AppBootstrapSummary>(fallbackSummary);
  const [runtime, setRuntime] = useState("Scaffold");
  const [statusMessage, setStatusMessage] = useState(
    "Ready. Import an OpenAPI spec or cURL to begin."
  );
  const [refreshBusy, setRefreshBusy] = useState(false);

  const loadSnapshots = useCallback(async (tauri: boolean) => {
    if (!tauri) return [] as ImportedApiCollection[];
    const summaries = await invoke<StoredCollectionSummary[]>(
      "list_imported_collections"
    );
    const enriched: ImportedApiCollection[] = [];
    for (const item of summaries) {
      try {
        const full = await invoke<CanonicalApiCollection | null>(
          "load_collection_snapshot",
          { collectionId: item.id }
        );
        if (full) {
          enriched.push({
            ...full,
            created_at: item.created_at,
            updated_at: item.updated_at,
            endpoint_count: item.endpoint_count
          });
        }
      } catch {
        /* skip broken snapshot, continue */
      }
    }
    return enriched;
  }, []);

  const refreshStoredCollections = useCallback(async () => {
    if (runtime !== "Tauri Runtime") return;
    try {
      setRefreshBusy(true);
      const enriched = await loadSnapshots(true);
      setStoredCollections(enriched);
    } catch (error) {
      setStatusMessage(`Failed to refresh collections: ${String(error)}`);
    } finally {
      setRefreshBusy(false);
    }
  }, [loadSnapshots, runtime]);

  useEffect(() => {
    let cancelled = false;
    async function bootstrap() {
      try {
        const data = await invoke<AppBootstrapSummary>("bootstrap_summary");
        if (cancelled) return;
        setSummary(data);
        setRuntime("Tauri Runtime");
        setStatusMessage(
          "Connected to Tauri runtime. Refreshing collections…"
        );
        const enriched = await loadSnapshots(true);
        if (cancelled) return;
        setStoredCollections(enriched);
        setStatusMessage(
          enriched.length === 0
            ? "Connected. No collections imported yet."
            : `Connected. ${enriched.length} collection(s) ready.`
        );
      } catch {
        if (cancelled) return;
        setRuntime("Local Fallback");
        setStatusMessage(
          "Tauri runtime unavailable. Showing local fallback preview."
        );
      }
    }
    bootstrap();
    return () => {
      cancelled = true;
    };
  }, [loadSnapshots]);

  return {
    storedCollections,
    summary,
    runtime,
    statusMessage,
    setStatusMessage,
    refreshBusy,
    refreshStoredCollections
  };
}
