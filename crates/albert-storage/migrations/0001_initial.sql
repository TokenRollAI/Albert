CREATE TABLE IF NOT EXISTS projects (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS api_collections (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  source_kind TEXT NOT NULL,
  name TEXT NOT NULL,
  raw_snapshot TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(project_id) REFERENCES projects(id)
);

CREATE TABLE IF NOT EXISTS api_endpoints (
  id TEXT PRIMARY KEY,
  collection_id TEXT NOT NULL,
  method TEXT NOT NULL,
  path TEXT NOT NULL,
  summary TEXT,
  FOREIGN KEY(collection_id) REFERENCES api_collections(id)
);

CREATE TABLE IF NOT EXISTS api_schemas (
  id TEXT PRIMARY KEY,
  endpoint_id TEXT NOT NULL,
  schema_role TEXT NOT NULL,
  payload TEXT NOT NULL,
  FOREIGN KEY(endpoint_id) REFERENCES api_endpoints(id)
);

CREATE TABLE IF NOT EXISTS mock_examples (
  id TEXT PRIMARY KEY,
  endpoint_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  payload TEXT NOT NULL,
  FOREIGN KEY(endpoint_id) REFERENCES api_endpoints(id)
);

CREATE TABLE IF NOT EXISTS provider_configs (
  id TEXT PRIMARY KEY,
  provider_name TEXT NOT NULL,
  environment TEXT,
  base_url TEXT NOT NULL,
  model TEXT NOT NULL,
  api_key_env TEXT NOT NULL,
  api_type TEXT NOT NULL DEFAULT 'openai_compatible',
  azure_deployment TEXT,
  azure_api_version TEXT,
  temperature REAL,
  max_output_tokens INTEGER,
  reasoning_effort TEXT,
  schema_repair_attempts INTEGER
);

-- Single-row store of gateway runtime preferences. Keyed on a constant
-- "singleton" id so UPSERTs are trivial.
CREATE TABLE IF NOT EXISTS gateway_preferences (
  id TEXT PRIMARY KEY,
  payload TEXT NOT NULL
);

-- Named gateway config snapshots ("scenarios"). Each row stores a full
-- GatewayConfigBundle as JSON plus the collection ids it depends on so
-- users can activate / archive named presets ("healthy", "rate limited",
-- "service down") with one click. Uniqueness is on the human-facing
-- name so the UI can dedupe by label.
CREATE TABLE IF NOT EXISTS gateway_scenarios (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  payload TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- Last-response cache keyed by a normalized request fingerprint. This is the
-- first Phase 5 slice: Try-it can remember the most recent response for a
-- concrete method/path/query/body/header-shape without changing mock serving
-- behavior.
CREATE TABLE IF NOT EXISTS request_fingerprint_cache (
  id TEXT PRIMARY KEY,
  collection_id TEXT NOT NULL,
  method TEXT NOT NULL,
  path TEXT NOT NULL,
  fingerprint TEXT NOT NULL,
  request_snapshot TEXT NOT NULL,
  response_snapshot TEXT NOT NULL,
  hit_count INTEGER NOT NULL DEFAULT 1,
  first_seen_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL,
  UNIQUE(collection_id, method, path, fingerprint)
);
