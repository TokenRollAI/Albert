import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { EndpointTabs } from "./components/EndpointTabs";
import { ImportDialog } from "./components/ImportDialog";
import { MockServerPanel } from "./components/MockServerPanel";
import { PromptPreviewModal } from "./components/PromptPreviewModal";
import { ProvidersPanel } from "./components/ProvidersPanel";
import { ShortcutsOverlay } from "./components/ShortcutsOverlay";
import { TryItPanel } from "./components/TryItPanel";
import { RequestPanel } from "./components/RequestPanel";
import { ResponsePane } from "./components/ResponsePane";
import { Sidebar, type SidebarHandle } from "./components/Sidebar";
import { StatusBar } from "./components/StatusBar";
import { ToastHost } from "./components/ToastHost";
import { TopBar } from "./components/TopBar";
import { UrlBar } from "./components/UrlBar";
import { WorkbenchEmpty } from "./components/WorkbenchEmpty";
import {
  fallbackParsedCollection,
  fallbackSummary,
  sampleImportText
} from "./data/fallback";
import { useAiActions, type PromptPreview } from "./hooks/useAiActions";
import { useEndpointTabs } from "./hooks/useEndpointTabs";
import {
  useKeyboardShortcuts,
  type ShortcutBinding
} from "./hooks/useKeyboardShortcuts";
import { useMockGateway } from "./hooks/useMockGateway";
import { useProviderDraft } from "./hooks/useProviderDraft";
import { useTheme } from "./hooks/useTheme";
import { useToasts } from "./hooks/useToasts";
import { seedTryItDraft } from "./hooks/useTryItDraft";
import type {
  AppBootstrapSummary,
  CanonicalApiCollection,
  CanonicalEndpoint,
  EndpointTab,
  ExampleKind,
  ImportResult,
  MockExample,
  MockExampleKind,
  RequestLogEntry,
  SidebarCollection,
  StoredCollectionSummary
} from "./types";

