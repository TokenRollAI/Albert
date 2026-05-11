import { invoke } from "@tauri-apps/api/core";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type PointerEvent as ReactPointerEvent
} from "react";
import { CommandPalette, type CommandItem } from "./components/CommandPalette";
import { EndpointTabs } from "./components/EndpointTabs";
import { ImportDialog } from "./components/ImportDialog";
import {
  ImportReportPanel,
  type ImportReport
} from "./components/ImportReportPanel";
import { MockServerPanel } from "./components/MockServerPanel";
import { buildCurlCommand } from "./components/UrlBar";
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
import { WorkspacePanel } from "./components/WorkspacePanel";
import {
  fallbackParsedCollection,
  fallbackSummary,
  sampleImportText
} from "./data/fallback";
import { useAiActions, type PromptPreview } from "./hooks/useAiActions";
import { useAppDrawers } from "./hooks/useAppDrawers";
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
import { importChangeGenerationContext } from "./lib/importReportContext";
import type {
  CanonicalApiCollection,
  CanonicalEndpoint,
  EndpointTab,
  ExampleKind,
  MockExample,
  MockExampleKind,
  SidebarCollection
} from "./types";

const SIDEBAR_WIDTH_KEY = "albert.layout.sidebarWidth";
const RESPONSE_WIDTH_KEY = "albert.layout.responseWidth";
const DEFAULT_SIDEBAR_WIDTH = 268;
const DEFAULT_RESPONSE_WIDTH = 400;

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function loadLayoutWidth(key: string, fallback: number, min: number, max: number) {
  if (typeof window === "undefined") return fallback;
  const value = Number(window.localStorage.getItem(key));
  return Number.isFinite(value) ? clamp(value, min, max) : fallback;
}

