import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState
} from "react";
import { Icon } from "./Icon";
import type {
  AuthRequirementHint,
  CanonicalEndpoint,
  SidebarCollection
} from "../types";

const EXPANDED_STORAGE_KEY = "albert.sidebar.expanded.v1";

function loadExpandedState(): Record<string, boolean> {
  try {
    const raw = window.localStorage.getItem(EXPANDED_STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      const out: Record<string, boolean> = {};
      for (const [key, value] of Object.entries(parsed)) {
        if (typeof value === "boolean") out[key] = value;
      }
      return out;
    }
    return {};
  } catch {
    return {};
  }
}

/// Bucketize endpoints by HTTP method for the collapsed-collection
/// chip row. Returns them in a stable order (GET/POST/PUT/PATCH/DELETE
/// first, then anything else alphabetically) so a collection's chips
/// look the same every render regardless of endpoint insertion order.
export function countMethods(
  endpoints: CanonicalEndpoint[]
): Array<{ method: string; count: number }> {
  const counts = new Map<string, number>();
  for (const endpoint of endpoints) {
    const key = endpoint.method.toUpperCase();
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  const order = ["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD"];
  const primary = order
    .map((m) => [m, counts.get(m)] as const)
    .filter((entry): entry is readonly [string, number] => entry[1] !== undefined)
    .map(([method, count]) => ({ method, count }));
  const extras = [...counts.entries()]
    .filter(([m]) => !order.includes(m))
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([method, count]) => ({ method, count }));
  return [...primary, ...extras];
}

const HTTP_METHODS = new Set([
  "get",
  "post",
  "put",
  "patch",
  "delete",
  "options",
  "head",
  "trace"
]);

/**
 * Decide whether an endpoint should survive the sidebar's text filter.
 *
 * Single-token query (default): case-insensitive substring match
 * against method, path, summary, or operation_id. Power-user path:
 * when the first token is an HTTP method (`get`, `post`, etc.) AND a
 * second token is present, the method is matched exactly and the
 * second token must match the path / summary / operation id. So
 * `get users` narrows to GET endpoints whose path/summary mentions
 * "users" (instead of matching any endpoint containing either word).
 * `get` alone still works as a single-token method filter.
 */
export function matchesEndpointQuery(
  endpoint: Pick<
    CanonicalEndpoint,
    "method" | "path" | "summary" | "operation_id"
  >,
  rawQuery: string
): boolean {
  const query = rawQuery.trim().toLowerCase();
  if (!query) return true;
  const tokens = query.split(/\s+/).filter(Boolean);
  const method = endpoint.method.toLowerCase();
  const path = endpoint.path.toLowerCase();
  const summary = (endpoint.summary ?? "").toLowerCase();
  const opId = (endpoint.operation_id ?? "").toLowerCase();

  if (tokens.length >= 2 && HTTP_METHODS.has(tokens[0])) {
    if (method !== tokens[0]) return false;
    const rest = tokens.slice(1).join(" ");
    return (
      path.includes(rest) || summary.includes(rest) || opId.includes(rest)
    );
  }

  return (
    path.includes(query) ||
    method.includes(query) ||
    summary.includes(query) ||
    opId.includes(query)
  );
}

