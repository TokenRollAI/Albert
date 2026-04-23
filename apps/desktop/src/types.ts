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
  }>;
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
