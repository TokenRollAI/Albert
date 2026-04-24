import { useCallback, useEffect, useRef, useState } from "react";
import type {
  CanonicalApiCollection,
  CanonicalEndpoint,
  EndpointTab,
  ExampleKind,
  InspectorKey,
  MockExample
} from "../types";

export function makeTabId(
  collectionId: string,
  method: string,
  path: string
): string {
  return `${collectionId}::${method.toUpperCase()}:${path}`;
}

const TABS_STORAGE_KEY = "albert.tabs.v1";

interface PersistedTabRef {
  id: string;
  collectionId: string;
  collectionName: string;
  method: string;
  path: string;
  inspector: InspectorKey;
  example: ExampleKind;
}

interface PersistedTabState {
  activeId: string | null;
  tabs: PersistedTabRef[];
}

function persist(state: PersistedTabState): void {
  try {
    window.localStorage.setItem(TABS_STORAGE_KEY, JSON.stringify(state));
  } catch {
    /* ignore quota/storage errors */
  }
}

function loadPersisted(): PersistedTabState {
  try {
    const raw = window.localStorage.getItem(TABS_STORAGE_KEY);
    if (!raw) return { activeId: null, tabs: [] };
    const parsed = JSON.parse(raw) as Partial<PersistedTabState>;
    const tabs = Array.isArray(parsed.tabs) ? parsed.tabs : [];
    return {
      activeId: typeof parsed.activeId === "string" ? parsed.activeId : null,
      tabs: tabs.filter(
        (t): t is PersistedTabRef =>
          typeof t === "object" &&
          t !== null &&
          typeof (t as PersistedTabRef).id === "string" &&
          typeof (t as PersistedTabRef).collectionId === "string" &&
          typeof (t as PersistedTabRef).method === "string" &&
          typeof (t as PersistedTabRef).path === "string"
      )
    };
  } catch {
    return { activeId: null, tabs: [] };
  }
}

function defaultExample(endpoint: CanonicalEndpoint): ExampleKind {
  const kinds = endpoint.examples.map((example) => example.kind);
  if (kinds.includes("success")) return "success";
  if (kinds.includes("empty")) return "empty";
  if (kinds.includes("error")) return "error";
  return "success";
}

