import { useMemo, useState } from "react";
import { Icon } from "./Icon";
import { useDirtyRoutes } from "../hooks/useTryItDraft";
import type { EndpointTab } from "../types";

interface EndpointTabsProps {
  tabs: EndpointTab[];
  activeId: string | null;
  onActivate: (id: string) => void;
  onClose: (id: string) => void;
  onNew: () => void;
  onReorder?: (fromId: string, toId: string) => void;
}

function routeKeyOf(tab: EndpointTab): string {
  return `${tab.method.toUpperCase()} ${tab.endpoint.path}`;
}

export function EndpointTabs({
  tabs,
  activeId,
  onActivate,
  onClose,
  onNew,
  onReorder
}: EndpointTabsProps) {
  const routeKeys = useMemo(() => tabs.map(routeKeyOf), [tabs]);
  const dirtyRoutes = useDirtyRoutes(routeKeys);
  const [draggingId, setDraggingId] = useState<string | null>(null);
  const [dropTargetId, setDropTargetId] = useState<string | null>(null);
  if (tabs.length === 0) {
    return null;
  }
  return (
    <div className="tabs" role="tablist">
      <div className="tabs__scroll">
        {tabs.map((tab) => {
          const active = tab.id === activeId;
          const dirty = dirtyRoutes.has(routeKeyOf(tab));
          const isDragging = draggingId === tab.id;
          const isDropTarget =
            onReorder != null && dropTargetId === tab.id && draggingId !== null && draggingId !== tab.id;
          const classes = ["tab"];
          if (active) classes.push("tab--active");
          if (isDragging) classes.push("tab--dragging");
          if (isDropTarget) classes.push("tab--drop-target");
          return (
            <div
              key={tab.id}
              className={classes.join(" ")}
              role="tab"
              aria-selected={active}
              draggable={onReorder != null}
              onDragStart={(event) => {
                if (!onReorder) return;
                setDraggingId(tab.id);
                event.dataTransfer.effectAllowed = "move";
                event.dataTransfer.setData("text/plain", tab.id);
              }}
              onDragOver={(event) => {
                if (!onReorder || draggingId == null) return;
                event.preventDefault();
                event.dataTransfer.dropEffect = "move";
                if (dropTargetId !== tab.id) {
                  setDropTargetId(tab.id);
                }
              }}
              onDragLeave={(event) => {
                // Only clear when leaving the element to outside, not to a child.
                if (
                  event.currentTarget.contains(
                    event.relatedTarget as Node | null
                  )
                ) {
                  return;
                }
                setDropTargetId((prev) => (prev === tab.id ? null : prev));
              }}
              onDrop={(event) => {
                if (!onReorder || !draggingId) return;
                event.preventDefault();
                onReorder(draggingId, tab.id);
                setDraggingId(null);
                setDropTargetId(null);
              }}
              onDragEnd={() => {
                setDraggingId(null);
                setDropTargetId(null);
              }}
            >
              <button
                type="button"
                className="tab__body"
                onClick={() => onActivate(tab.id)}
                title={
                  dirty
                    ? `${tab.method} ${tab.path} — unsaved Try-it draft`
                    : `${tab.method} ${tab.path}`
                }
              >
                <span
                  className={`method method--${tab.method.toLowerCase()}`}
                >
                  {tab.method}
                </span>
                <span className="tab__path">{tab.path}</span>
                {dirty ? (
                  <span className="tab__dirty" aria-label="Unsaved draft">
                    •
                  </span>
                ) : null}
              </button>
              <button
                type="button"
                className="tab__close"
                onClick={(event) => {
                  event.stopPropagation();
                  onClose(tab.id);
                }}
                aria-label={`Close ${tab.method} ${tab.path}`}
              >
                <Icon name="close" size={12} />
              </button>
            </div>
          );
        })}
      </div>
      <button
        type="button"
        className="tabs__add"
        onClick={onNew}
        aria-label="Import new source"
        title="Import"
      >
        <Icon name="plus" size={14} />
      </button>
    </div>
  );
}