function App() {
  const { theme, toggleTheme } = useTheme();
  const [sidebarWidth, setSidebarWidth] = useState(() =>
    loadLayoutWidth(SIDEBAR_WIDTH_KEY, DEFAULT_SIDEBAR_WIDTH, 220, 560)
  );
  const [responseWidth, setResponseWidth] = useState(() =>
    loadLayoutWidth(RESPONSE_WIDTH_KEY, DEFAULT_RESPONSE_WIDTH, 320, 760)
  );

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
  const [latestImportReport, setLatestImportReport] =
    useState<ImportReport | null>(null);

  const drawers = useAppDrawers();
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
    resetTabs,
    restoreTabs,
    reorderTabs
  } = useEndpointTabs();

  // Reopen persisted tabs once collections finish loading. The hook
  // guards against double-apply (it bails when tabs already exist), so
  // running this effect on every storedCollections change is harmless.
  useEffect(() => {
    if (storedCollections.length > 0) {
      restoreTabs(storedCollections);
    }
  }, [storedCollections, restoreTabs]);

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
        combo: "Mod+p",
        description: "Open command palette",
        handler: () => drawers.palette.toggle()
      },
      {
        combo: "Mod+.",
        description: "Toggle mock server panel",
        handler: () => drawers.mockServer.toggle()
      },
      {
        combo: "Mod+i",
        description: "Open import dialog",
        handler: () => drawers.import.open$()
      },
      {
        combo: "Mod+Shift+i",
        description: "Open import report",
        handler: () => drawers.importReport.open$()
      },
      {
        combo: "Mod+Shift+w",
        description: "Open workspace collections",
        handler: () => drawers.workspace.toggle()
      },
      {
        combo: "Mod+Shift+p",
        description: "Open providers panel",
        handler: () => drawers.providers.toggle()
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
        handler: () => drawers.shortcuts.toggle()
      },
      {
        // Mod+Tab isn't capturable in browsers (reserved for window
        // switching), so we use Mod+Alt+Right/Left to cycle tabs. On Mac
        // this is Cmd+Opt+Arrow, which matches Safari/Chrome tab cycling.
        combo: "Mod+alt+arrowright",
        description: "Next endpoint tab",
        handler: () => {
          if (tabs.length <= 1 || !activeId) return;
          const idx = tabs.findIndex((t) => t.id === activeId);
          if (idx === -1) return;
          activateTab(tabs[(idx + 1) % tabs.length].id);
        }
      },
      {
        combo: "Mod+alt+arrowleft",
        description: "Previous endpoint tab",
        handler: () => {
          if (tabs.length <= 1 || !activeId) return;
          const idx = tabs.findIndex((t) => t.id === activeId);
          if (idx === -1) return;
          activateTab(tabs[(idx - 1 + tabs.length) % tabs.length].id);
        }
      },
      ...[1, 2, 3, 4, 5, 6, 7, 8, 9].map<ShortcutBinding>((n) => ({
        combo: `Mod+${n}`,
        description: `Jump to tab ${n}`,
        handler: () => {
          const target = tabs[n - 1];
          if (target) activateTab(target.id);
        }
      }))
    ],
    // drawers, sidebarRef identities are stable across renders, so tracking
    // them isn't necessary — only activeId / closeTab actually change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [activeId, closeTab, tabs, activateTab]
  );

  useKeyboardShortcuts(shortcutBindings);

  useEffect(() => {
    window.localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidth));
  }, [sidebarWidth]);

  useEffect(() => {
    window.localStorage.setItem(RESPONSE_WIDTH_KEY, String(responseWidth));
  }, [responseWidth]);

  const startSidebarResize = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      event.preventDefault();
      const startX = event.clientX;
      const startWidth = sidebarWidth;

      function handleMove(moveEvent: PointerEvent) {
        setSidebarWidth(
          clamp(startWidth + moveEvent.clientX - startX, 220, 560)
        );
      }

      function handleUp() {
        document.removeEventListener("pointermove", handleMove);
        document.removeEventListener("pointerup", handleUp);
        document.body.classList.remove("is-resizing-pane");
      }

      document.body.classList.add("is-resizing-pane");
      document.addEventListener("pointermove", handleMove);
      document.addEventListener("pointerup", handleUp, { once: true });
    },
    [sidebarWidth]
  );

  const startResponseResize = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      event.preventDefault();
      const startX = event.clientX;
      const startWidth = responseWidth;

      function handleMove(moveEvent: PointerEvent) {
        setResponseWidth(
          clamp(startWidth - (moveEvent.clientX - startX), 320, 760)
        );
      }

      function handleUp() {
        document.removeEventListener("pointermove", handleMove);
        document.removeEventListener("pointerup", handleUp);
        document.body.classList.remove("is-resizing-pane");
      }

      document.body.classList.add("is-resizing-pane");
      document.addEventListener("pointermove", handleMove);
      document.addEventListener("pointerup", handleUp, { once: true });
    },
    [responseWidth]
  );

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
        endpoints: collection.endpoints,
        createdAt: collection.created_at,
        updatedAt: collection.updated_at,
        endpointCount: collection.endpoint_count
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

  const paletteItems = useMemo<CommandItem[]>(() => {
    const items: CommandItem[] = [];
    // Active-route actions float to the top when a tab is open so the
    // palette can double as a workflow shortcut, not just a navigator.
    if (activeTab) {
      const routeLabel = `${activeTab.method.toUpperCase()} ${activeTab.endpoint.path}`;
      items.push({
        kind: "action",
        id: "action:active:copy-curl",
        label: `Copy cURL for ${routeLabel}`,
        subtitle: mockGateway.status.bind_address
          ? `targets http://${mockGateway.status.bind_address}`
          : "targets https://api.example.com placeholder",
        run: () => {
          const command = buildCurlCommand(
            activeTab,
            mockGateway.status.bind_address
              ? `http://${mockGateway.status.bind_address}`
              : null
          );
          void navigator.clipboard?.writeText(command).then(
            () => toasts.success("cURL copied to clipboard."),
            () => toasts.warn("Clipboard access was denied.")
          );
        }
      });
      items.push({
        kind: "action",
        id: "action:active:seed-tryit",
        label: `Jump to Try-it for ${routeLabel}`,
        subtitle: "scrolls the response pane into view",
        run: () => {
          // No-op beyond "make the active tab visible" — the tab is
          // already open; palette close is enough.
        }
      });
    }

    for (const collection of sidebarCollections) {
      for (const endpoint of collection.endpoints) {
        items.push({
          kind: "endpoint",
          id: `${collection.id}::${endpoint.method.toUpperCase()}:${endpoint.path}`,
          label: `${endpoint.method.toUpperCase()} ${endpoint.path}`,
          subtitle:
            endpoint.summary ??
            (collection.name ? collection.name : undefined),
          collectionId: collection.id,
          endpointMethod: endpoint.method,
          endpointPath: endpoint.path
        });
      }
    }
    items.push({
      kind: "action",
      id: "action:toggle-mock",
      label: mockGateway.status.running ? "Stop mock server" : "Start mock server",
      subtitle: "Mock Server runtime",
      run: () => {
        if (mockGateway.status.running) {
          void mockGateway.stop();
        } else {
          drawers.mockServer.open$();
        }
      }
    });
    items.push({
      kind: "action",
      id: "action:open-mock-panel",
      label: "Open Mock Server drawer",
      subtitle: "⌘.",
      run: () => drawers.mockServer.open$()
    });
    items.push({
      kind: "action",
      id: "action:open-workspace",
      label: "Open Workspace collections",
      subtitle: "⌘⇧W",
      run: () => drawers.workspace.open$()
    });
    items.push({
      kind: "action",
      id: "action:open-import",
      label: "Import OpenAPI / cURL",
      subtitle: "⌘I",
      run: () => drawers.import.open$()
    });
    items.push({
      kind: "action",
      id: "action:open-import-report",
      label: "Open Import report",
      subtitle: "⌘⇧I",
      run: () => drawers.importReport.open$()
    });
    items.push({
      kind: "action",
      id: "action:open-providers",
      label: "Open Providers drawer",
      subtitle: "⌘⇧P",
      run: () => drawers.providers.open$()
    });
    items.push({
      kind: "action",
      id: "action:toggle-theme",
      label: theme === "dark" ? "Switch to light theme" : "Switch to dark theme",
      run: toggleTheme
    });
    items.push({
      kind: "action",
      id: "action:show-shortcuts",
      label: "Show keyboard shortcuts",
      subtitle: "⌘/",
      run: () => drawers.shortcuts.open$()
    });
    return items;
  }, [activeTab, sidebarCollections, mockGateway, drawers, theme, toggleTheme, toasts]);

  const runPaletteItem = useCallback(
    (item: CommandItem) => {
      drawers.palette.close();
      if (item.kind === "endpoint") {
        const collection = sidebarCollections.find(
          (c) => c.id === item.collectionId
        );
        const endpoint = collection?.endpoints.find(
          (e) =>
            e.method.toUpperCase() === item.endpointMethod.toUpperCase() &&
            e.path === item.endpointPath
        );
        if (collection && endpoint) {
          openTab(collection.id, collection.name, endpoint);
        }
        return;
      }
      item.run();
    },
    [drawers, openTab, sidebarCollections]
  );

  const importActions = useImportActions({
    isTauriRuntime,
    toasts,
    setPreviewCollection,
    setStatusMessage,
    refreshStoredCollections,
    onImportComplete: (result, collection) => {
      setLatestImportReport({ result, collection });
      drawers.importReport.open$();
    },
    openTab,
    onClose: () => drawers.import.close()
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
    setMockPanelOpen: drawers.mockServer.set,
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
        onWorkspaceClick={() => drawers.workspace.open$()}
        onImportClick={() => drawers.import.open$()}
        onMockServerClick={() => drawers.mockServer.open$()}
        onProvidersClick={() => drawers.providers.open$()}
        onExportAll={collectionActions.exportAll}
        canExportAll={isTauriRuntime && storedCollections.length > 0}
        gatewayRunning={mockGateway.status.running}
        gatewayBind={mockGateway.status.bind_address ?? null}
      />

      <div
        className="shell__main"
        style={
          {
            "--sidebar-w": `${sidebarWidth}px`,
            "--response-w": `${responseWidth}px`
          } as CSSProperties
        }
      >
        <Sidebar
          ref={sidebarRef}
          collections={sidebarCollections}
          activeTabId={activeId}
          onOpenEndpoint={handleOpenEndpoint}
          onImportClick={() => drawers.import.open$()}
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
        <div
          className="shell__resizer"
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize collections sidebar"
          title="Drag to resize collections"
          onPointerDown={startSidebarResize}
        />

        <main className="workbench">
          <EndpointTabs
            tabs={tabs}
            activeId={activeId}
            onActivate={activateTab}
            onClose={closeTab}
            onNew={() => drawers.import.open$()}
            onReorder={reorderTabs}
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
              <div
                className="workbench__resizer"
                role="separator"
                aria-orientation="vertical"
                aria-label="Resize mock response panel"
                title="Drag to resize mock response"
                onPointerDown={startResponseResize}
              />
              <div className="workbench__response">
                <ResponsePane
                  tab={activeTab}
                  onSelectExample={(kind) => setExample(activeTab.id, kind)}
                  connected={isTauriRuntime}
                  provider={providerDraft}
                  apiKeyOverride={apiKeyOverride}
                  onGenerate={aiActions.generate}
                  onGenerateWithContext={aiActions.generateWithContext}
                  onGenerateAll={aiActions.generateAll}
                  onGenerateAllWithContexts={aiActions.generateAllWithContexts}
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
                  connected={isTauriRuntime}
                  onSaveResponseAsExample={aiActions.saveExample}
                  canGenerateFromCache={
                    Boolean(providerDraft.base_url) && Boolean(providerDraft.model)
                  }
                  onGenerateFromCache={(tab, kind, context) =>
                    aiActions.generateWithContext(tab, kind, true, context)
                  }
                  onPreviewPromptFromCache={(tab, kind, context) =>
                    aiActions.previewPrompt(tab, kind, context)
                  }
                  requestCacheRoutingEnabled={
                    mockGateway.status.config.use_request_cache ?? false
                  }
                  onReloadRequestCache={gatewayActions.reloadRequestCache}
                />
              </div>
            </div>
          ) : (
            <WorkbenchEmpty onImportClick={drawers.import.open$} />
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

      <WorkspacePanel
        open={drawers.workspace.open}
        collections={sidebarCollections}
        connected={isTauriRuntime}
        onClose={drawers.workspace.close}
        onImportClick={() => {
          drawers.workspace.close();
          drawers.import.open$();
        }}
        onOpenEndpoint={(collection, endpoint) => {
          openTab(collection.id, collection.name, endpoint);
          drawers.workspace.close();
        }}
        onRefresh={() => {
          resetTabs();
          setPreviewCollection(null);
          refreshStoredCollections();
        }}
        onRenameCollection={collectionActions.rename}
        onExportCollection={collectionActions.exportOne}
        onDeleteCollection={collectionActions.remove}
        busy={refreshBusy}
      />

      <ImportReportPanel
        open={drawers.importReport.open}
        report={latestImportReport}
        onClose={drawers.importReport.close}
        onImportClick={() => {
          drawers.importReport.close();
          drawers.import.open$();
        }}
        onOpenEndpoint={(collection, endpoint) => {
          openTab(collection.id, collection.name, endpoint);
          drawers.importReport.close();
        }}
        onPreviewEndpointPrompt={(collection, endpoint, change) => {
          const tab: EndpointTab = {
            id: `import-report:${collection.id}:${endpoint.method}:${endpoint.path}`,
            collectionId: collection.id,
            collectionName: collection.name,
            method: endpoint.method,
            path: endpoint.path,
            inspector: "ai",
            example: "success",
            endpoint
          };
          void aiActions.previewPrompt(
            tab,
            "success",
            importChangeGenerationContext(change)
          );
          drawers.importReport.close();
        }}
        onRefreshEndpointMock={(collection, endpoint, change) => {
          const context = importChangeGenerationContext(change);
          if (!context) return;
          const tab: EndpointTab = {
            id: `import-report:${collection.id}:${endpoint.method}:${endpoint.path}`,
            collectionId: collection.id,
            collectionName: collection.name,
            method: endpoint.method,
            path: endpoint.path,
            inspector: "ai",
            example: "success",
            endpoint
          };
          void aiActions.generateWithContext(tab, "success", true, context);
          drawers.importReport.close();
        }}
        onRefreshChangedMocks={(collection, changes) => {
          void (async () => {
            for (const { endpoint, change } of changes) {
              const context = importChangeGenerationContext(change);
              if (!context) continue;
              const tab: EndpointTab = {
                id: `import-report:${collection.id}:${endpoint.method}:${endpoint.path}`,
                collectionId: collection.id,
                collectionName: collection.name,
                method: endpoint.method,
                path: endpoint.path,
                inspector: "ai",
                example: "success",
                endpoint
              };
              await aiActions.generateWithContext(tab, "success", true, context);
            }
          })();
          drawers.importReport.close();
        }}
      />

      <ImportDialog
        open={drawers.import.open}
        onClose={() => {
          if (importActions.importBusy) return;
          drawers.import.close();
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
        open={drawers.mockServer.open}
        onClose={drawers.mockServer.close}
        connected={isTauriRuntime}
        status={mockGateway.status}
        busy={mockGateway.busy}
        error={mockGateway.error}
        requests={mockGateway.requests}
        savedPreferences={mockGateway.savedPreferences}
        onStart={gatewayActions.start}
        onStop={mockGateway.stop}
        onApplyOverrides={gatewayActions.applyOverrides}
        onApplyConditionalRules={gatewayActions.applyConditionalRules}
        onApplyChaos={gatewayActions.applyChaos}
        onToggleCaptureBodies={gatewayActions.toggleCaptureBodies}
        onToggleEnforceRequestBodies={gatewayActions.toggleEnforceRequestBodies}
        onApplyRateLimits={gatewayActions.applyRateLimits}
        onApplyStatusOverrides={gatewayActions.applyStatusOverrides}
        onApplyResponseHeaders={gatewayActions.applyResponseHeaders}
        onSeedRequiredHeadersFromHints={gatewayActions.seedRequiredHeadersFromHints}
        onApplyProxyUpstream={gatewayActions.applyProxyUpstream}
        onToggleRequestCache={gatewayActions.toggleRequestCache}
        onReloadRequestCache={gatewayActions.reloadRequestCache}
        onClearLog={gatewayActions.clearLog}
        onExportBundle={gatewayActions.exportBundle}
        onImportBundle={gatewayActions.importBundle}
        onReplayRequest={gatewayActions.replayRequest}
        scenarios={{
          list: gatewayActions.listScenarios,
          save: gatewayActions.saveScenario,
          load: gatewayActions.loadScenario,
          del: gatewayActions.deleteScenario,
          rename: gatewayActions.renameScenario
        }}
      />

      <ProvidersPanel
        open={drawers.providers.open}
        onClose={drawers.providers.close}
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
        open={drawers.shortcuts.open}
        bindings={shortcutBindings}
        onClose={drawers.shortcuts.close}
      />

      <CommandPalette
        open={drawers.palette.open}
        items={paletteItems}
        onClose={drawers.palette.close}
        onRun={runPaletteItem}
      />

      <ToastHost toasts={toasts.toasts} onDismiss={toasts.dismiss} />
    </div>
  );
}

export default App;
