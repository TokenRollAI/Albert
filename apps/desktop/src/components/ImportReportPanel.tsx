import { Icon } from "./Icon";
import { describeImportDiff } from "../hooks/useImportActions";
import type {
  CanonicalApiCollection,
  CanonicalEndpoint,
  ImportDiffSummary,
  ImportEndpointChange,
  ImportResult
} from "../types";

export interface ImportReport {
  result: ImportResult;
  collection: CanonicalApiCollection;
}

interface ImportReportPanelProps {
  open: boolean;
  report: ImportReport | null;
  onClose: () => void;
  onImportClick: () => void;
  onOpenEndpoint: (
    collection: CanonicalApiCollection,
    endpoint: CanonicalEndpoint
  ) => void;
  onPreviewEndpointPrompt?: (
    collection: CanonicalApiCollection,
    endpoint: CanonicalEndpoint,
    change: ImportEndpointChange
  ) => void;
  onRefreshEndpointMock?: (
    collection: CanonicalApiCollection,
    endpoint: CanonicalEndpoint,
    change: ImportEndpointChange
  ) => void;
  onRefreshChangedMocks?: (
    collection: CanonicalApiCollection,
    changes: Array<{
      change: ImportEndpointChange;
      endpoint: CanonicalEndpoint;
    }>
  ) => void;
}

function endpointKey(method: string, path: string): string {
  return `${method.toUpperCase()} ${path}`;
}

function findEndpoint(
  collection: CanonicalApiCollection,
  change: ImportEndpointChange
): CanonicalEndpoint | null {
  return (
    collection.endpoints.find(
      (endpoint) =>
        endpoint.method.toUpperCase() === change.method.toUpperCase() &&
        endpoint.path === change.path
    ) ?? null
  );
}

function methodClass(method: string): string {
  return `method--${method.toLowerCase()}`;
}

function changedTotal(diff: ImportDiffSummary): number {
  return diff.added.length + diff.changed.length + diff.removed.length;
}

function refreshableChangedEndpoints(
  collection: CanonicalApiCollection,
  changes: ImportEndpointChange[]
): Array<{ change: ImportEndpointChange; endpoint: CanonicalEndpoint }> {
  return changes.flatMap((change) => {
    if (!change.reasons?.length) return [];
    const endpoint = findEndpoint(collection, change);
    return endpoint ? [{ change, endpoint }] : [];
  });
}

function ChangeList({
  title,
  changes,
  collection,
  onOpenEndpoint,
  onPreviewEndpointPrompt,
  onRefreshEndpointMock,
  openable
}: {
  title: string;
  changes: ImportEndpointChange[];
  collection: CanonicalApiCollection;
  onOpenEndpoint: (
    collection: CanonicalApiCollection,
    endpoint: CanonicalEndpoint
  ) => void;
  onPreviewEndpointPrompt?: (
    collection: CanonicalApiCollection,
    endpoint: CanonicalEndpoint,
    change: ImportEndpointChange
  ) => void;
  onRefreshEndpointMock?: (
    collection: CanonicalApiCollection,
    endpoint: CanonicalEndpoint,
    change: ImportEndpointChange
  ) => void;
  openable: boolean;
}) {
  if (changes.length === 0) return null;

  return (
    <section className="import-report__section" aria-label={title}>
      <div className="import-report__section-head">
        <h3>{title}</h3>
        <span>{changes.length}</span>
      </div>
      <div className="import-report__changes">
        {changes.map((change) => {
          const endpoint = openable ? findEndpoint(collection, change) : null;
          return (
            <article
              key={endpointKey(change.method, change.path)}
              className="import-report__change"
            >
              <div className="import-report__endpoint">
                <span className={`method-pill ${methodClass(change.method)}`}>
                  {change.method.toUpperCase()}
                </span>
                <div>
                  <code>{change.path}</code>
                  {change.summary ? <p>{change.summary}</p> : null}
                  {change.reasons && change.reasons.length > 0 ? (
                    <ul className="import-report__reasons">
                      {change.reasons.map((reason) => (
                        <li key={reason}>{reason}</li>
                      ))}
                    </ul>
                  ) : null}
                  {change.details && change.details.length > 0 ? (
                    <ul className="import-report__details">
                      {change.details.map((detail) => (
                        <li key={detail}>{detail}</li>
                      ))}
                    </ul>
                  ) : null}
                </div>
              </div>
              {endpoint ? (
                <div className="import-report__actions">
                  <button
                    type="button"
                    className="btn btn--ghost btn--sm"
                    onClick={() => onOpenEndpoint(collection, endpoint)}
                  >
                    <Icon name="play" size={12} />
                    <span>Open</span>
                  </button>
                  {onPreviewEndpointPrompt ? (
                    <button
                      type="button"
                      className="btn btn--ghost btn--sm"
                      onClick={() =>
                        onPreviewEndpointPrompt(collection, endpoint, change)
                      }
                    >
                      <Icon name="sparkles" size={12} />
                      <span>Prompt</span>
                    </button>
                  ) : null}
                  {onRefreshEndpointMock && change.reasons?.length ? (
                    <button
                      type="button"
                      className="btn btn--ghost btn--sm"
                      onClick={() =>
                        onRefreshEndpointMock(collection, endpoint, change)
                      }
                    >
                      <Icon name="refresh" size={12} />
                      <span>Refresh</span>
                    </button>
                  ) : null}
                </div>
              ) : null}
            </article>
          );
        })}
      </div>
    </section>
  );
}

