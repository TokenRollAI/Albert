import { useCallback, useState } from "react";
import type {
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
} {
  const [tabs, setTabs] = useState<EndpointTab[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);

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
    resetTabs
  };
}
