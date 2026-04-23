import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { PanelCard } from "./components/PanelCard";
import { StatusPill } from "./components/StatusPill";
import {
  fallbackParsedCollection,
  fallbackSummary,
  sampleImportText
} from "./data/fallback";
import type {
  AppBootstrapSummary,
  CanonicalApiCollection,
  CanonicalEndpoint,
  ImportResult,
  StoredCollectionSummary,
  StoredEndpointSummary
} from "./types";

function App() {
  const [summary, setSummary] = useState<AppBootstrapSummary>(fallbackSummary);
  const [runtimeSource, setRuntimeSource] = useState("Scaffold");
  const [importName, setImportName] = useState("Albert Example API");
  const [importText, setImportText] = useState(sampleImportText);
  const [parsedCollection, setParsedCollection] =
    useState<CanonicalApiCollection>(fallbackParsedCollection);
  const [storedCollections, setStoredCollections] = useState<StoredCollectionSummary[]>([]);
  const [storedEndpoints, setStoredEndpoints] = useState<StoredEndpointSummary[]>([]);
  const [selectedStoredCollectionId, setSelectedStoredCollectionId] = useState<string | null>(
    null
  );
  const [selectedEndpointKey, setSelectedEndpointKey] = useState(
    endpointKey(
      fallbackParsedCollection.endpoints[0]?.method ?? "GET",
      fallbackParsedCollection.endpoints[0]?.path ?? "/"
    )
  );
  const [importMessage, setImportMessage] = useState(
    "Ready. Paste OpenAPI JSON/YAML or cURL, then parse or import."
  );
  const [busyAction, setBusyAction] = useState<"parse" | "import" | null>(null);

  const isTauriRuntime = runtimeSource === "Tauri Runtime";

  useEffect(() => {
    let active = true;

    async function loadBootstrap() {
      try {
        const data = await invoke<AppBootstrapSummary>("bootstrap_summary");

        if (!active) {
          return;
        }

        setSummary(data);
        setRuntimeSource("Tauri Runtime");
        await refreshImportedCollections();
      } catch {
        if (!active) {
          return;
        }

        setRuntimeSource("Local Fallback");
      }
    }

    loadBootstrap();

    return () => {
      active = false;
    };
  }, []);

  async function refreshImportedCollections(targetCollectionId?: string) {
    try {
      const collections = await invoke<StoredCollectionSummary[]>("list_imported_collections");
      setStoredCollections(collections);

      const nextCollectionId =
        targetCollectionId ?? selectedStoredCollectionId ?? collections[0]?.id ?? null;
      setSelectedStoredCollectionId(nextCollectionId);

      if (!nextCollectionId) {
        setStoredEndpoints([]);
        return;
      }

      const endpoints = await invoke<StoredEndpointSummary[]>("list_imported_endpoints", {
        collectionId: nextCollectionId
      });
      setStoredEndpoints(endpoints);
    } catch {
      setStoredCollections([]);
      setStoredEndpoints([]);
    }
  }

  async function handleSelectStoredCollection(collectionId: string) {
    setSelectedStoredCollectionId(collectionId);

    if (!isTauriRuntime) {
      return;
    }

    try {
      const endpoints = await invoke<StoredEndpointSummary[]>("list_imported_endpoints", {
        collectionId
      });
      setStoredEndpoints(endpoints);
    } catch {
      setStoredEndpoints([]);
    }
  }

  async function handleParse() {
    if (!isTauriRuntime) {
      setParsedCollection(fallbackParsedCollection);
      setImportMessage("Tauri runtime unavailable. Showing local fallback preview.");
      return;
    }

    try {
      setBusyAction("parse");
      const collection = await invoke<CanonicalApiCollection>("parse_api_description", {
        body: importText,
        name: importName || null
      });
      setParsedCollection(collection);
      setSelectedEndpointKey(
        endpointKey(collection.endpoints[0]?.method ?? "GET", collection.endpoints[0]?.path ?? "/")
      );
      setImportMessage(
        `Parsed ${collection.endpoints.length} endpoint(s) from ${collection.source}.`
      );
    } catch (error) {
      setImportMessage(`Parse failed: ${String(error)}`);
    } finally {
      setBusyAction(null);
    }
  }

  async function handleImport() {
    if (!isTauriRuntime) {
      setImportMessage("SQLite import requires the Tauri runtime.");
      return;
    }

    try {
      setBusyAction("import");
      const result = await invoke<ImportResult>("import_api_description", {
        body: importText,
        name: importName || null
      });
      const collection = await invoke<CanonicalApiCollection>("parse_api_description", {
        body: importText,
        name: importName || null
      });
      setParsedCollection(collection);
      setSelectedEndpointKey(
        endpointKey(collection.endpoints[0]?.method ?? "GET", collection.endpoints[0]?.path ?? "/")
      );
      await refreshImportedCollections(result.collection_id);
      setImportMessage(
        `Imported ${result.endpoint_count} endpoint(s) into ${result.database_url}.`
      );
    } catch (error) {
      setImportMessage(`Import failed: ${String(error)}`);
    } finally {
      setBusyAction(null);
    }
  }

  const selectedEndpoint =
    parsedCollection.endpoints.find(
      (endpoint) => endpointKey(endpoint.method, endpoint.path) === selectedEndpointKey
    ) ?? parsedCollection.endpoints[0];

  return (
    <div className="tool-shell">
      <header className="titlebar">
        <div className="titlebar__left">
          <img
            src="/favicon-32x32.png"
            alt="Albert"
            className="titlebar__logo"
          />
          <span className="titlebar__product">Albert</span>
          <span className="titlebar__separator">/</span>
          <span className="titlebar__workspace">{parsedCollection.name}</span>
        </div>

        <div className="titlebar__right">
          <ToolStat label="runtime" value={runtimeSource} />
          <ToolStat label="phase" value={summary.current_phase} />
          <ToolStat label="parsed" value={`${parsedCollection.endpoints.length} endpoints`} />
        </div>
      </header>

      <div className="toolbar">
        <div className="toolbar__group">
          <button
            type="button"
            className="tool-button tool-button--primary"
            onClick={handleParse}
            disabled={busyAction !== null}
          >
            {busyAction === "parse" ? "Parsing..." : "Parse"}
          </button>
          <button
            type="button"
            className="tool-button"
            onClick={handleImport}
            disabled={busyAction !== null}
          >
            {busyAction === "import" ? "Importing..." : "Import To SQLite"}
          </button>
        </div>

        <div className="toolbar__group toolbar__group--meta">
          <span className="toolbar__meta">source {parsedCollection.source}</span>
          <span className="toolbar__meta">stored {storedCollections.length}</span>
          <span className="toolbar__meta">status {isTauriRuntime ? "connected" : "fallback"}</span>
        </div>
      </div>

      <main className="tool-workbench">
        <section className="tool-column tool-column--left">
          <PanelCard
            eyebrow="Input"
            title="Source"
            className="panel-card panel-card--tool panel-card--grow"
          >
            <div className="tool-form">
              <label className="field-block">
                <span>Collection</span>
                <input
                  type="text"
                  value={importName}
                  onChange={(event) => setImportName(event.target.value)}
                  placeholder="Orders API"
                />
              </label>

              <label className="field-block field-block--grow">
                <span>OpenAPI JSON/YAML or cURL</span>
                <textarea
                  value={importText}
                  onChange={(event) => setImportText(event.target.value)}
                  spellCheck={false}
                />
              </label>
            </div>
          </PanelCard>

          <PanelCard
            eyebrow="Storage"
            title="Collections"
            className="panel-card panel-card--tool"
          >
            {storedCollections.length === 0 ? (
              <p className="tool-muted">
                {isTauriRuntime ? "No imported collections." : "Tauri runtime required for SQLite."}
              </p>
            ) : (
              <div className="tool-list">
                {storedCollections.map((collection) => {
                  const active = selectedStoredCollectionId === collection.id;
                  return (
                    <button
                      key={collection.id}
                      type="button"
                      className={active ? "tool-list__item tool-list__item--active" : "tool-list__item"}
                      onClick={() => handleSelectStoredCollection(collection.id)}
                    >
                      <strong>{collection.name}</strong>
                      <span>
                        {collection.source_kind} · {collection.endpoint_count}
                      </span>
                    </button>
                  );
                })}
              </div>
            )}
          </PanelCard>
        </section>

        <section className="tool-column tool-column--center">
          <PanelCard
            eyebrow="Endpoints"
            title="Collection View"
            aside={<span className="tool-meta-chip">{parsedCollection.id}</span>}
            className="panel-card panel-card--tool panel-card--grow"
          >
            <div className="endpoint-workbench">
              <div className="endpoint-browser">
                {parsedCollection.endpoints.map((endpoint) => {
                  const key = endpointKey(endpoint.method, endpoint.path);
                  const active = selectedEndpointKey === key;
                  return (
                    <button
                      key={key}
                      type="button"
                      className={
                        active
                          ? "endpoint-browser__item endpoint-browser__item--active"
                          : "endpoint-browser__item"
                      }
                      onClick={() => setSelectedEndpointKey(key)}
                    >
                      <strong>
                        {endpoint.method.toUpperCase()} {endpoint.path}
                      </strong>
                      <span>{endpoint.summary ?? endpoint.operation_id ?? "Endpoint"}</span>
                    </button>
                  );
                })}
              </div>

              <div className="endpoint-inspector">
                <header className="endpoint-inspector__header">
                  <div>
                    <p className="endpoint-inspector__eyebrow">Selected</p>
                    <h2>
                      {selectedEndpoint.method.toUpperCase()} {selectedEndpoint.path}
                    </h2>
                  </div>
                  <div className="endpoint-inspector__summary">
                    {selectedEndpoint.summary ?? selectedEndpoint.operation_id ?? "Canonical endpoint"}
                  </div>
                </header>

                <div className="inspector-grid">
                  <InspectorBlock
                    title="Parameters"
                    emptyText="No parameters."
                    items={selectedEndpoint.parameters.map(
                      (parameter) =>
                        `${parameter.location}.${parameter.name}${
                          parameter.required ? " [required]" : ""
                        }`
                    )}
                  />
                  <InspectorBlock
                    title="Request"
                    emptyText="No request body."
                    items={
                      selectedEndpoint.request_body
                        ? [
                            selectedEndpoint.request_body.content_type,
                            selectedEndpoint.request_body.schema.node_type
                          ]
                        : []
                    }
                  />
                  <InspectorBlock
                    title="Responses"
                    emptyText="No responses."
                    items={selectedEndpoint.responses.map(
                      (response) =>
                        `${response.status_code} · ${response.content_type}${
                          response.schema ? ` · ${response.schema.node_type}` : ""
                        }`
                    )}
                  />
                  <InspectorBlock
                    title="Tags"
                    emptyText="No tags."
                    items={selectedEndpoint.tags}
                  />
                </div>
              </div>
            </div>
          </PanelCard>
        </section>

        <section className="tool-column tool-column--right">
          <PanelCard
            eyebrow="SQLite"
            title="Stored Endpoints"
            className="panel-card panel-card--tool panel-card--grow"
          >
            {storedEndpoints.length === 0 ? (
              <p className="tool-muted">Import a collection to populate SQLite endpoint records.</p>
            ) : (
              <div className="tool-list">
                {storedEndpoints.map((endpoint) => (
                  <div
                    key={endpoint.id}
                    className="tool-list__item tool-list__item--read-only"
                  >
                    <strong>
                      {endpoint.method} {endpoint.path}
                    </strong>
                    <span>{endpoint.summary ?? "Imported endpoint"}</span>
                  </div>
                ))}
              </div>
            )}
          </PanelCard>

          <PanelCard
            eyebrow="Mocks"
            title="Example States"
            className="panel-card panel-card--tool"
          >
            <div className="tool-pill-row">
              {selectedEndpoint.examples.map((example) => (
                <StatusPill key={example.kind} stage={example.kind} />
              ))}
            </div>
            <p className="tool-muted">
              Static mock states are already persisted, so the runtime layer can attach routing later.
            </p>
          </PanelCard>
        </section>
      </main>

      <footer className="statusbar">
        <span className="statusbar__message">{importMessage}</span>
        <span className="statusbar__detail">
          {selectedEndpoint.method.toUpperCase()} {selectedEndpoint.path}
        </span>
      </footer>
    </div>
  );
}

function ToolStat({ label, value }: { label: string; value: string }) {
  return (
    <div className="tool-stat">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function InspectorBlock({
  title,
  items,
  emptyText
}: {
  title: string;
  items: string[];
  emptyText: string;
}) {
  return (
    <section className="inspector-block">
      <h3>{title}</h3>
      {items.length === 0 ? (
        <p className="tool-muted">{emptyText}</p>
      ) : (
        <ul className="inspector-tags">
          {items.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      )}
    </section>
  );
}

function endpointKey(method: string, path: string) {
  return `${method.toUpperCase()}:${path}`;
}

export default App;
