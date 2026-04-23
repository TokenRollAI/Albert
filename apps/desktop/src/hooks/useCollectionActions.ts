import { invoke } from "@tauri-apps/api/core";
import { useCallback } from "react";
import type { SidebarCollection } from "../types";
import type { UseToasts } from "./useToasts";

interface UseCollectionActionsArgs {
  isTauriRuntime: boolean;
  toasts: UseToasts;
  refreshStoredCollections: () => Promise<void>;
  resetTabs: () => void;
}

export interface CollectionActions {
  rename: (collection: SidebarCollection) => Promise<void>;
  exportOne: (collection: SidebarCollection) => Promise<void>;
  exportAll: () => Promise<void>;
  remove: (collection: SidebarCollection) => Promise<void>;
}

/**
 * Sidebar + topbar collection mutations that cross the Tauri boundary.
 * Each handler guards against preview/fallback collections and surfaces a
 * toast on success or failure.
 */
export function useCollectionActions({
  isTauriRuntime,
  toasts,
  refreshStoredCollections,
  resetTabs
}: UseCollectionActionsArgs): CollectionActions {
  const rename = useCallback<CollectionActions["rename"]>(
    async (collection) => {
      if (!isTauriRuntime || collection.origin !== "imported") {
        toasts.warn("Rename requires an imported collection + Tauri runtime.");
        return;
      }
      const next = window.prompt(
        `Rename "${collection.name}" to:`,
        collection.name
      );
      if (!next || !next.trim() || next.trim() === collection.name) return;
      try {
        await invoke<boolean>("rename_collection", {
          collectionId: collection.id,
          newName: next.trim()
        });
        await refreshStoredCollections();
        toasts.success(`Renamed to "${next.trim()}".`);
      } catch (error) {
        toasts.error(`Rename failed: ${String(error)}`);
      }
    },
    [isTauriRuntime, refreshStoredCollections, toasts]
  );

  const exportOne = useCallback<CollectionActions["exportOne"]>(
    async (collection) => {
      if (!isTauriRuntime || collection.origin !== "imported") {
        toasts.warn("Export requires an imported collection + Tauri runtime.");
        return;
      }
      try {
        const json = await invoke<string>("export_collection_json", {
          collectionId: collection.id
        });
        triggerDownload(`${collection.name || "collection"}.json`, json);
        toasts.success(`Exported ${collection.name} as JSON.`);
      } catch (error) {
        toasts.error(`Export failed: ${String(error)}`);
      }
    },
    [isTauriRuntime, toasts]
  );

  const exportAll = useCallback<CollectionActions["exportAll"]>(
    async () => {
      if (!isTauriRuntime) {
        toasts.warn("Export all requires the Tauri runtime.");
        return;
      }
      try {
        const json = await invoke<string>("export_all_collections_json");
        const stamp = new Date().toISOString().slice(0, 10);
        triggerDownload(`albert-bundle-${stamp}.json`, json);
        toasts.success("Exported all collections as a bundle.");
      } catch (error) {
        toasts.error(`Export all failed: ${String(error)}`);
      }
    },
    [isTauriRuntime, toasts]
  );

  const remove = useCallback<CollectionActions["remove"]>(
    async (collection) => {
      if (!isTauriRuntime || collection.origin !== "imported") {
        toasts.warn("Delete requires an imported collection + Tauri runtime.");
        return;
      }
      const confirmed = window.confirm(
        `Delete "${collection.name}" and all its endpoints? This cannot be undone.`
      );
      if (!confirmed) return;
      try {
        await invoke<boolean>("delete_collection", {
          collectionId: collection.id
        });
        await refreshStoredCollections();
        resetTabs();
        toasts.success(`Deleted ${collection.name}.`);
      } catch (error) {
        toasts.error(`Delete failed: ${String(error)}`);
      }
    },
    [isTauriRuntime, refreshStoredCollections, resetTabs, toasts]
  );

  return { rename, exportOne, exportAll, remove };
}

function triggerDownload(filename: string, body: string): void {
  const blob = new Blob([body], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
}
