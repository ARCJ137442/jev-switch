//! Canonical graph-domain model for the next DAG engine phase.
//!
//! The current daemon still stores and serves [`crate::router::RouteEdge`] for
//! compatibility. This module gives new callers a typed node view without
//! changing the existing Router or HTTP contracts yet.

use crate::router::{check_acyclic, RouteEdge};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// User-facing role of a graph node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum GraphNodeKind {
    /// A node that may be selected by an external model request.
    Public,
    /// A reusable node that is only reachable from another graph node.
    Internal,
    /// A terminal node backed by a provider configuration and adapter.
    Provider,
}

/// Stable graph metadata. Provider credentials remain in the daemon's
/// ProviderConfig and are intentionally absent here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct GraphNode {
    pub id: String,
    pub kind: GraphNodeKind,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub public_models: Vec<String>,
}

/// A version-independent graph document used at the core/domain boundary.
/// Strategy configuration remains owned by the daemon layer until its policy
/// inheritance rules are frozen; route edge behavior is preserved verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "ts-rs",
    derive(::ts_rs::TS),
    ts(export, export_to = "../../../../ui/src/generated/")
)]
pub struct GraphDocument {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<RouteEdge>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GraphValidationError {
    #[error("graph node id must not be empty")]
    EmptyNodeId,
    #[error("graph node id is duplicated: {0}")]
    DuplicateNode(String),
    #[error("provider and public node use the same id: {0}")]
    ProviderPublicCollision(String),
    #[error("provider node '{0}' must reference its provider id")]
    ProviderReferenceMismatch(String),
    #[error("non-provider node '{0}' must not carry a provider reference")]
    UnexpectedProviderReference(String),
    #[error("public node '{0}' must expose at least one public model")]
    PublicModelMissing(String),
    #[error("edge field left/right must not be empty")]
    EmptyEdgeEndpoint,
    #[error("edge right references unknown node '{0}'")]
    UnknownRightNode(String),
    #[error("provider node '{0}' cannot have outgoing edges")]
    ProviderHasOutgoingEdges(String),
    #[error("graph contains a cycle: {0}")]
    Cycle(String),
}

impl GraphDocument {
    /// Derive typed nodes from the current compatible edge document.
    ///
    /// `public_ids` comes from `service_endpoints`; `provider_ids` comes from
    /// the provider configuration map. Any remaining route identifier becomes
    /// an internal node. This is deliberately read-only migration logic.
    pub fn from_route_edges<I, J>(
        edges: Vec<RouteEdge>,
        provider_ids: I,
        public_ids: J,
    ) -> Result<Self, GraphValidationError>
    where
        I: IntoIterator,
        I::Item: Into<String>,
        J: IntoIterator,
        J::Item: Into<String>,
    {
        let providers: BTreeSet<String> = provider_ids.into_iter().map(Into::into).collect();
        let public: BTreeSet<String> = public_ids.into_iter().map(Into::into).collect();
        let mut ids = BTreeSet::new();
        for edge in &edges {
            if edge.left.trim().is_empty() || edge.right.trim().is_empty() {
                return Err(GraphValidationError::EmptyEdgeEndpoint);
            }
            ids.insert(edge.left.clone());
            ids.insert(edge.right.clone());
        }
        for id in providers.intersection(&public) {
            return Err(GraphValidationError::ProviderPublicCollision(id.clone()));
        }

        let nodes = Self::derive_nodes(&edges, providers.iter().cloned(), public.iter().cloned());
        let document = Self { nodes, edges };
        document.validate()?;
        Ok(document)
    }

    /// Compatibility migration helper. It intentionally does not apply the
    /// stricter target-model validation, so an existing route snapshot can be
    /// surfaced for diagnostics before the user opts into graph validation.
    pub fn derive_nodes<I, J>(
        edges: &[RouteEdge],
        provider_ids: I,
        public_ids: J,
    ) -> Vec<GraphNode>
    where
        I: IntoIterator,
        I::Item: Into<String>,
        J: IntoIterator,
        J::Item: Into<String>,
    {
        let providers: BTreeSet<String> = provider_ids.into_iter().map(Into::into).collect();
        let public: BTreeSet<String> = public_ids.into_iter().map(Into::into).collect();
        let mut ids = BTreeSet::new();
        for edge in edges {
            ids.insert(edge.left.as_str());
            ids.insert(edge.right.as_str());
        }
        ids.into_iter()
            .map(|id| {
                let kind = if providers.contains(id) {
                    GraphNodeKind::Provider
                } else if public.contains(id) {
                    GraphNodeKind::Public
                } else {
                    GraphNodeKind::Internal
                };
                GraphNode {
                    provider_id: (kind == GraphNodeKind::Provider).then(|| id.to_string()),
                    public_models: (kind == GraphNodeKind::Public)
                        .then(|| vec![id.to_string()])
                        .unwrap_or_default(),
                    id: id.to_string(),
                    kind,
                    enabled: true,
                }
            })
            .collect()
    }

