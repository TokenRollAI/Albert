import type {
  AppBootstrapSummary,
  EndpointPreview,
  PhasePreview,
  ProviderPreview
} from "../types";

export const fallbackSummary: AppBootstrapSummary = {
  project_name: "Albert",
  current_phase: "Phase 1 - Foundation",
  ui_surfaces: [
    "Overview",
    "Import",
    "Endpoints",
    "Providers",
    "Mock Server"
  ],
  parser_capabilities: [
    {
      name: "OpenAPI parser",
      stage: "scaffolded",
      note: "Trait and placeholder implementation exist."
    },
    {
      name: "cURL parser",
      stage: "scaffolded",
      note: "Tokenizer boundary is reserved for Phase 2."
    },
    {
      name: "Canonical schema transform",
      stage: "partial",
      note: "Core types exist, transformation rules still need implementation."
    }
  ],
  storage_capabilities: [
    {
      name: "SQLite table plan",
      stage: "scaffolded",
      note: "Projects, endpoints, schemas, examples, and providers are modeled."
    },
    {
      name: "Migration script",
      stage: "partial",
      note: "Initial SQL file exists as a contract seed."
    },
    {
      name: "Repository implementation",
      stage: "not_implemented",
      note: "Concrete SQLite wiring is deferred to Phase 2."
    }
  ],
  provider_capabilities: [
    {
      name: "OpenAI adapter",
      stage: "scaffolded",
      note: "Chat Completions boundary is defined."
    },
    {
      name: "Responses API",
      stage: "planned",
      note: "Documented as a future extension point."
    }
  ],
  gateway_capabilities: [
    {
      name: "Static mock strategy",
      stage: "scaffolded",
      note: "Success, empty, and error example states are modeled."
    },
    {
      name: "HTTP listener runtime",
      stage: "not_implemented",
      note: "Rust server integration begins in Phase 3."
    }
  ]
};

export const endpointPreviews: EndpointPreview[] = [
  {
    method: "GET",
    path: "/api/orders",
    title: "List orders",
    source: "openapi",
    status: "success",
    summary: "Returns a paginated collection of orders.",
    request_shape: ["query.status", "query.page", "header.authorization"],
    response_shape: ["data[]", "meta.page", "meta.total"]
  },
  {
    method: "GET",
    path: "/api/orders/{id}",
    title: "Get order detail",
    source: "openapi",
    status: "empty",
    summary: "Resolves a single order view or an empty placeholder response.",
    request_shape: ["path.id", "header.authorization"],
    response_shape: ["data.id", "data.status", "data.total_amount"]
  },
  {
    method: "POST",
    path: "/api/orders",
    title: "Create order",
    source: "curl",
    status: "error",
    summary: "Creates an order and models a validation failure branch.",
    request_shape: ["body.customer_id", "body.items[]", "header.content-type"],
    response_shape: ["data.id", "data.created_at", "error.message"]
  }
];

export const providerPreviews: ProviderPreview[] = [
  {
    name: "OpenAI Chat Completions",
    mode: "Primary",
    status: "Scaffolded",
    note: "API key, base URL, and model fields are reserved."
  },
  {
    name: "OpenAI Responses",
    mode: "Future",
    status: "Planned",
    note: "Documented for a later phase, intentionally not wired now."
  }
];

export const phasePreviews: PhasePreview[] = [
  {
    name: "Phase 1",
    summary: "Docs, shell UI, canonical model, and workspace boundaries."
  },
  {
    name: "Phase 2",
    summary: "OpenAPI and cURL ingestion with SQLite persistence."
  },
  {
    name: "Phase 3",
    summary: "Static mock runtime with route matching and CORS handling."
  },
  {
    name: "Phase 4",
    summary: "OpenAI-backed structured mock generation."
  }
];

export const openQuestions = [
  "OpenAPI 3.0 and 3.1 need an explicit compatibility target before parser implementation expands.",
  "The ALBRT visual style is still a naming-system decision, not a settled product rule.",
  "We still need to decide whether raw source snapshots are mandatory persistence artifacts."
];

