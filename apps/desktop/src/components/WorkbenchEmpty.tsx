import { Icon } from "./Icon";

interface WorkbenchEmptyProps {
  onImportClick: () => void;
}

export function WorkbenchEmpty({ onImportClick }: WorkbenchEmptyProps) {
  return (
    <div className="empty">
      <div className="empty__mark">
        <Icon name="paper-plane" size={56} />
      </div>
      <p className="empty__title">No endpoint open</p>
      <p className="empty__hint">
        Import an OpenAPI spec or cURL request, then pick an endpoint from the
        left.
      </p>

      <div className="empty__shortcuts">
        <button type="button" className="btn btn--ghost" onClick={onImportClick}>
          <Icon name="import" size={14} />
          <span>Import source</span>
        </button>
        <div className="empty__row">
          <span>Open Endpoint</span>
          <kbd>click</kbd>
        </div>
        <div className="empty__row">
          <span>Toggle Theme</span>
          <kbd>top-right ☼</kbd>
        </div>
      </div>
    </div>
  );
}
