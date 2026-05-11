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

export interface CanonicalSchemaNode {
  node_type: string;
  description?: string | null;
  required?: boolean;
  nullable?: boolean;
  properties?: Record<string, CanonicalSchemaNode>;
  items?: CanonicalSchemaNode | null;
  enum_values?: unknown[];
  example?: unknown;
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
  schema: CanonicalSchemaNode;
}

export interface CanonicalRequestBody {
  content_type: string;
  required: boolean;
  schema: CanonicalSchemaNode;
}

export interface CanonicalResponse {
  status_code: string;
  description?: string | null;
  content_type: string;
  schema?: CanonicalSchemaNode | null;
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

export interface ImportedApiCollection extends CanonicalApiCollection {
  created_at: string;
  updated_at: string;
  endpoint_count: number;
}

export interface ImportEndpointChange {
  method: string;
  path: string;
  summary?: string | null;
  reasons?: string[];
  details?: string[];
}

export interface ImportDiffSummary {
  added: ImportEndpointChange[];
  removed: ImportEndpointChange[];
  changed: ImportEndpointChange[];
  unchanged: number;
}

export interface ImportResult {
  collection_id: string;
  collection_name: string;
  endpoint_count: number;
  database_url: string;
  diff: ImportDiffSummary;
}

export interface StoredCollectionSummary {
  id: string;
  name: string;
  source_kind: string;
  endpoint_count: number;
  created_at: string;
  updated_at: string;
}

export interface StoredEndpointSummary {
  id: string;
  collection_id: string;
  method: string;
  path: string;
  summary?: string | null;
}

export interface StoredScenarioSummary {
  id: string;
  name: string;
  created_at: string;
  updated_at: string;
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

export type RequestCondition =
  | { source: "query"; name: string; equals: string }
  | { source: "header"; name: string; equals: string }
  | { source: "body"; path: string; equals: unknown };

export interface ConditionalExampleRule {
  name: string;
  example: MockExampleKind;
  when: RequestCondition[];
}

export interface GatewayConfig {
  host: string;
  port: number;
  cors_enabled: boolean;
  example_overrides: Record<string, MockExampleKind>;
  conditional_example_rules?: Record<string, ConditionalExampleRule[]>;
  use_request_cache?: boolean;
  request_cache_entries?: Record<string, unknown>;
  default_latency_ms?: number | null;
  latency_overrides?: Record<string, number>;
  latency_jitter_ms?: Record<string, number>;
  error_rate?: number;
  capture_bodies?: boolean;
  enforce_request_bodies?: boolean;
  response_headers?: Record<string, Record<string, string>>;
  required_headers?: Record<string, RequiredHeader[]>;
  rate_limits?: Record<string, RateLimitRule>;
  status_overrides?: Record<string, number>;
  proxy_upstream?: string | null;
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
  /// Correlation id emitted on the response `x-request-id` header.
  /// Honored from the client when supplied, otherwise generated.
  request_id?: string | null;
}

export interface RequestCacheEntry {
  id: string;
  collection_id: string;
  method: string;
  path: string;
  fingerprint: string;
  request_snapshot: unknown;
  response_snapshot: unknown;
  hit_count: number;
  first_seen_at: string;
  last_seen_at: string;
}

export interface GenerationContext {
  request_snapshot?: unknown;
  response_snapshot?: unknown;
  note?: string | null;
}

export interface ProviderConfigDraft {
  provider_name: string;
  environment?: string | null;
  base_url: string;
  model: string;
  api_key_env: string;
  api_type?:
    | "openai_compatible"
    | "azure_openai"
    | "openai_responses"
    | "azure_openai_responses";
  azure_deployment?: string | null;
  azure_api_version?: string | null;
  temperature?: number | null;
  max_output_tokens?: number | null;
  reasoning_effort?:
    | "none"
    | "minimal"
    | "low"
    | "medium"
    | "high"
    | "xhigh"
    | null;
  schema_repair_attempts?: number | null;
}

export interface GenerationRequest {
  endpoint: CanonicalEndpoint;
  intent: MockExampleKind;
  provider: ProviderConfigDraft;
  collection_id?: string;
  persist?: boolean;
  database_url?: string;
  api_key_override?: string;
  generation_context?: GenerationContext | null;
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
  createdAt?: string;
  updatedAt?: string;
  endpointCount?: number;
}