function App() {
  const { theme, toggleTheme } = useTheme();

  const [summary, setSummary] = useState<AppBootstrapSummary>(fallbackSummary);
  const [runtime, setRuntime] = useState("Scaffold");
  const [statusMessage, setStatusMessage] = useState(
    "Ready. Import an OpenAPI spec or cURL to begin."
  );

  const [storedCollections, setStoredCollections] = useState<
    CanonicalApiCollection[]
  >([]);
  const [previewCollection, setPreviewCollection] =
    useState<CanonicalApiCollection | null>(null);
  const [refreshBusy, setRefreshBusy] = useState(false);

  const [importOpen, setImportOpen] = useState(false);
  const [importBusy, setImportBusy] = useState<"parse" | "import" | null>(null);
  const [importMessage, setImportMessage] = useState<string | null>(null);

  const [mockPanelOpen, setMockPanelOpen] = useState(false);
  const [providersOpen, setProvidersOpen] = useState(false);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const [promptPreview, setPromptPreview] = useState<PromptPreview | null>(null);
  const [promptPreviewOpen, setPromptPreviewOpen] = useState(false);
  const [promptPreviewLoading, setPromptPreviewLoading] = useState(false);
  const [promptPreviewError, setPromptPreviewError] = useState<string | null>(
    null
  );
  const toasts = useToasts();
  const sidebarRef = useRef<SidebarHandle | null>(null);

  const {
    tabs,
    activeId,
    activeTab,
    openTab,
    closeTab,
    activateTab,
    setInspector,
    setExample,
    updateEndpointExample,
    resetTabs
  } = useEndpointTabs();

  const {
    draft: providerDraft,
    apiKeyOverride,
    update: updateProvider,
    setApiKeyOverride
  } = useProviderDraft();

  const isTauriRuntime = runtime === "Tauri Runtime";

  const mockGateway = useMockGateway({ enabled: isTauriRuntime });

  const shortcutBindings = useMemo<ShortcutBinding[]>(
    () => [
      {
        combo: "Mod+K",
        description: "Focus collection search",
        handler: () => sidebarRef.current?.focusSearch()
      },
      {
        combo: "Mod+.",
        description: "Toggle mock server panel",
        handler: () => setMockPanelOpen((prev) => !prev)
      },
      {
        combo: "Mod+i",
        description: "Open import dialog",
        handler: () => setImportOpen(true)
      },
      {
        combo: "Mod+Shift+p",
        description: "Open providers panel",
        handler: () => setProvidersOpen((prev) => !prev)
      },
      {
        combo: "Mod+w",
        description: "Close active endpoint tab",
        handler: () => {
          if (activeId) closeTab(activeId);
        }
      },
      {
        combo: "Mod+/",
        description: "Show keyboard shortcuts",
        handler: () => setShortcutsOpen((prev) => !prev)
      }
    ],
    [activeId, closeTab]
  );

  useKeyboardShortcuts(shortcutBindings);

  const sidebarCollections: SidebarCollection[] = useMemo(() => {
    const result: SidebarCollection[] = [];
    if (previewCollection) {
      result.push({
        id: `preview:${previewCollection.id}`,
        name: `${previewCollection.name}  (preview)`,
        origin: "preview",
        source: previewCollection.source,
        endpoints: previewCollection.endpoints
      });
    }
    for (const collection of storedCollections) {
      result.push({
        id: collection.id,
        name: collection.name,
        origin: "imported",
        source: collection.source,
        endpoints: collection.endpoints
      });
    }
    if (result.length === 0) {
      result.push({
        id: fallbackParsedCollection.id,
        name: fallbackParsedCollection.name,
        origin: "fallback",
        source: fallbackParsedCollection.source,
        endpoints: fallbackParsedCollection.endpoints
      });
    }
    return result;
  }, [previewCollection, storedCollections]);

  const loadSnapshots = useCallback(
    async (tauri: boolean) => {
      if (!tauri) {
        return [] as CanonicalApiCollection[];
      }
      const summaries = await invoke<StoredCollectionSummary[]>(
        "list_imported_collections"
      );
      const enriched: CanonicalApiCollection[] = [];
      for (const item of summaries) {
        try {
          const full = await invoke<CanonicalApiCollection | null>(
            "load_collection_snapshot",
            { collectionId: item.id }
          );
          if (full) {
            enriched.push(full);
          }
        } catch {
          /* skip one bad collection, continue */
        }
      }
      return enriched;
    },
    []
  );

  const refreshStoredCollections = useCallback(async () => {
    if (!isTauriRuntime) {
      return;
    }
    try {
      setRefreshBusy(true);
      const enriched = await loadSnapshots(true);
      setStoredCollections(enriched);
    } catch (error) {
      setStatusMessage(`Failed to refresh collections: ${String(error)}`);
    } finally {
      setRefreshBusy(false);
    }
  }, [isTauriRuntime, loadSnapshots]);

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

  const handleOpenEndpoint = useCallback(
    (collection: SidebarCollection, endpoint: CanonicalEndpoint) => {
      openTab(collection.id, collection.name, endpoint);
    },
    [openTab]
  );

  const handleImport = useCallback(
    async (name: string, body: string) => {
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
        setImportMessage(null);
        setImportOpen(false);
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
    [isTauriRuntime, openTab, refreshStoredCollections, toasts]
  );

  const handleParsePreview = useCallback(
    async (name: string, body: string) => {
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
    [isTauriRuntime]
  );

  const handleStartGateway = useCallback(
    async (port: number, host: string, cors: boolean) => {
      const result = await mockGateway.start({
        port,
        host,
        corsEnabled: cors
      });
      if (result?.running && result.bind_address) {
        toasts.success(`Mock server listening at http://${result.bind_address}`);
      }
    },
    [mockGateway, toasts]
  );

  const handleApplyOverrides = useCallback(
    async (overrides: Record<string, MockExampleKind>) => {
      const result = await mockGateway.update({ overrides });
      if (result) {
        toasts.info(
          `Applied overrides for ${Object.keys(overrides).length} route(s).`
        );
      }
    },
    [mockGateway, toasts]
  );

  const handleRenameCollection = useCallback(
    async (collection: SidebarCollection) => {
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

  const handleExportAll = useCallback(async () => {
    if (!isTauriRuntime) {
      toasts.warn("Export all requires the Tauri runtime.");
      return;
    }
    try {
      const json = await invoke<string>("export_all_collections_json");
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `albert-bundle-${new Date()
        .toISOString()
        .slice(0, 10)}.json`;
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      URL.revokeObjectURL(url);
      toasts.success("Exported all collections as a bundle.");
    } catch (error) {
      toasts.error(`Export all failed: ${String(error)}`);
    }
  }, [isTauriRuntime, toasts]);

  const handleDeleteCollection = useCallback(
    async (collection: SidebarCollection) => {
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

  const handleExportCollection = useCallback(
    async (collection: SidebarCollection) => {
      if (!isTauriRuntime || collection.origin !== "imported") {
        toasts.warn("Export requires an imported collection + Tauri runtime.");
        return;
      }
      try {
        const json = await invoke<string>("export_collection_json", {
          collectionId: collection.id
        });
        const blob = new Blob([json], { type: "application/json" });
        const url = URL.createObjectURL(blob);
        const link = document.createElement("a");
        link.href = url;
        link.download = `${collection.name || "collection"}.json`;
        document.body.appendChild(link);
        link.click();
        document.body.removeChild(link);
        URL.revokeObjectURL(url);
        toasts.success(`Exported ${collection.name} as JSON.`);
      } catch (error) {
        toasts.error(`Export failed: ${String(error)}`);
      }
    },
    [isTauriRuntime, toasts]
  );

  const handleApplyChaos = useCallback(
    async (defaultLatencyMs: number, errorRate: number) => {
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

  const handleReplayRequest = useCallback(
    (entry: RequestLogEntry) => {
      if (!entry.matched_route) return;
      // matched_route format is "METHOD /path"
      const [methodRaw, ...pathParts] = entry.matched_route.split(" ");
      const path = pathParts.join(" ");
      const method = methodRaw.toUpperCase();

      // Find the collection + endpoint whose canonical route matches.
      for (const collection of sidebarCollections) {
        const match = collection.endpoints.find(
          (e) =>
            e.path === path && e.method.toUpperCase() === method
        );
        if (match) {
          openTab(collection.id, collection.name, match);
          // Seed the Try-it draft for this endpoint with the replayed
          // request's query + body. Path params are inferred from the
          // matched_route template by scanning the live path; we can't
          // always recover them 100% (we'd need the original path from
          // the request), so leave them blank for the user to fill.
          const routeKey = `${method} ${path}`;
          seedTryItDraft(routeKey, {
            query: entry.query ?? "",
            body: entry.request_body ?? "",
            // keep existing params/headers
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
    [sidebarCollections, openTab, toasts]
  );

  const handleToggleCaptureBodies = useCallback(
    async (enabled: boolean) => {
      await mockGateway.update({ captureBodies: enabled });
      toasts.info(enabled ? "Request body capture on." : "Request body capture off.");
    },
    [mockGateway, toasts]
  );

  const aiActions = useAiActions({
    isTauriRuntime,
    providerDraft,
    apiKeyOverride,
    toasts,
    setStatusMessage,
    refreshStoredCollections,
    updateEndpointExample,
    promptPreviewSetters: useMemo(
      () => ({
        setPreview: setPromptPreview,
        setOpen: setPromptPreviewOpen,
        setLoading: setPromptPreviewLoading,
        setError: setPromptPreviewError
      }),
      []
    )
  });

  const workspaceName =
    activeTab?.collectionName ??
    sidebarCollections[0]?.name ??
    "Workspace";

  return (
    <div className="shell">
      <TopBar
        workspace={workspaceName}
        theme={theme}
        onToggleTheme={toggleTheme}
        onImportClick={() => setImportOpen(true)}
        onMockServerClick={() => setMockPanelOpen(true)}
        onProvidersClick={() => setProvidersOpen(true)}
        onExportAll={handleExportAll}
        canExportAll={isTauriRuntime && storedCollections.length > 0}
        gatewayRunning={mockGateway.status.running}
        gatewayBind={mockGateway.status.bind_address ?? null}
      />

      <div className="shell__main">
        <Sidebar
          ref={sidebarRef}
          collections={sidebarCollections}
          activeTabId={activeId}
          onOpenEndpoint={handleOpenEndpoint}
          onImportClick={() => setImportOpen(true)}
          onRefresh={() => {
            resetTabs();
            setPreviewCollection(null);
            refreshStoredCollections();
          }}
          onExportCollection={handleExportCollection}
          onDeleteCollection={handleDeleteCollection}
          onRenameCollection={handleRenameCollection}
          busy={refreshBusy}
        />

        <main className="workbench">
          <EndpointTabs
            tabs={tabs}
            activeId={activeId}
            onActivate={activateTab}
            onClose={closeTab}
            onNew={() => setImportOpen(true)}
          />

          {activeTab ? (
            <div className="workbench__grid">
              <div className="workbench__editor">
                <UrlBar tab={activeTab} disabled />
                <RequestPanel
                  tab={activeTab}
                  onSelectInspector={(key) => setInspector(activeTab.id, key)}
                />
              </div>
              <div className="workbench__response">
                <ResponsePane
                  tab={activeTab}
                  onSelectExample={(kind) => setExample(activeTab.id, kind)}
                  connected={isTauriRuntime}
                  provider={providerDraft}
                  apiKeyOverride={apiKeyOverride}
                  onGenerate={aiActions.generate}
                  onGenerateAll={aiActions.generateAll}
                  onPreviewPrompt={aiActions.previewPrompt}
                  onSaveExample={aiActions.saveExample}
                />
                <TryItPanel
                  tab={activeTab}
                  baseUrl={
                    mockGateway.status.running &&
                    mockGateway.status.bind_address
                      ? `http://${mockGateway.status.bind_address}`
                      : null
                  }
                />
              </div>
            </div>
          ) : (
            <WorkbenchEmpty onImportClick={() => setImportOpen(true)} />
          )}
        </main>
      </div>

      <StatusBar
        runtime={runtime}
        collectionsCount={storedCollections.length}
        message={statusMessage}
        phase={summary.current_phase}
        mockRunning={mockGateway.status.running}
        mockBind={mockGateway.status.bind_address ?? null}
      />

      <ImportDialog
        open={importOpen}
        onClose={() => {
          if (importBusy) return;
          setImportOpen(false);
          setImportMessage(null);
        }}
        onParse={handleParsePreview}
        onImport={handleImport}
        canImport={isTauriRuntime}
        canFetch={isTauriRuntime}
        busy={importBusy}
        message={importMessage}
        initialName="Albert Example API"
        initialBody={sampleImportText}
      />

      <MockServerPanel
        open={mockPanelOpen}
        onClose={() => setMockPanelOpen(false)}
        connected={isTauriRuntime}
        status={mockGateway.status}
        busy={mockGateway.busy}
        error={mockGateway.error}
        requests={mockGateway.requests}
        savedPreferences={mockGateway.savedPreferences}
        onStart={handleStartGateway}
        onStop={mockGateway.stop}
        onApplyOverrides={handleApplyOverrides}
        onApplyChaos={handleApplyChaos}
        onToggleCaptureBodies={handleToggleCaptureBodies}
        onReplayRequest={handleReplayRequest}
      />

      <ProvidersPanel
        open={providersOpen}
        onClose={() => setProvidersOpen(false)}
        draft={providerDraft}
        apiKeyOverride={apiKeyOverride}
        onUpdateDraft={updateProvider}
        onUpdateApiKey={setApiKeyOverride}
      />

      <PromptPreviewModal
        open={promptPreviewOpen}
        preview={promptPreview}
        loading={promptPreviewLoading}
        error={promptPreviewError}
        onClose={() => {
          setPromptPreviewOpen(false);
        }}
      />

      <ShortcutsOverlay
        open={shortcutsOpen}
        bindings={shortcutBindings}
        onClose={() => setShortcutsOpen(false)}
      />

      <ToastHost toasts={toasts.toasts} onDismiss={toasts.dismiss} />
    </div>
  );
}

export default App;
