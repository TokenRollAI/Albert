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
  base_url TEXT NOT NULL,
  model TEXT NOT NULL,
  api_key_env TEXT NOT NULL
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

