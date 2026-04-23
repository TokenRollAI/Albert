import { forwardRef, useImperativeHandle, useMemo, useRef, useState } from "react";
import { Icon } from "./Icon";
import type { CanonicalEndpoint, SidebarCollection } from "../types";

interface SidebarProps {
  collections: SidebarCollection[];
  activeTabId: string | null;
  onOpenEndpoint: (collection: SidebarCollection, endpoint: CanonicalEndpoint) => void;
  onImportClick: () => void;
  onRefresh: () => void;
  onExportCollection?: (collection: SidebarCollection) => void;
  onDeleteCollection?: (collection: SidebarCollection) => void;
  onRenameCollection?: (collection: SidebarCollection) => void;
  busy: boolean;
}

export interface SidebarHandle {
  focusSearch: () => void;
}

export const Sidebar = forwardRef<SidebarHandle, SidebarProps>(function Sidebar(
  {
    collections,
    activeTabId,
    onOpenEndpoint,
    onImportClick,
    onRefresh,
    onExportCollection,
    onDeleteCollection,
    onRenameCollection,
    busy
  }: SidebarProps,
  ref
) {
  const [query, setQuery] = useState("");
  const [expanded, setExpanded] = useState<Record<string, boolean>>(() =>
    collections.length === 1 ? { [collections[0].id]: true } : {}
  );
  const inputRef = useRef<HTMLInputElement | null>(null);

  useImperativeHandle(
    ref,
    () => ({
      focusSearch: () => {
        inputRef.current?.focus();
        inputRef.current?.select();
      }
    }),
    []
  );

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return collections;
    return collections
      .map((collection) => {
        const matchCollection = collection.name.toLowerCase().includes(q);
        const endpoints = collection.endpoints.filter(
          (endpoint) =>
            endpoint.path.toLowerCase().includes(q) ||
            endpoint.method.toLowerCase().includes(q) ||
            (endpoint.summary ?? "").toLowerCase().includes(q) ||
            (endpoint.operation_id ?? "").toLowerCase().includes(q)
        );
        if (matchCollection) {
          return collection;
        }
        if (endpoints.length > 0) {
          return { ...collection, endpoints };
        }
        return null;
      })
      .filter((value): value is SidebarCollection => value !== null);
  }, [collections, query]);

  // Flat list of all currently-visible endpoint rows, used for arrow-key
  // navigation. Only expanded collections contribute; collapsed ones are
  // skipped so the keyboard follows what the user can actually see.
  const visibleEndpoints = useMemo(() => {
    const out: { collection: SidebarCollection; endpoint: CanonicalEndpoint }[] = [];
    for (const collection of filtered) {
      if (!(expanded[collection.id] ?? false)) continue;
      for (const endpoint of collection.endpoints) {
        out.push({ collection, endpoint });
      }
    }
    return out;
  }, [filtered, expanded]);

  function toggle(id: string) {
    setExpanded((prev) => ({ ...prev, [id]: !prev[id] }));
  }

  function handleSearchKey(event: React.KeyboardEvent<HTMLInputElement>) {
    if (event.key !== "ArrowDown" && event.key !== "Enter") return;
    if (visibleEndpoints.length === 0) return;
    event.preventDefault();
    if (event.key === "Enter" && visibleEndpoints.length > 0) {
      const first = visibleEndpoints[0];
      onOpenEndpoint(first.collection, first.endpoint);
      return;
    }
    // ArrowDown → focus the first endpoint button
    const firstBtn = document.querySelector<HTMLButtonElement>(
      ".sidebar__list .endpoint"
    );
    firstBtn?.focus();
  }

  function handleEndpointKey(
    event: React.KeyboardEvent<HTMLButtonElement>,
    index: number
  ) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const next = event.key === "ArrowDown" ? index + 1 : index - 1;
      if (next < 0) {
        inputRef.current?.focus();
        return;
      }
      const buttons = document.querySelectorAll<HTMLButtonElement>(
        ".sidebar__list .endpoint"
      );
      const target = buttons.item(next);
      target?.focus();
    }
  }

  return (
    <aside className="sidebar">
      <div className="sidebar__head">
        <span className="sidebar__title">Collections</span>
        <div className="sidebar__head-actions">
          <button
            type="button"
            className="btn btn--icon btn--icon-sm"
            onClick={onRefresh}
            disabled={busy}
            aria-label="Refresh collections"
            title="Refresh"
          >
            <Icon name="refresh" size={14} />
          </button>
          <button
            type="button"
            className="btn btn--icon btn--icon-sm"
            onClick={onImportClick}
            aria-label="Import source"
            title="Import"
          >
            <Icon name="plus" size={14} />
          </button>
        </div>
      </div>

      <div className="sidebar__search">
        <Icon name="search" size={14} />
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={handleSearchKey}
          placeholder="Search endpoints  (⌘K)"
          spellCheck={false}
        />
      </div>

      <div className="sidebar__list">
        {filtered.length === 0 ? (
          <div className="sidebar__empty">
            <p>No collections yet.</p>
            <button
              type="button"
              className="btn btn--ghost btn--sm"
              onClick={onImportClick}
            >
              <Icon name="import" size={12} />
              <span>Import source</span>
            </button>
          </div>
        ) : (
          filtered.map((collection) => {
            const isOpen = expanded[collection.id] ?? false;
            return (
              <div key={collection.id} className="coll">
                <button
                  type="button"
                  className="coll__head"
                  onClick={() => toggle(collection.id)}
                >
                  <Icon
                    name={isOpen ? "chevron-down" : "chevron-right"}
                    size={12}
                  />
                  <Icon
                    name={isOpen ? "folder-open" : "folder"}
                    size={14}
                  />
                  <span className="coll__name" title={collection.name}>
                    {collection.name}
                  </span>
                  <span
                    className={
                      collection.origin === "imported"
                        ? "coll__badge coll__badge--imported"
                        : collection.origin === "preview"
                          ? "coll__badge coll__badge--preview"
                          : "coll__badge coll__badge--fallback"
                    }
                  >
                    {collection.origin === "imported"
                      ? "db"
                      : collection.origin === "preview"
                        ? "preview"
                        : "demo"}
                  </span>
                  {onRenameCollection && collection.origin === "imported" ? (
                    <span
                      role="button"
                      tabIndex={0}
                      className="btn btn--icon btn--icon-sm coll__action"
                      onClick={(event) => {
                        event.stopPropagation();
                        onRenameCollection(collection);
                      }}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.stopPropagation();
                          event.preventDefault();
                          onRenameCollection(collection);
                        }
                      }}
                      title="Rename collection"
                      aria-label={`Rename ${collection.name}`}
                    >
                      <Icon name="settings" size={12} />
                    </span>
                  ) : null}
                  {onExportCollection && collection.origin === "imported" ? (
                    <span
                      role="button"
                      tabIndex={0}
                      className="btn btn--icon btn--icon-sm coll__action"
                      onClick={(event) => {
                        event.stopPropagation();
                        onExportCollection(collection);
                      }}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.stopPropagation();
                          event.preventDefault();
                          onExportCollection(collection);
                        }
                      }}
                      title="Download snapshot as JSON"
                      aria-label={`Export ${collection.name} as JSON`}
                    >
                      <Icon name="save" size={12} />
                    </span>
                  ) : null}
                  {onDeleteCollection && collection.origin === "imported" ? (
                    <span
                      role="button"
                      tabIndex={0}
                      className="btn btn--icon btn--icon-sm coll__action coll__action--danger"
                      onClick={(event) => {
                        event.stopPropagation();
                        onDeleteCollection(collection);
                      }}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.stopPropagation();
                          event.preventDefault();
                          onDeleteCollection(collection);
                        }
                      }}
                      title="Delete collection"
                      aria-label={`Delete ${collection.name}`}
                    >
                      <Icon name="close" size={12} />
                    </span>
                  ) : null}
                </button>
                {isOpen ? (
                  <ul className="coll__items">
                    {collection.endpoints.map((endpoint) => {
                      const tabId = `${collection.id}::${endpoint.method.toUpperCase()}:${endpoint.path}`;
                      const active = activeTabId === tabId;
                      const flatIndex = visibleEndpoints.findIndex(
                        (entry) =>
                          entry.collection.id === collection.id &&
                          entry.endpoint.method === endpoint.method &&
                          entry.endpoint.path === endpoint.path
                      );
                      return (
                        <li key={tabId}>
                          <button
                            type="button"
                            className={
                              active
                                ? "endpoint endpoint--active"
                                : "endpoint"
                            }
                            onClick={() =>
                              onOpenEndpoint(collection, endpoint)
                            }
                            onKeyDown={(event) =>
                              handleEndpointKey(event, flatIndex)
                            }
                            title={endpoint.summary ?? endpoint.path}
                          >
                            <span
                              className={`method method--${endpoint.method.toLowerCase()}`}
                            >
                              {endpoint.method.toUpperCase()}
                            </span>
                            <span className="endpoint__path">
                              {endpoint.path}
                            </span>
                          </button>
                        </li>
                      );
                    })}
                    {collection.endpoints.length === 0 ? (
                      <li className="endpoint endpoint--empty">
                        No endpoints
                      </li>
                    ) : null}
                  </ul>
                ) : null}
              </div>
            );
          })
        )}
      </div>
    </aside>
  );
});
