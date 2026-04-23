export type DeliveryStage =
  | "planned"
  | "scaffolded"
  | "partial"
  | "not_implemented";

export interface CapabilityStatus {
  name: string;
  stage: DeliveryStage;
  note: string;
}

export interface AppBootstrapSummary {
  project_name: string;
  current_phase: string;
  ui_surfaces: string[];
  parser_capabilities: CapabilityStatus[];
  storage_capabilities: CapabilityStatus[];
  provider_capabilities: CapabilityStatus[];
  gateway_capabilities: CapabilityStatus[];
}

export interface EndpointPreview {
  method: string;
  path: string;
  title: string;
  source: "openapi" | "curl";
  status: "success" | "empty" | "error";
  summary: string;
  request_shape: string[];
  response_shape: string[];
}

export interface ProviderPreview {
  name: string;
  mode: string;
  status: string;
  note: string;
}

export interface PhasePreview {
  name: string;
  summary: string;
}

export interface CanonicalParameter {
  name: string;
  location: "path" | "query" | "header" | "cookie";
  description?: string | null;
  required: boolean;
  schema: {
    node_type: string;
    properties: Record<string, unknown>;
    items?: unknown;
  };
}

export interface CanonicalRequestBody {
  content_type: string;
  required: boolean;
  schema: {
    node_type: string;
    properties: Record<string, unknown>;
    items?: unknown;
  };
}

export interface CanonicalResponse {
  status_code: string;
  description?: string | null;
  content_type: string;
  schema?: {
    node_type: string;
    properties: Record<string, unknown>;
    items?: unknown;
  } | null;
}

export type AuthScheme =
  | "http_bearer"
  | "http_basic"
  | "api_key_header"
  | "oauth2"
  | "other";

export interface AuthRequirementHint {
  scheme: AuthScheme;
  header_name: string;
  value_prefix?: string | null;
  description?: string | null;
}

export interface CanonicalEndpoint {
  operation_id?: string | null;
  method: string;
  path: string;
  summary?: string | null;
  description?: string | null;
  tags: string[];
  parameters: CanonicalParameter[];
  request_body?: CanonicalRequestBody | null;
  responses: CanonicalResponse[];
  examples: Array<{
    kind: string;
    title: string;
    payload?: unknown;
    note?: string | null;
  }>;
  auth?: AuthRequirementHint | null;
}

export interface CanonicalApiCollection {
  id: string;
  name: string;
  source: "openapi" | "curl";
  description?: string | null;
  endpoints: CanonicalEndpoint[];
}

export interface ImportResult {
  collection_id: string;
  collection_name: string;
  endpoint_count: number;
  database_url: string;
}

export interface StoredCollectionSummary {
  id: string;
  name: string;
  source_kind: string;
  endpoint_count: number;
}

export interface StoredEndpointSummary {
  id: string;
  collection_id: string;
  method: string;
  path: string;
  summary?: string | null;
}

export type InspectorKey =
  | "params"
  | "headers"
  | "body"
  | "responses"
  | "schema"
  | "ai";

export type ExampleKind = "success" | "empty" | "error";

export type MockExampleKind = ExampleKind;

export interface MockExample {
  kind: MockExampleKind;
  title: string;
  payload: unknown;
  note?: string | null;
}

export interface GatewayRouteSummary {
  method: string;
  path: string;
  collection_name: string;
  operation_id?: string | null;
  summary?: string | null;
  selected_example?: MockExampleKind | null;
  available_examples: MockExampleKind[];
  latency_ms?: number | null;
}

export interface RequiredHeader {
  name: string;
  value_prefix?: string | null;
  value_equals?: string | null;
}

export interface RateLimitRule {
  limit: number;
  window_ms: number;
}

export interface GatewayConfig {
  host: string;
  port: number;
  cors_enabled: boolean;
  example_overrides: Record<string, MockExampleKind>;
  default_latency_ms?: number | null;
  latency_overrides?: Record<string, number>;
  error_rate?: number;
  capture_bodies?: boolean;
  response_headers?: Record<string, Record<string, string>>;
  required_headers?: Record<string, RequiredHeader[]>;
  rate_limits?: Record<string, RateLimitRule>;
}

export interface GatewayStatus {
  running: boolean;
  bind_address?: string | null;
  route_count: number;
  started_at_epoch_ms?: number | null;
  config: GatewayConfig;
  routes: GatewayRouteSummary[];
}

export interface RequestLogEntry {
  at_epoch_ms: number;
  method: string;
  path: string;
  query?: string | null;
  matched_route?: string | null;
  collection_name?: string | null;
  status: number;
  kind?: MockExampleKind | null;
  source: string;
  latency_ms: number;
  request_body?: string | null;
}

export interface ProviderConfigDraft {
  provider_name: string;
  base_url: string;
  model: string;
  api_key_env: string;
}

export interface GenerationRequest {
  endpoint: CanonicalEndpoint;
  intent: MockExampleKind;
  provider: ProviderConfigDraft;
  collection_id?: string;
  persist?: boolean;
  database_url?: string;
  api_key_override?: string;
}

export type ThemeMode = "dark" | "light";

export interface EndpointTab {
  id: string;
  collectionId: string;
  collectionName: string;
  method: string;
  path: string;
  endpoint: CanonicalEndpoint;
  inspector: InspectorKey;
  example: ExampleKind;
}

export interface SidebarCollection {
  id: string;
  name: string;
  origin: "imported" | "preview" | "fallback";
  source: "openapi" | "curl";
  endpoints: CanonicalEndpoint[];
}
