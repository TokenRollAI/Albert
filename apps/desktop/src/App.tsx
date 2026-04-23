import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { PanelCard } from "./components/PanelCard";
import { StatusPill } from "./components/StatusPill";
import {
  endpointPreviews,
  fallbackSummary,
  openQuestions,
  phasePreviews,
  providerPreviews
} from "./data/fallback";
import type { AppBootstrapSummary } from "./types";

type SectionKey =
  | "overview"
  | "import"
  | "endpoints"
  | "providers"
  | "server";

const sections: Array<{ key: SectionKey; label: string }> = [
  { key: "overview", label: "Overview" },
  { key: "import", label: "Import" },
  { key: "endpoints", label: "Endpoints" },
  { key: "providers", label: "Providers" },
  { key: "server", label: "Mock Server" }
];

function App() {
  const [activeSection, setActiveSection] = useState<SectionKey>("overview");
  const [selectedPath, setSelectedPath] = useState(endpointPreviews[0]?.path ?? "");
  const [summary, setSummary] = useState<AppBootstrapSummary>(fallbackSummary);
  const [runtimeSource, setRuntimeSource] = useState("Scaffold");

  useEffect(() => {
    let active = true;

    async function loadBootstrap() {
      try {
        const data = await invoke<AppBootstrapSummary>("bootstrap_summary");

        if (!active) {
          return;
        }

        setSummary(data);
        setRuntimeSource("Tauri Runtime");
      } catch {
        if (!active) {
          return;
        }

        setRuntimeSource("Local Fallback");
      }
    }

    loadBootstrap();

    return () => {
      active = false;
    };
  }, []);

  const selectedEndpoint =
    endpointPreviews.find((item) => item.path === selectedPath) ?? endpointPreviews[0];

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand-block">
          <p className="brand-block__kicker">AI-native API mocking</p>
          <h1>Albert</h1>
          <p className="brand-block__copy">
            A desktop-first control plane for canonical API schemas, static mock
            assets, and future OpenAI-backed generation.
          </p>
        </div>

        <nav className="sidebar__nav">
          {sections.map((section) => (
            <button
              key={section.key}
              type="button"
              className={
                activeSection === section.key
                  ? "sidebar__nav-item sidebar__nav-item--active"
                  : "sidebar__nav-item"
              }
              onClick={() => setActiveSection(section.key)}
            >
              <span>{section.label}</span>
            </button>
          ))}
        </nav>

        <div className="sidebar__footer">
          <p className="sidebar__footer-label">Runtime</p>
          <strong>{runtimeSource}</strong>
          <p>{summary.current_phase}</p>
        </div>
      </aside>

      <main className="content">
        <header className="hero">
          <div>
            <p className="hero__eyebrow">Foundation workspace</p>
            <h2>Build the mock stack before the complexity arrives.</h2>
            <p className="hero__copy">
              Phase 1 keeps the system intentionally narrow: canonical schema,
              parser boundaries, storage contracts, a desktop shell, and
              explicit not-implemented seams.
            </p>
          </div>
          <div className="hero__meta">
            <div>
              <span>Project</span>
              <strong>{summary.project_name}</strong>
            </div>
            <div>
              <span>UI Surfaces</span>
              <strong>{summary.ui_surfaces.length}</strong>
            </div>
          </div>
        </header>

        {activeSection === "overview" ? (
          <div className="grid">
            <PanelCard
              eyebrow="Status"
              title="Capability map"
              aside={<StatusPill stage="scaffolded" />}
            >
              <div className="capability-columns">
                <CapabilityGroup
                  title="Parser"
                  items={summary.parser_capabilities}
                />
                <CapabilityGroup
                  title="Storage"
                  items={summary.storage_capabilities}
                />
                <CapabilityGroup
                  title="Provider"
                  items={summary.provider_capabilities}
                />
                <CapabilityGroup
                  title="Gateway"
                  items={summary.gateway_capabilities}
                />
              </div>
            </PanelCard>

            <PanelCard eyebrow="Roadmap" title="Delivery phases">
              <div className="phase-list">
                {phasePreviews.map((phase) => (
                  <article key={phase.name} className="phase-item">
                    <h3>{phase.name}</h3>
                    <p>{phase.summary}</p>
                  </article>
                ))}
              </div>
            </PanelCard>

            <PanelCard eyebrow="Decisions" title="Open design questions">
              <ul className="plain-list">
                {openQuestions.map((question) => (
                  <li key={question}>{question}</li>
                ))}
              </ul>
            </PanelCard>
          </div>
        ) : null}

        {activeSection === "import" ? (
          <div className="grid">
            <PanelCard eyebrow="Ingestion" title="Supported inputs">
              <div className="import-grid">
                <article className="import-card">
                  <h3>OpenAPI / Swagger</h3>
                  <p>
                    Import JSON or YAML and normalize paths, methods, parameter
                    rules, and response schemas into the Albert canonical model.
                  </p>
                </article>
                <article className="import-card">
                  <h3>cURL</h3>
                  <p>
                    Accept pasted terminal requests and extract URL, headers,
                    query parameters, and body structure as parser input.
                  </p>
                </article>
              </div>
            </PanelCard>

            <PanelCard eyebrow="Flow" title="Planned import pipeline">
              <ol className="ordered-list">
                <li>Receive OpenAPI or cURL content from the desktop UI.</li>
                <li>Select the matching parser and emit canonical endpoint data.</li>
                <li>Persist normalized assets and mock example placeholders.</li>
                <li>Expose imported endpoints to the control panel.</li>
              </ol>
            </PanelCard>
          </div>
        ) : null}

        {activeSection === "endpoints" ? (
          <div className="grid grid--two-column">
            <PanelCard eyebrow="Catalog" title="Endpoint inventory">
              <div className="endpoint-list">
                {endpointPreviews.map((endpoint) => (
                  <button
                    key={`${endpoint.method}:${endpoint.path}`}
                    type="button"
                    className={
                      selectedEndpoint?.path === endpoint.path
                        ? "endpoint-list__item endpoint-list__item--active"
                        : "endpoint-list__item"
                    }
                    onClick={() => setSelectedPath(endpoint.path)}
                  >
                    <div className="endpoint-list__meta">
                      <strong>
                        {endpoint.method} {endpoint.path}
                      </strong>
                      <span>{endpoint.title}</span>
                    </div>
                    <StatusPill stage={endpoint.status} />
                  </button>
                ))}
              </div>
            </PanelCard>

            <PanelCard eyebrow="Detail" title={selectedEndpoint.title}>
              <div className="detail-stack">
                <p>{selectedEndpoint.summary}</p>
                <div className="detail-shelf">
                  <span>Source</span>
                  <strong>{selectedEndpoint.source}</strong>
                </div>
                <div>
                  <h3>Request shape</h3>
                  <ul className="token-list">
                    {selectedEndpoint.request_shape.map((shape) => (
                      <li key={shape}>{shape}</li>
                    ))}
                  </ul>
                </div>
                <div>
                  <h3>Response shape</h3>
                  <ul className="token-list">
                    {selectedEndpoint.response_shape.map((shape) => (
                      <li key={shape}>{shape}</li>
                    ))}
                  </ul>
                </div>
              </div>
            </PanelCard>
          </div>
        ) : null}

        {activeSection === "providers" ? (
          <div className="grid">
            <PanelCard eyebrow="Provider" title="OpenAI adapter plan">
              <div className="provider-list">
                {providerPreviews.map((provider) => (
                  <article key={provider.name} className="provider-item">
                    <div>
                      <h3>{provider.name}</h3>
                      <p>{provider.note}</p>
                    </div>
                    <div className="provider-item__meta">
                      <span>{provider.mode}</span>
                      <strong>{provider.status}</strong>
                    </div>
                  </article>
                ))}
              </div>
            </PanelCard>

            <PanelCard eyebrow="Constraints" title="Phase 1 provider boundary">
              <ul className="plain-list">
                <li>OpenAI only</li>
                <li>Chat Completions first</li>
                <li>No tool calling, streaming, or reasoning controls yet</li>
                <li>Responses API stays documented as a next-step seam</li>
              </ul>
            </PanelCard>
          </div>
        ) : null}

        {activeSection === "server" ? (
          <div className="grid">
            <PanelCard eyebrow="Runtime" title="Mock server plan">
              <div className="server-matrix">
                <article>
                  <h3>Phase 1</h3>
                  <p>Server contracts only. No active listener.</p>
                </article>
                <article>
                  <h3>Phase 3</h3>
                  <p>Static response routing, CORS, and basic path matching.</p>
                </article>
                <article>
                  <h3>Future</h3>
                  <p>AI-driven generation, cache policies, and diff-aware refresh.</p>
                </article>
              </div>
            </PanelCard>

            <PanelCard eyebrow="Mock Examples" title="Supported states">
              <div className="mock-state-row">
                <StatusPill stage="success" />
                <StatusPill stage="empty" />
                <StatusPill stage="error" />
              </div>
              <p className="muted-copy">
                Static examples are the only mock strategy in Phase 1. They are
                modeled now so the gateway can consume them later without schema
                redesign.
              </p>
            </PanelCard>
          </div>
        ) : null}
      </main>
    </div>
  );
}

function CapabilityGroup({
  title,
  items
}: {
  title: string;
  items: AppBootstrapSummary["parser_capabilities"];
}) {
  return (
    <div className="capability-group">
      <h3>{title}</h3>
      <ul className="plain-list">
        {items.map((item) => (
          <li key={item.name} className="capability-item">
            <div className="capability-item__row">
              <strong>{item.name}</strong>
              <StatusPill stage={item.stage} />
            </div>
            <p>{item.note}</p>
          </li>
        ))}
      </ul>
    </div>
  );
}

export default App;

