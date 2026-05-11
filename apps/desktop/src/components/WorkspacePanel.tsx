import { Icon } from "./Icon";
import {
  countMethods,
  formatCollectionTimestamp
} from "./Sidebar";
import type { CanonicalEndpoint, SidebarCollection } from "../types";

interface WorkspacePanelProps {
  open: boolean;
  collections: SidebarCollection[];
  connected: boolean;
  onClose: () => void;
  onImportClick: () => void;
  onOpenEndpoint: (collection: SidebarCollection, endpoint: CanonicalEndpoint) => void;
  onRefresh: () => void;
  onRenameCollection?: (collection: SidebarCollection) => void;
  onExportCollection?: (collection: SidebarCollection) => void;
  onDeleteCollection?: (collection: SidebarCollection) => void;
  busy?: boolean;
}

function originLabel(collection: SidebarCollection): string {
  if (collection.origin === "imported") return "imported";
  if (collection.origin === "preview") return "preview";
  return "demo";
}

export function workspaceCollectionMeta(collection: SidebarCollection): string {
  const updated = formatCollectionTimestamp(collection.updatedAt);
  const imported = formatCollectionTimestamp(collection.createdAt);
  const count = collection.endpointCount ?? collection.endpoints.length;
  const endpointLabel = `${count} endpoint${count === 1 ? "" : "s"}`;
  const dateLabel = updated
    ? `Updated ${updated}`
    : imported
      ? `Imported ${imported}`
      : "No timestamp";
  return `${dateLabel} · ${endpointLabel}`;
}

export function WorkspacePanel({
  open,
  collections,
  connected,
  onClose,
  onImportClick,
  onOpenEndpoint,
  onRefresh,
  onRenameCollection,
  onExportCollection,
  onDeleteCollection,
  busy = false
}: WorkspacePanelProps) {
  if (!open) return null;

  const importedCollections = collections.filter(
    (collection) => collection.origin === "imported"
  );
  const visibleCollections =
    importedCollections.length > 0 ? importedCollections : collections;
  const endpointCount = importedCollections.reduce(
    (sum, collection) => sum + (collection.endpointCount ?? collection.endpoints.length),
    0
  );

  return (
    <div className="drawer" role="dialog" aria-label="Workspace collections">
      <div className="drawer__backdrop" onClick={onClose} />
      <div className="drawer__panel drawer__panel--lg">
        <header className="drawer__head">
          <div className="drawer__title">
            <Icon name="database" size={16} />
            <h2>Workspace</h2>
            <span className={connected ? "pill pill--ok" : "pill pill--warn"}>
              {connected ? "connected" : "fallback"}
            </span>
          </div>
          <div className="drawer__head-actions">
            <button
              type="button"
              className="btn btn--ghost btn--sm"
              onClick={onRefresh}
              disabled={busy || !connected}
            >
              <Icon name="refresh" size={12} />
              <span>{busy ? "Refreshing" : "Refresh"}</span>
            </button>
            <button
              type="button"
              className="btn btn--primary btn--sm"
              onClick={onImportClick}
            >
              <Icon name="import" size={12} />
              <span>Import</span>
            </button>
            <button
              type="button"
              className="btn btn--icon"
              onClick={onClose}
              aria-label="Close workspace panel"
            >
              <Icon name="close" size={16} />
            </button>
          </div>
        </header>

        <div className="drawer__body">
          <section className="workspace-summary" aria-label="Workspace summary">
            <div className="workspace-summary__item">
              <span className="workspace-summary__value">
                {importedCollections.length}
              </span>
              <span className="workspace-summary__label">collections</span>
            </div>
            <div className="workspace-summary__item">
              <span className="workspace-summary__value">{endpointCount}</span>
              <span className="workspace-summary__label">endpoints</span>
            </div>
            <div className="workspace-summary__item">
              <span className="workspace-summary__value">
                {connected ? "SQLite" : "Preview"}
              </span>
              <span className="workspace-summary__label">source</span>
            </div>
          </section>

          {visibleCollections.length === 0 ? (
            <section className="panel workspace-empty">
              <Icon name="database" size={20} />
              <h3>No imported collections</h3>
              <p>Import an OpenAPI spec or cURL request to populate this workspace.</p>
              <button
                type="button"
                className="btn btn--primary btn--sm"
                onClick={onImportClick}
              >
                <Icon name="import" size={12} />
                <span>Import source</span>
              </button>
            </section>
          ) : (
            <section className="workspace-list" aria-label="Collection history">
              {visibleCollections.map((collection) => {
                const methodCounts = countMethods(collection.endpoints);
                const firstEndpoint = collection.endpoints[0] ?? null;
                const isImported = collection.origin === "imported";
                return (
                  <article key={collection.id} className="workspace-card">
                    <div className="workspace-card__main">
                      <div className="workspace-card__title-row">
                        <h3 title={collection.name}>{collection.name}</h3>
                        <span
                          className={`coll__badge coll__badge--${collection.origin}`}
                        >
                          {originLabel(collection)}
                        </span>
                      </div>
                      <p className="workspace-card__meta">
                        {workspaceCollectionMeta(collection)}
                      </p>
                      {methodCounts.length > 0 ? (
                        <div
                          className="workspace-card__methods"
                          aria-label={`${collection.name} methods`}
                        >
                          {methodCounts.map(({ method, count }) => (
                            <span
                              key={method}
                              className={`coll__method-chip method--${method.toLowerCase()}`}
                            >
                              {method} {count}
                            </span>
                          ))}
                        </div>
                      ) : (
                        <p className="workspace-card__empty">No endpoints</p>
                      )}
                    </div>
                    <div className="workspace-card__actions">
                      <button
                        type="button"
                        className="btn btn--ghost btn--sm"
                        onClick={() => firstEndpoint && onOpenEndpoint(collection, firstEndpoint)}
                        disabled={!firstEndpoint}
                      >
                        <Icon name="play" size={12} />
                        <span>Open</span>
                      </button>
                      {isImported && onRenameCollection ? (
                        <button
                          type="button"
                          className="btn btn--icon btn--icon-sm"
                          onClick={() => onRenameCollection(collection)}
                          title="Rename collection"
                          aria-label={`Rename ${collection.name}`}
                        >
                          <Icon name="settings" size={12} />
                        </button>
                      ) : null}
                      {isImported && onExportCollection ? (
                        <button
                          type="button"
                          className="btn btn--icon btn--icon-sm"
                          onClick={() => onExportCollection(collection)}
                          title="Export collection"
                          aria-label={`Export ${collection.name}`}
                        >
                          <Icon name="save" size={12} />
                        </button>
                      ) : null}
                      {isImported && onDeleteCollection ? (
                        <button
                          type="button"
                          className="btn btn--icon btn--icon-sm coll__action--danger"
                          onClick={() => onDeleteCollection(collection)}
                          title="Delete collection"
                          aria-label={`Delete ${collection.name}`}
                        >
                          <Icon name="close" size={12} />
                        </button>
                      ) : null}
                    </div>
                  </article>
                );
              })}
            </section>
          )}
        </div>
      </div>
    </div>
  );
}