    pub fn validate(&self) -> Result<(), GraphValidationError> {
        let mut nodes = BTreeMap::new();
        for node in &self.nodes {
            if node.id.trim().is_empty() {
                return Err(GraphValidationError::EmptyNodeId);
            }
            if nodes.insert(node.id.clone(), node).is_some() {
                return Err(GraphValidationError::DuplicateNode(node.id.clone()));
            }
            match node.kind {
                GraphNodeKind::Provider if node.provider_id.as_deref() != Some(node.id.as_str()) => {
                    return Err(GraphValidationError::ProviderReferenceMismatch(node.id.clone()));
                }
                GraphNodeKind::Provider => {}
                _ if node.provider_id.is_some() => {
                    return Err(GraphValidationError::UnexpectedProviderReference(node.id.clone()));
                }
                _ => {}
            }
            if node.kind == GraphNodeKind::Public && node.public_models.is_empty() {
                return Err(GraphValidationError::PublicModelMissing(node.id.clone()));
            }
        }

        for edge in &self.edges {
            if edge.left.trim().is_empty() || edge.right.trim().is_empty() {
                return Err(GraphValidationError::EmptyEdgeEndpoint);
            }
            if !nodes.contains_key(&edge.right) {
                return Err(GraphValidationError::UnknownRightNode(edge.right.clone()));
            }
            if nodes
                .get(&edge.left)
                .is_some_and(|node| node.kind == GraphNodeKind::Provider)
            {
                return Err(GraphValidationError::ProviderHasOutgoingEdges(edge.left.clone()));
            }
        }

        check_acyclic(&self.edges).map_err(|error| {
            GraphValidationError::Cycle(error.to_string())
        })?;
        Ok(())
    }

    pub fn node(&self, id: &str) -> Option<&GraphNode> {
        self.nodes.iter().find(|node| node.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::{MatchMode, OnError, Sticky};

    fn edge(left: &str, right: &str) -> RouteEdge {
        RouteEdge {
            left: left.into(),
            r#match: MatchMode::Exact,
            right: right.into(),
            upstream_model: None,
            priority: 0,
            sticky: Sticky::None,
            on_error: OnError::Next,
        }
    }

    #[test]
    fn derives_public_internal_and_provider_nodes_without_copying_credentials() {
        let graph = GraphDocument::from_route_edges(
            vec![edge("jev", "fallback"), edge("fallback", "typesafe")],
            ["typesafe"],
            ["jev"],
        )
        .unwrap();

        assert_eq!(graph.node("jev").unwrap().kind, GraphNodeKind::Public);
        assert_eq!(graph.node("fallback").unwrap().kind, GraphNodeKind::Internal);
        let provider = graph.node("typesafe").unwrap();
        assert_eq!(provider.kind, GraphNodeKind::Provider);
        assert_eq!(provider.provider_id.as_deref(), Some("typesafe"));
        assert!(provider.public_models.is_empty());
    }

    #[test]
    fn rejects_provider_public_collision_and_provider_outgoing_edges() {
        let collision = GraphDocument::from_route_edges(
            vec![edge("jev", "typesafe")],
            ["typesafe"],
            ["typesafe"],
        )
        .unwrap_err();
        assert_eq!(
            collision,
            GraphValidationError::ProviderPublicCollision("typesafe".into())
        );

        let graph = GraphDocument {
            nodes: vec![
                GraphNode {
                    id: "typesafe".into(),
                    kind: GraphNodeKind::Provider,
                    enabled: true,
                    provider_id: Some("typesafe".into()),
                    public_models: Vec::new(),
                },
                GraphNode {
                    id: "other".into(),
                    kind: GraphNodeKind::Provider,
                    enabled: true,
                    provider_id: Some("other".into()),
                    public_models: Vec::new(),
                },
            ],
            edges: vec![edge("typesafe", "other")],
        };
        assert_eq!(
            graph.validate().unwrap_err(),
            GraphValidationError::ProviderHasOutgoingEdges("typesafe".into())
        );
    }

    #[test]
    fn rejects_unknown_right_nodes_and_cycles() {
        let unknown = GraphDocument {
            nodes: vec![GraphNode {
                id: "jev".into(),
                kind: GraphNodeKind::Public,
                enabled: true,
                provider_id: None,
                public_models: vec!["jev".into()],
            }],
            edges: vec![edge("jev", "missing")],
        };
        assert_eq!(
            unknown.validate().unwrap_err(),
            GraphValidationError::UnknownRightNode("missing".into())
        );

        let cycle = GraphDocument {
            nodes: vec![
                GraphNode {
                    id: "a".into(),
                    kind: GraphNodeKind::Internal,
                    enabled: true,
                    provider_id: None,
                    public_models: Vec::new(),
                },
                GraphNode {
                    id: "b".into(),
                    kind: GraphNodeKind::Internal,
                    enabled: true,
                    provider_id: None,
                    public_models: Vec::new(),
                },
            ],
            edges: vec![edge("a", "b"), edge("b", "a")],
        };
        assert!(matches!(
            cycle.validate(),
            Err(GraphValidationError::Cycle(_))
        ));
    }
}
