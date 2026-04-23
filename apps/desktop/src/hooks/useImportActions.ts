import { invoke } from "@tauri-apps/api/core";
import { useCallback, useState } from "react";
import { fallbackParsedCollection } from "../data/fallback";
import type {
  CanonicalApiCollection,
  CanonicalEndpoint,
  ImportResult
} from "../types";
import type { UseToasts } from "./useToasts";

interface UseImportActionsArgs {
  isTauriRuntime: boolean;
  toasts: UseToasts;
  setPreviewCollection: (collection: CanonicalApiCollection | null) => void;
  setStatusMessage: (msg: string) => void;
  refreshStoredCollections: () => Promise<void>;
  openTab: (
    collectionId: string,
    collectionName: string,
    endpoint: CanonicalEndpoint
  ) => void;
  onClose: () => void;
}

export interface ImportActions {
  importBusy: "parse" | "import" | null;
  importMessage: string | null;
  setImportMessage: (msg: string | null) => void;
  runImport: (name: string, body: string) => Promise<void>;
  runParsePreview: (name: string, body: string) => Promise<void>;
}

/**
 * Owns the two-phase import flow that used to live inline in App.tsx:
 * - `runParsePreview` runs the parser only and stuffs the result into the
 *   sidebar as a preview collection.
 * - `runImport` parses + persists via the import_api_description Tauri
 *   command, refreshes the sidebar, opens the first endpoint tab, and
 *   dismisses the dialog.
 *
 * Busy + inline message state is kept here so the dialog can react to it
 * without needing a bunch of props shuttled through App.tsx.
 */
export function useImportActions({
  isTauriRuntime,
  toasts,
  setPreviewCollection,
  setStatusMessage,
  refreshStoredCollections,
  openTab,
  onClose
}: UseImportActionsArgs): ImportActions {
  const [importBusy, setImportBusy] = useState<"parse" | "import" | null>(null);
  const [importMessage, setImportMessage] = useState<string | null>(null);

  const runImport = useCallback<ImportActions["runImport"]>(
    async (name, body) => {
      if (!isTauriRuntime) {
        setImportMessage(
          "SQLite import requires the Tauri runtime. Use Parse Preview instead."
        );
        return;
      }
      try {
        setImportBusy("import");
        setImportMessage(null);
        const result = await invoke<ImportResult>("import_api_description", {
          body,
          name: name || null
        });
        const collection = await invoke<CanonicalApiCollection>(
          "parse_api_description",
          { body, name: name || null }
        );
        setPreviewCollection(null);
        await refreshStoredCollections();
        setStatusMessage(
          `Imported ${result.endpoint_count} endpoint(s) into ${result.database_url}.`
        );
        toasts.success(
          `Imported ${result.endpoint_count} endpoint(s) as "${result.collection_name}".`
        );
        onClose();
        const first = collection.endpoints[0];
        if (first) {
          openTab(result.collection_id, result.collection_name, first);
        }
      } catch (error) {
        const message = `Import failed: ${String(error)}`;
        setImportMessage(message);
        toasts.error(message);
      } finally {
        setImportBusy(null);
      }
    },
    [
      isTauriRuntime,
      onClose,
      openTab,
      refreshStoredCollections,
      setPreviewCollection,
      setStatusMessage,
      toasts
    ]
  );

  const runParsePreview = useCallback<ImportActions["runParsePreview"]>(
    async (name, body) => {
      try {
        setImportBusy("parse");
        setImportMessage(null);
        if (!isTauriRuntime) {
          setPreviewCollection({
            ...fallbackParsedCollection,
            name: name || fallbackParsedCollection.name
          });
          setImportMessage(
            "Preview uses local fallback because Tauri runtime is unavailable."
          );
          setStatusMessage("Preview populated from local fallback.");
          return;
        }
        const collection = await invoke<CanonicalApiCollection>(
          "parse_api_description",
          { body, name: name || null }
        );
        setPreviewCollection(collection);
        setStatusMessage(
          `Parsed ${collection.endpoints.length} endpoint(s) from ${collection.source}.`
        );
        setImportMessage(
          `Parsed ${collection.endpoints.length} endpoint(s). Review in the sidebar; import to persist.`
        );
      } catch (error) {
        setImportMessage(`Parse failed: ${String(error)}`);
      } finally {
        setImportBusy(null);
      }
    },
    [isTauriRuntime, setPreviewCollection, setStatusMessage]
  );

  return {
    importBusy,
    importMessage,
    setImportMessage,
    runImport,
    runParsePreview
  };
}
