//! Route table and path pattern matching for the mock gateway.

use std::collections::BTreeMap;

use albert_core::HttpMethod;

use crate::route::{MatchedRoute, MockRoute};

/// A compiled endpoint path, e.g. `/users/{id}/posts/{postId}` →
/// segments `["users", "{id}", "posts", "{postId}"]` with specificity derived
/// from how many segments are literal vs parameter.
#[derive(Debug, Clone)]
pub struct CompiledRoute {
    pub route: MockRoute,
    pub segments: Vec<PathSegment>,
    pub literal_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSegment {
    Literal(String),
    Param(String),
}

impl CompiledRoute {
    pub fn new(route: MockRoute) -> Self {
        let segments = compile_path(&route.path);
        let literal_count = segments
            .iter()
            .filter(|s| matches!(s, PathSegment::Literal(_)))
            .count();
        Self {
            route,
            segments,
            literal_count,
        }
    }
}

fn compile_path(path: &str) -> Vec<PathSegment> {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            if let Some(stripped) = segment
                .strip_prefix('{')
                .and_then(|rest| rest.strip_suffix('}'))
            {
                PathSegment::Param(stripped.to_string())
            } else if let Some(stripped) = segment.strip_prefix(':') {
                PathSegment::Param(stripped.to_string())
            } else {
                PathSegment::Literal(segment.to_string())
            }
        })
        .collect()
}

/// A read-only route table that matches incoming requests against compiled
/// route templates, preferring literal matches over parameter wildcards.
#[derive(Debug, Clone, Default)]
pub struct RouteTable {
    routes: Vec<CompiledRoute>,
}

impl RouteTable {
    pub fn from_routes(routes: Vec<MockRoute>) -> Self {
        let mut compiled: Vec<CompiledRoute> = routes.into_iter().map(CompiledRoute::new).collect();
        // Higher literal count and longer paths win earlier; same tie-break keeps
        // more specific routes in front when iterating.
        compiled.sort_by(|a, b| {
            b.literal_count
                .cmp(&a.literal_count)
                .then(b.segments.len().cmp(&a.segments.len()))
                .then(a.route.path.cmp(&b.route.path))
        });
        Self { routes: compiled }
    }

    pub fn len(&self) -> usize {
        self.routes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    pub fn match_route<'a>(&'a self, method: &HttpMethod, path: &str) -> Option<MatchedRoute<'a>> {
        let request_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        for candidate in &self.routes {
            if &candidate.route.method != method {
                continue;
            }
            if candidate.segments.len() != request_segments.len() {
                continue;
            }
            let mut params = BTreeMap::new();
            let mut ok = true;
            for (template, actual) in candidate.segments.iter().zip(request_segments.iter()) {
                match template {
                    PathSegment::Literal(literal) => {
                        if literal != *actual {
                            ok = false;
                            break;
                        }
                    }
                    PathSegment::Param(name) => {
                        params.insert(name.clone(), (*actual).to_string());
                    }
                }
            }
            if ok {
                return Some(MatchedRoute {
                    route: &candidate.route,
                    params,
                });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use albert_core::{MockExample, MockExampleKind};

    fn make_route(method: HttpMethod, path: &str) -> MockRoute {
        MockRoute {
            collection_id: "c".into(),
            collection_name: "c".into(),
            method,
            path: path.to_string(),
            operation_id: None,
            summary: None,
            examples: vec![MockExample {
                kind: MockExampleKind::Success,
                title: "t".into(),
                payload: serde_json::json!({}),
                note: None,
            }],
        }
    }

    #[test]
    fn matches_literal_and_param_paths() {
        let table = RouteTable::from_routes(vec![
            make_route(HttpMethod::Get, "/users"),
            make_route(HttpMethod::Get, "/users/{id}"),
            make_route(HttpMethod::Get, "/users/me"),
        ]);
        assert_eq!(table.len(), 3);

        let m = table.match_route(&HttpMethod::Get, "/users/me").unwrap();
        assert_eq!(m.route.path, "/users/me");
        assert!(m.params.is_empty());

        let m = table.match_route(&HttpMethod::Get, "/users/42").unwrap();
        assert_eq!(m.route.path, "/users/{id}");
        assert_eq!(m.params.get("id").unwrap(), "42");

        let m = table.match_route(&HttpMethod::Get, "/users").unwrap();
        assert_eq!(m.route.path, "/users");

        assert!(table.match_route(&HttpMethod::Post, "/users").is_none());
        assert!(
            table
                .match_route(&HttpMethod::Get, "/users/42/posts")
                .is_none()
        );
    }

    #[test]
    fn matches_nested_parameters() {
        let table = RouteTable::from_routes(vec![make_route(
            HttpMethod::Get,
            "/users/{userId}/posts/{postId}",
        )]);
        let m = table
            .match_route(&HttpMethod::Get, "/users/1/posts/9")
            .unwrap();
        assert_eq!(m.params.get("userId").unwrap(), "1");
        assert_eq!(m.params.get("postId").unwrap(), "9");
    }
}
