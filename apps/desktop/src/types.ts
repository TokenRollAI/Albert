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

