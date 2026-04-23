import { Markdown } from "./Markdown";
import { SchemaTree, type SchemaNodeShape } from "./SchemaTree";
import type {
  CanonicalEndpoint,
  CanonicalParameter,
  EndpointTab,
  InspectorKey
} from "../types";

interface RequestPanelProps {
  tab: EndpointTab;
  onSelectInspector: (key: InspectorKey) => void;
}

const INSPECTORS: { key: InspectorKey; label: string }[] = [
  { key: "params", label: "Params" },
  { key: "headers", label: "Headers" },
  { key: "body", label: "Body" },
  { key: "responses", label: "Responses" },
  { key: "schema", label: "Schema" },
  { key: "ai", label: "AI Mock" }
];

export function RequestPanel({ tab, onSelectInspector }: RequestPanelProps) {
  const { endpoint, inspector } = tab;

  return (
    <section className="request-panel">
      {endpoint.description ? (
        <div className="endpoint-desc">
          <Markdown source={endpoint.description} />
        </div>
      ) : null}
      <nav className="request-panel__tabs" role="tablist">
        {INSPECTORS.map((item) => {
          const active = item.key === inspector;
          const dot = hasContent(endpoint, item.key);
          return (
            <button
              key={item.key}
              type="button"
              role="tab"
              aria-selected={active}
              className={active ? "sub-tab sub-tab--active" : "sub-tab"}
              onClick={() => onSelectInspector(item.key)}
            >
              <span>{item.label}</span>
              {dot ? <span className="sub-tab__dot" aria-hidden="true" /> : null}
            </button>
          );
        })}
      </nav>

      <div className="request-panel__body">
        {inspector === "params" && <ParamsView endpoint={endpoint} />}
        {inspector === "headers" && <HeadersView endpoint={endpoint} />}
        {inspector === "body" && <BodyView endpoint={endpoint} />}
        {inspector === "responses" && <ResponsesView endpoint={endpoint} />}
        {inspector === "schema" && <SchemaView endpoint={endpoint} />}
        {inspector === "ai" && <AiView />}
      </div>
    </section>
  );
}

function hasContent(endpoint: CanonicalEndpoint, key: InspectorKey): boolean {
  switch (key) {
    case "params":
      return endpoint.parameters.filter((p) => p.location !== "header").length > 0;
    case "headers":
      return endpoint.parameters.some((p) => p.location === "header");
    case "body":
      return Boolean(endpoint.request_body);
    case "responses":
      return endpoint.responses.length > 0;
    case "schema":
      return Boolean(endpoint.request_body) || endpoint.responses.some((r) => Boolean(r.schema));
    case "ai":
      return false;
    default:
      return false;
  }
}

function ParamsView({ endpoint }: { endpoint: CanonicalEndpoint }) {
  const params = endpoint.parameters.filter((p) => p.location !== "header");
  if (params.length === 0) {
    return <EmptyHint label="No query, path, or cookie parameters." />;
  }
  return <ParamTable title="Query / Path / Cookie" params={params} />;
}

function HeadersView({ endpoint }: { endpoint: CanonicalEndpoint }) {
  const headers = endpoint.parameters.filter((p) => p.location === "header");
  if (headers.length === 0) {
    return <EmptyHint label="No declared headers." />;
  }
  return <ParamTable title="Headers" params={headers} />;
}

function BodyView({ endpoint }: { endpoint: CanonicalEndpoint }) {
  const body = endpoint.request_body;
  if (!body) {
    return <EmptyHint label="This endpoint has no request body." />;
  }
  return (
    <div className="kv-block">
      <h4>Request body</h4>
      <div className="kv-row">
        <span className="kv-row__label">Content-Type</span>
        <code>{body.content_type}</code>
      </div>
      <div className="kv-row">
        <span className="kv-row__label">Required</span>
        <code>{String(body.required)}</code>
      </div>
      <div className="kv-row">
        <span className="kv-row__label">Root</span>
        <code>{body.schema.node_type}</code>
      </div>
      {Object.keys(body.schema.properties ?? {}).length > 0 ? (
        <div className="kv-row kv-row--wrap">
          <span className="kv-row__label">Properties</span>
          <ul className="chip-list">
            {Object.keys(body.schema.properties).map((key) => (
              <li key={key} className="chip">
                {key}
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </div>
  );
}

function ResponsesView({ endpoint }: { endpoint: CanonicalEndpoint }) {
  if (endpoint.responses.length === 0) {
    return <EmptyHint label="No responses declared." />;
  }
  return (
    <table className="data-table">
      <thead>
        <tr>
          <th style={{ width: 80 }}>Status</th>
          <th style={{ width: 160 }}>Content-Type</th>
          <th>Schema</th>
          <th>Description</th>
        </tr>
      </thead>
      <tbody>
        {endpoint.responses.map((response) => (
          <tr key={`${response.status_code}:${response.content_type}`}>
            <td>
              <code>{response.status_code}</code>
            </td>
            <td>
              <code>{response.content_type}</code>
            </td>
            <td>
              {response.schema ? (
                <code>{response.schema.node_type}</code>
              ) : (
                <span className="muted">—</span>
              )}
            </td>
            <td>{response.description ?? <span className="muted">—</span>}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function SchemaView({ endpoint }: { endpoint: CanonicalEndpoint }) {
  const requestSchema = endpoint.request_body?.schema ?? null;
  const responseSchemas = endpoint.responses
    .map((r) => ({ status: r.status_code, schema: r.schema }))
    .filter((entry) => entry.schema !== null && entry.schema !== undefined);

  if (!requestSchema && responseSchemas.length === 0) {
    return <EmptyHint label="No schemas available." />;
  }

  return (
    <div className="schema-stack">
      {requestSchema ? (
        <div className="schema-card">
          <header>Request</header>
          <SchemaTree schema={requestSchema as SchemaNodeShape} />
        </div>
      ) : null}
      {responseSchemas.map((entry) => (
        <div key={entry.status} className="schema-card">
          <header>Response · {entry.status}</header>
          <SchemaTree schema={entry.schema as SchemaNodeShape} />
        </div>
      ))}
    </div>
  );
}

function AiView() {
  return (
    <div className="ai-view">
      <div className="ai-view__card">
        <h4>AI Mock Generation</h4>
        <p>
          OpenAI-backed mock payload generation arrives in Phase 4. Once wired,
          this panel will let you describe the scenario, pick a provider, and
          persist the generated payload as a mock example.
        </p>
        <p className="muted">Not implemented yet.</p>
      </div>
    </div>
  );
}

function ParamTable({
  title,
  params
}: {
  title: string;
  params: CanonicalParameter[];
}) {
  return (
    <div className="data-table-wrap">
      <h4>{title}</h4>
      <table className="data-table">
        <thead>
          <tr>
            <th style={{ width: 32 }} />
            <th>Name</th>
            <th style={{ width: 80 }}>In</th>
            <th style={{ width: 120 }}>Type</th>
            <th>Description</th>
          </tr>
        </thead>
        <tbody>
          {params.map((param) => (
            <tr key={`${param.location}:${param.name}`}>
              <td>
                <span
                  className={
                    param.required ? "req-dot req-dot--on" : "req-dot"
                  }
                  title={param.required ? "required" : "optional"}
                />
              </td>
              <td>
                <code>{param.name}</code>
              </td>
              <td>
                <span className="param-in">{param.location}</span>
              </td>
              <td>
                <code>{param.schema.node_type}</code>
              </td>
              <td>
                {param.description ?? <span className="muted">—</span>}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function EmptyHint({ label }: { label: string }) {
  return <p className="panel-empty">{label}</p>;
}