function authTitle(hint: AuthRequirementHint): string {
  const base = (() => {
    switch (hint.scheme) {
      case "http_bearer":
        return `Requires ${hint.header_name}: Bearer …`;
      case "http_basic":
        return `Requires ${hint.header_name}: Basic …`;
      case "oauth2":
        return `Requires ${hint.header_name}: Bearer … (OAuth2)`;
      case "api_key_header":
        return `Requires ${hint.header_name} header`;
      default:
        return `Requires ${hint.header_name}`;
    }
  })();
  return hint.description ? `${base} — ${hint.description}` : base;
}

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
  const [activeTag, setActiveTag] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<Record<string, boolean>>(() => {
    const saved = loadExpandedState();
    if (Object.keys(saved).length > 0) return saved;
    // First-ever mount: expand the single collection so the user sees
    // endpoints without having to click. Multi-collection imports stay
    // collapsed to keep the initial view scannable.
    return collections.length === 1 ? { [collections[0].id]: true } : {};
  });
  const inputRef = useRef<HTMLInputElement | null>(null);

  // Mirror the expanded map into localStorage on every change. We guard
  // with a ref so the initial state doesn't overwrite a just-loaded map
  // before the user has done anything.
  const hasMountedRef = useRef(false);
  useEffect(() => {
    if (!hasMountedRef.current) {
      hasMountedRef.current = true;
      return;
    }
    try {
      window.localStorage.setItem(
        EXPANDED_STORAGE_KEY,
        JSON.stringify(expanded)
      );
    } catch {
      /* quota / serialization — ignore */
    }
  }, [expanded]);

  const expandAll = useCallback(() => {
    setExpanded(() => {
      const next: Record<string, boolean> = {};
      for (const collection of collections) {
        next[collection.id] = true;
      }
      return next;
    });
  }, [collections]);

  const collapseAll = useCallback(() => {
    setExpanded(() => {
      const next: Record<string, boolean> = {};
      for (const collection of collections) {
        next[collection.id] = false;
      }
      return next;
    });
  }, [collections]);

  const anyExpanded = useMemo(
    () => collections.some((c) => expanded[c.id]),
    [collections, expanded]
  );

  // Union of all endpoint tags across currently-visible collections.
  // Sorted alphabetically so the chip order is stable across renders.
  const availableTags = useMemo(() => {
    const set = new Set<string>();
    for (const collection of collections) {
      for (const endpoint of collection.endpoints) {
        for (const tag of endpoint.tags ?? []) {
          if (tag) set.add(tag);
        }
      }
    }
    return [...set].sort((a, b) => a.localeCompare(b));
  }, [collections]);

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
    const tagActive = activeTag !== null;
    if (!q && !tagActive) return collections;
    return collections
      .map((collection) => {
        const matchCollection = !tagActive && collection.name.toLowerCase().includes(q);
        const endpoints = collection.endpoints.filter((endpoint) => {
          if (tagActive && !(endpoint.tags ?? []).includes(activeTag)) {
            return false;
          }
          return matchesEndpointQuery(endpoint, q);
        });
        if (matchCollection) {
          // Tag filter inactive and collection name matches — keep everything
          // that passes the endpoint filter (which in this branch is just the
          // text filter).
          return { ...collection, endpoints };
        }
        if (endpoints.length > 0) {
          return { ...collection, endpoints };
        }
        return null;
      })
      .filter((value): value is SidebarCollection => value !== null);
  }, [activeTag, collections, query]);

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
            onClick={anyExpanded ? collapseAll : expandAll}
            disabled={collections.length === 0}
            aria-label={anyExpanded ? "Collapse all" : "Expand all"}
            title={anyExpanded ? "Collapse all" : "Expand all"}
          >
            <Icon
              name={anyExpanded ? "chevron-down" : "chevron-right"}
              size={14}
            />
          </button>
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

      {availableTags.length > 0 ? (
        <div className="sidebar__tags" role="group" aria-label="Filter by tag">
          {availableTags.map((tag) => {
            const active = tag === activeTag;
            return (
              <button
                key={tag}
                type="button"
                className={active ? "tag-chip tag-chip--active" : "tag-chip"}
                onClick={() => setActiveTag(active ? null : tag)}
                title={active ? `Clear ${tag} filter` : `Show only ${tag}`}
              >
                {tag}
              </button>
            );
          })}
          {activeTag ? (
            <button
              type="button"
              className="tag-chip tag-chip--clear"
              onClick={() => setActiveTag(null)}
              title="Clear tag filter"
            >
              ✕
            </button>
          ) : null}
        </div>
      ) : null}

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
            const methodCounts = countMethods(collection.endpoints);
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
                  {!isOpen && methodCounts.length > 0 ? (
                    <span className="coll__method-chips" aria-hidden="true">
                      {methodCounts.map(({ method, count }) => (
                        <span
                          key={method}
                          className={`coll__method-chip method--${method.toLowerCase()}`}
                          title={`${count} ${method} endpoint${count === 1 ? "" : "s"}`}
                        >
                          {method} {count}
                        </span>
                      ))}
                    </span>
                  ) : null}
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
                            {endpoint.auth ? (
                              <span
                                className="endpoint__auth"
                                title={authTitle(endpoint.auth)}
                                aria-label="Requires authentication"
                              >
                                <Icon name="shield" size={11} />
                              </span>
                            ) : null}
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
