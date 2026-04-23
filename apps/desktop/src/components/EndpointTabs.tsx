import { Icon } from "./Icon";
import type { EndpointTab } from "../types";

interface EndpointTabsProps {
  tabs: EndpointTab[];
  activeId: string | null;
  onActivate: (id: string) => void;
  onClose: (id: string) => void;
  onNew: () => void;
}

export function EndpointTabs({
  tabs,
  activeId,
  onActivate,
  onClose,
  onNew
}: EndpointTabsProps) {
  if (tabs.length === 0) {
    return null;
  }
  return (
    <div className="tabs" role="tablist">
      <div className="tabs__scroll">
        {tabs.map((tab) => {
          const active = tab.id === activeId;
          return (
            <div
              key={tab.id}
              className={active ? "tab tab--active" : "tab"}
              role="tab"
              aria-selected={active}
            >
              <button
                type="button"
                className="tab__body"
                onClick={() => onActivate(tab.id)}
                title={`${tab.method} ${tab.path}`}
              >
                <span
                  className={`method method--${tab.method.toLowerCase()}`}
                >
                  {tab.method}
                </span>
                <span className="tab__path">{tab.path}</span>
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