export function ImportReportPanel({
  open,
  report,
  onClose,
  onImportClick,
  onOpenEndpoint,
  onPreviewEndpointPrompt,
  onRefreshEndpointMock,
  onRefreshChangedMocks
}: ImportReportPanelProps) {
  if (!open) return null;

  const refreshableChanged =
    report && onRefreshChangedMocks
      ? refreshableChangedEndpoints(report.collection, report.result.diff.changed)
      : [];

  return (
    <div className="drawer" role="dialog" aria-label="Import report">
      <div className="drawer__backdrop" onClick={onClose} />
      <div className="drawer__panel drawer__panel--lg">
        <header className="drawer__head">
          <div className="drawer__title">
            <Icon name="import" size={16} />
            <h2>Import Report</h2>
            {report ? (
              <span
                className={
                  changedTotal(report.result.diff) > 0
                    ? "pill pill--ok"
                    : "pill pill--idle"
                }
              >
                {changedTotal(report.result.diff) > 0 ? "changed" : "no changes"}
              </span>
            ) : null}
          </div>
          <div className="drawer__head-actions">
            {report && onRefreshChangedMocks && refreshableChanged.length > 0 ? (
              <button
                type="button"
                className="btn btn--ghost btn--sm"
                onClick={() =>
                  onRefreshChangedMocks(report.collection, refreshableChanged)
                }
              >
                <Icon name="refresh" size={12} />
                <span>Refresh changed ({refreshableChanged.length})</span>
              </button>
            ) : null}
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
              aria-label="Close import report"
            >
              <Icon name="close" size={16} />
            </button>
          </div>
        </header>

        <div className="drawer__body">
          {report ? (
            <>
              <section className="workspace-summary" aria-label="Import summary">
                <div className="workspace-summary__item">
                  <span className="workspace-summary__value">
                    {report.result.diff.added.length}
                  </span>
                  <span className="workspace-summary__label">added</span>
                </div>
                <div className="workspace-summary__item">
                  <span className="workspace-summary__value">
                    {report.result.diff.changed.length}
                  </span>
                  <span className="workspace-summary__label">changed</span>
                </div>
                <div className="workspace-summary__item">
                  <span className="workspace-summary__value">
                    {report.result.diff.removed.length}
                  </span>
                  <span className="workspace-summary__label">removed</span>
                </div>
                <div className="workspace-summary__item">
                  <span className="workspace-summary__value">
                    {report.result.diff.unchanged}
                  </span>
                  <span className="workspace-summary__label">unchanged</span>
                </div>
              </section>

              <section className="panel import-report__overview">
                <h3>{report.result.collection_name}</h3>
                <p>{describeImportDiff(report.result.diff)}</p>
                <p>{report.result.endpoint_count} endpoint(s) in SQLite</p>
              </section>

              <ChangeList
                title="Added"
                changes={report.result.diff.added}
                collection={report.collection}
                onOpenEndpoint={onOpenEndpoint}
                onPreviewEndpointPrompt={onPreviewEndpointPrompt}
                openable={true}
              />
              <ChangeList
                title="Changed"
                changes={report.result.diff.changed}
                collection={report.collection}
                onOpenEndpoint={onOpenEndpoint}
                onPreviewEndpointPrompt={onPreviewEndpointPrompt}
                onRefreshEndpointMock={onRefreshEndpointMock}
                openable={true}
              />
              <ChangeList
                title="Removed"
                changes={report.result.diff.removed}
                collection={report.collection}
                onOpenEndpoint={onOpenEndpoint}
                openable={false}
              />
              {changedTotal(report.result.diff) === 0 ? (
                <section className="panel workspace-empty">
                  <Icon name="info" size={20} />
                  <h3>No endpoint changes</h3>
                  <p>The latest import matched the previous endpoint contract.</p>
                </section>
              ) : null}
            </>
          ) : (
            <section className="panel workspace-empty">
              <Icon name="import" size={20} />
              <h3>No import report yet</h3>
              <p>Import a source to review added, changed, and removed endpoints.</p>
              <button
                type="button"
                className="btn btn--primary btn--sm"
                onClick={onImportClick}
              >
                <Icon name="import" size={12} />
                <span>Import source</span>
              </button>
            </section>
          )}
        </div>
      </div>
    </div>
  );
}
