//! Per-endpoint mock route definition.

use albert_core::{
    CanonicalApiCollection, CanonicalEndpoint, HttpMethod, MockExample, MockExampleKind,
    SchemaNode, default_mock_examples,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A snapshot of one mock endpoint that the HTTP runtime can serve.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MockRoute {
    pub collection_id: String,
    pub collection_name: String,
    pub method: HttpMethod,
    pub path: String,
    pub operation_id: Option<String>,
    pub summary: Option<String>,
    pub examples: Vec<MockExample>,
    /// Request body schema lifted from the canonical endpoint. Used by
    /// the opt-in validator when `enforce_request_bodies` is set.
    #[serde(default)]
    pub request_body_schema: Option<SchemaNode>,
}

impl MockRoute {
    pub fn from_endpoint(
        collection: &CanonicalApiCollection,
        endpoint: &CanonicalEndpoint,
    ) -> Self {
        let examples = if endpoint.examples.is_empty() {
            default_mock_examples()
        } else {
            endpoint.examples.clone()
        };
        Self {
            collection_id: collection.id.clone(),
            collection_name: collection.name.clone(),
            method: endpoint.method.clone(),
            path: endpoint.path.clone(),
            operation_id: endpoint.operation_id.clone(),
            summary: endpoint.summary.clone(),
            examples,
            request_body_schema: endpoint.request_body.as_ref().map(|b| b.schema.clone()),
        }
    }

    pub fn example(&self, kind: &MockExampleKind) -> Option<&MockExample> {
        self.examples
            .iter()
            .find(|candidate| &candidate.kind == kind)
    }

    pub fn preferred_example(
        &self,
        override_kind: Option<&MockExampleKind>,
    ) -> Option<&MockExample> {
        if let Some(kind) = override_kind
            && let Some(found) = self.example(kind)
        {
            return Some(found);
        }
        self.example(&MockExampleKind::Success)
            .or_else(|| self.examples.first())
    }
}

pub struct MatchedRoute<'a> {
    pub route: &'a MockRoute,
    pub params: BTreeMap<String, String>,
}

pub fn route_key(method: &HttpMethod, path: &str) -> String {
    format!("{} {}", method.as_str(), path)
}

pub fn build_routes(collections: &[CanonicalApiCollection]) -> Vec<MockRoute> {
    let mut routes = Vec::new();
    for collection in collections {
        for endpoint in &collection.endpoints {
            routes.push(MockRoute::from_endpoint(collection, endpoint));
        }
    }
    routes
}