export function useEndpointTabs(): {
  tabs: EndpointTab[];
  activeId: string | null;
  activeTab: EndpointTab | null;
  openTab: (
    collectionId: string,
    collectionName: string,
    endpoint: CanonicalEndpoint
  ) => void;
  closeTab: (id: string) => void;
  activateTab: (id: string) => void;
  setInspector: (id: string, inspector: InspectorKey) => void;
  setExample: (id: string, example: ExampleKind) => void;
  updateEndpointExample: (id: string, example: MockExample) => void;
  resetTabs: () => void;
  restoreTabs: (collections: CanonicalApiCollection[]) => void;
  reorderTabs: (fromId: string, toId: string) => void;
} {
  const [tabs, setTabs] = useState<EndpointTab[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const mounted = useRef(false);

  // Persist the tab set whenever it changes. Stored as lightweight refs
  // (id + locator) rather than full CanonicalEndpoint blobs — the real
  // endpoint is re-resolved from storage on next boot. Skip the initial
  // render so mounting a fresh hook doesn't wipe the persisted state
  // before `restoreTabs` gets a chance to hydrate it.
  useEffect(() => {
    if (!mounted.current) {
      mounted.current = true;
      return;
    }
    persist({
      activeId,
      tabs: tabs.map((tab) => ({
        id: tab.id,
        collectionId: tab.collectionId,
        collectionName: tab.collectionName,
        method: tab.method,
        path: tab.path,
        inspector: tab.inspector,
        example: tab.example
      }))
    });
  }, [tabs, activeId]);

  const openTab = useCallback(
    (
      collectionId: string,
      collectionName: string,
      endpoint: CanonicalEndpoint
    ) => {
      const id = makeTabId(collectionId, endpoint.method, endpoint.path);
      setTabs((prev) => {
        if (prev.some((tab) => tab.id === id)) {
          return prev;
        }
        const next: EndpointTab = {
          id,
          collectionId,
          collectionName,
          method: endpoint.method.toUpperCase(),
          path: endpoint.path,
          endpoint,
          inspector: "params",
          example: defaultExample(endpoint)
        };
        return [...prev, next];
      });
      setActiveId(id);
    },
    []
  );

  const closeTab = useCallback(
    (id: string) => {
      setTabs((prev) => {
        const index = prev.findIndex((tab) => tab.id === id);
        if (index === -1) {
          return prev;
        }
        const next = prev.filter((tab) => tab.id !== id);
        setActiveId((current) => {
          if (current !== id) {
            return current;
          }
          if (next.length === 0) {
            return null;
          }
          const fallback = next[Math.min(index, next.length - 1)];
          return fallback.id;
        });
        return next;
      });
    },
    []
  );

  const activateTab = useCallback((id: string) => {
    setActiveId(id);
  }, []);

  const setInspector = useCallback((id: string, inspector: InspectorKey) => {
    setTabs((prev) =>
      prev.map((tab) => (tab.id === id ? { ...tab, inspector } : tab))
    );
  }, []);

  const setExample = useCallback((id: string, example: ExampleKind) => {
    setTabs((prev) =>
      prev.map((tab) => (tab.id === id ? { ...tab, example } : tab))
    );
  }, []);

  const updateEndpointExample = useCallback(
    (id: string, example: MockExample) => {
      setTabs((prev) =>
        prev.map((tab) => {
          if (tab.id !== id) return tab;
          const nextExamples = [...tab.endpoint.examples];
          const existing = nextExamples.findIndex((e) => e.kind === example.kind);
          const projected = {
            kind: example.kind,
            title: example.title,
            payload: example.payload,
            note: example.note
          };
          if (existing >= 0) {
            nextExamples[existing] = projected;
          } else {
            nextExamples.push(projected);
          }
          return {
            ...tab,
            example: example.kind,
            endpoint: { ...tab.endpoint, examples: nextExamples }
          };
        })
      );
    },
    []
  );

  const resetTabs = useCallback(() => {
    setTabs([]);
    setActiveId(null);
  }, []);

  /**
   * Move the tab identified by `fromId` to the slot currently held by
   * `toId`. No-op if either id is missing or if `fromId === toId`.
   * Called from the EndpointTabs drag handler — the tab bar is pinned
   * to native HTML5 drag events so no dependencies required.
   */
  const reorderTabs = useCallback((fromId: string, toId: string) => {
    if (fromId === toId) return;
    setTabs((prev) => {
      const fromIndex = prev.findIndex((tab) => tab.id === fromId);
      const toIndex = prev.findIndex((tab) => tab.id === toId);
      if (fromIndex === -1 || toIndex === -1) return prev;
      const next = prev.slice();
      const [moved] = next.splice(fromIndex, 1);
      next.splice(toIndex, 0, moved);
      return next;
    });
  }, []);

  /**
   * Walk the persisted tab set and reopen any tab whose collection +
   * endpoint still exist in the freshly-loaded `collections`. Safe to
   * call multiple times — it bails out when the live state already has
   * tabs (so user edits aren't clobbered).
   */
  const restoreTabs = useCallback(
    (collections: CanonicalApiCollection[]) => {
      if (tabs.length > 0) return;
      const persisted = loadPersisted();
      if (persisted.tabs.length === 0) return;
      const revived: EndpointTab[] = [];
      for (const ref of persisted.tabs) {
        const collection = collections.find((c) => c.id === ref.collectionId);
        if (!collection) continue;
        const endpoint = collection.endpoints.find(
          (e) =>
            e.method.toUpperCase() === ref.method.toUpperCase() &&
            e.path === ref.path
        );
        if (!endpoint) continue;
        revived.push({
          id: ref.id,
          collectionId: ref.collectionId,
          collectionName: collection.name,
          method: endpoint.method.toUpperCase(),
          path: endpoint.path,
          endpoint,
          inspector: ref.inspector ?? "params",
          example: ref.example ?? defaultExample(endpoint)
        });
      }
      if (revived.length === 0) return;
      setTabs(revived);
      const activeStillValid =
        persisted.activeId != null &&
        revived.some((t) => t.id === persisted.activeId);
      setActiveId(activeStillValid ? persisted.activeId : revived[0].id);
    },
    [tabs.length]
  );

  const activeTab = tabs.find((tab) => tab.id === activeId) ?? null;

  return {
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
  };
}
