import { Icon } from "./Icon";
import type { EndpointTab } from "../types";

interface UrlBarProps {
  tab: EndpointTab;
  disabled: boolean;
  onGenerateMock?: () => void;
}

export function UrlBar({ tab, disabled, onGenerateMock }: UrlBarProps) {
  return (
    <div className="urlbar">
      <div className="urlbar__input">
        <span
          className={`method method--${tab.method.toLowerCase()} method--chip`}
        >
          {tab.method}
        </span>
        <span className="urlbar__path" title={tab.path}>
          {tab.path}
        </span>
        <span className="urlbar__summary">
          {tab.endpoint.summary ?? tab.endpoint.operation_id ?? "endpoint"}
        </span>
      </div>

      <button
        type="button"
        className="btn btn--primary"
        onClick={onGenerateMock}
        disabled={disabled}
        title="Generate Mock (coming soon)"
      >
        <Icon name="sparkles" size={14} />
        <span>Generate Mock</span>
      </button>
    </div>
  );
}
