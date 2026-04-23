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
import { useCollectionActions } from "./hooks/useCollectionActions";
import { useCollectionData } from "./hooks/useCollectionData";
import { useEndpointTabs } from "./hooks/useEndpointTabs";
import { useGatewayActions } from "./hooks/useGatewayActions";
import { useImportActions } from "./hooks/useImportActions";
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
  CanonicalApiCollection,
  CanonicalEndpoint,
  EndpointTab,
  ExampleKind,
  MockExample,
  MockExampleKind,
  SidebarCollection
} from "./types";

function App() {
  const { theme, toggleTheme } = useTheme();

  const {
    storedCollections,
    summary,
    runtime,
    statusMessage,
    setStatusMessage,
    refreshBusy,
    refreshStoredCollections
  } = useCollectionData();

  const [previewCollection, setPreviewCollection] =
    useState<CanonicalApiCollection | null>(null);

  const [importOpen, setImportOpen] = useState(false);

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


  const handleOpenEndpoint = useCallback(
    (collection: SidebarCollection, endpoint: CanonicalEndpoint) => {
      openTab(collection.id, collection.name, endpoint);
    },
    [openTab]
  );

  const importActions = useImportActions({
    isTauriRuntime,
    toasts,
    setPreviewCollection,
    setStatusMessage,
    refreshStoredCollections,
    openTab,
    onClose: () => setImportOpen(false)
  });

  const collectionActions = useCollectionActions({
    isTauriRuntime,
    toasts,
    refreshStoredCollections,
    resetTabs
  });

  const gatewayActions = useGatewayActions({
    mockGateway,
    sidebarCollections,
    openTab,
    setMockPanelOpen,
    toasts
  });

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
        onExportAll={collectionActions.exportAll}
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
          onExportCollection={collectionActions.exportOne}
          onDeleteCollection={collectionActions.remove}
          onRenameCollection={collectionActions.rename}
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
                <UrlBar
                  tab={activeTab}
                  disabled={false}
                  baseUrl={
                    mockGateway.status.running &&
                    mockGateway.status.bind_address
                      ? `http://${mockGateway.status.bind_address}`
                      : null
                  }
                />
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
          if (importActions.importBusy) return;
          setImportOpen(false);
          importActions.setImportMessage(null);
        }}
        onParse={importActions.runParsePreview}
        onImport={importActions.runImport}
        canImport={isTauriRuntime}
        canFetch={isTauriRuntime}
        busy={importActions.importBusy}
        message={importActions.importMessage}
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
        onStart={gatewayActions.start}
        onStop={mockGateway.stop}
        onApplyOverrides={gatewayActions.applyOverrides}
        onApplyChaos={gatewayActions.applyChaos}
        onToggleCaptureBodies={gatewayActions.toggleCaptureBodies}
        onReplayRequest={gatewayActions.replayRequest}
      />

      <ProvidersPanel
        open={providersOpen}
        onClose={() => setProvidersOpen(false)}
        draft={providerDraft}
        apiKeyOverride={apiKeyOverride}
        connected={isTauriRuntime}
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
