import type {
  AuthRequirementHint,
  CanonicalEndpoint,
  RequiredHeader
} from "../types";

/**
 * Convert a single canonical endpoint's auth hint into the
 * `RequiredHeader[]` shape the gateway's `required_headers` map expects.
 * Returns an empty array when the hint is absent or describes a scheme
 * that can't be enforced by a header check (e.g. API key in query/cookie,
 * OpenID Connect discovery flows).
 */
export function authHintToRequiredHeaders(
  hint: AuthRequirementHint | null | undefined
): RequiredHeader[] {
  if (!hint) return [];
  if (
    hint.scheme === "http_bearer" ||
    hint.scheme === "http_basic" ||
    hint.scheme === "oauth2"
  ) {
    return [
      {
        name: hint.header_name,
        value_prefix: hint.value_prefix ?? null,
        value_equals: null
      }
    ];
  }
  if (hint.scheme === "api_key_header") {
    return [
      {
        name: hint.header_name,
        value_prefix: null,
        value_equals: null
      }
    ];
  }
  // "other" hints are surfaced as descriptive notes but never seeded.
  return [];
}

/**
 * Build the full `required_headers` map for a list of endpoints, keyed by
 * `METHOD /path`. Skips endpoints without an auth hint. Caller decides
 * whether to merge this with an existing user-defined map.
 */
export function seedRequiredHeadersFromEndpoints(
  endpoints: CanonicalEndpoint[]
): Record<string, RequiredHeader[]> {
  const out: Record<string, RequiredHeader[]> = {};
  for (const endpoint of endpoints) {
    const rules = authHintToRequiredHeaders(endpoint.auth);
    if (rules.length === 0) continue;
    const key = `${endpoint.method.toUpperCase()} ${endpoint.path}`;
    out[key] = rules;
  }
  return out;
}
