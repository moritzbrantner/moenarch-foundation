//! Library-owned runtime surface for `graph-analysis-core`.

use runtime_core::{
    OperationId, PackageSurface, RuntimeCapabilities, SurfaceOperation, SurfaceRequest,
    SurfaceResponse,
};
use serde::Deserialize;

use crate::{
    analyze_tree, connected_components, shortest_path, strongly_connected_components,
    weakly_connected_components, Graph, GraphEdge, GraphKind,
};

/// Returns the package surface exposed by every transport wrapper.
pub fn package_surface() -> PackageSurface {
    PackageSurface {
        library: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: RuntimeCapabilities::pure_rust(),
        operations: vec![
            operation(
                "describe",
                "Describe package",
                "Graph and tree analysis primitives for video-analysis.",
                serde_json::json!({"includeOperations": true}),
            ),
            operation(
                "graph.components",
                "Graph components",
                "Returns connected, weakly connected, or strongly connected graph components.",
                serde_json::json!({
                    "kind": "undirected",
                    "edges": [{"source": "a", "target": "b"}, {"source": "c", "target": "d"}],
                    "mode": "connected"
                }),
            ),
            operation(
                "graph.shortestPath",
                "Shortest path",
                "Returns the shortest weighted path between two graph nodes when reachable.",
                serde_json::json!({
                    "kind": "directed",
                    "edges": [{"source": "a", "target": "b", "weight": 2.0}],
                    "source": "a",
                    "target": "b"
                }),
            ),
            operation(
                "graph.validateTree",
                "Validate tree",
                "Analyzes an undirected graph as a tree or forest.",
                serde_json::json!({
                    "kind": "undirected",
                    "edges": [{"source": "a", "target": "b"}, {"source": "b", "target": "c"}],
                    "root": "a"
                }),
            ),
        ],
    }
}

fn operation(
    id: &str,
    name: &str,
    description: &str,
    example_request: serde_json::Value,
) -> SurfaceOperation {
    SurfaceOperation {
        id: OperationId::new(id),
        name: name.to_string(),
        description: Some(description.to_string()),
        curation: runtime_core::SurfaceOperationCuration::from_operation_id(id),
        input_schema: serde_json::json!({"type": "object", "additionalProperties": true, "xOperationCategory": runtime_core::operation_category(id)}),
        output_schema: serde_json::json!({"type": "object", "xOperationCategory": runtime_core::operation_category(id)}),
        example_request,
        wasm_supported: true,
        server_supported: true,
    }
}

/// Runs one library-owned operation.
pub fn run_surface_operation(request: SurfaceRequest) -> Result<SurfaceResponse, String> {
    let operation = request.operation.clone();
    let value = match request.operation.as_str() {
        "describe" => describe_value(request.input),
        "graph.components" => components_value(parse_input(request.input)?)?,
        "graph.shortestPath" => shortest_path_value(parse_input(request.input)?)?,
        "graph.validateTree" => validate_tree_value(parse_input(request.input)?)?,
        operation => {
            return Err(format!(
                "unsupported operation `{operation}` for {}",
                env!("CARGO_PKG_NAME")
            ));
        }
    };
    Ok(response(operation, value))
}

fn describe_value(input: serde_json::Value) -> serde_json::Value {
    let surface = package_surface();
    serde_json::json!({
        "library": surface.library,
        "version": surface.version,
        "operationCount": surface.operations.len(),
        "operations": surface
            .operations
            .iter()
            .map(|operation| operation.id.as_str())
            .collect::<Vec<_>>(),
        "input": input
    })
}

fn response(operation: OperationId, value: serde_json::Value) -> SurfaceResponse {
    SurfaceResponse {
        operation,
        value,
        diagnostics: Vec::new(),
        artifacts: Vec::new(),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphRequest {
    kind: String,
    #[serde(default)]
    nodes: Vec<String>,
    edges: Vec<EdgeRequest>,
    #[serde(default)]
    mode: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ShortestPathRequest {
    kind: String,
    #[serde(default)]
    nodes: Vec<String>,
    edges: Vec<EdgeRequest>,
    source: String,
    target: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TreeRequest {
    kind: String,
    #[serde(default)]
    nodes: Vec<String>,
    edges: Vec<EdgeRequest>,
    #[serde(default)]
    root: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EdgeRequest {
    source: String,
    target: String,
    #[serde(default)]
    weight: Option<f64>,
}

fn components_value(request: GraphRequest) -> Result<serde_json::Value, String> {
    let kind = parse_kind(&request.kind)?;
    let mode = request
        .mode
        .unwrap_or_else(|| default_component_mode(kind).to_string());
    let graph = graph_from_parts(kind, request.nodes, request.edges)?;
    let components = match mode.as_str() {
        "connected" => connected_components(&graph).map_err(|error| error.to_string())?,
        "weak" => weakly_connected_components(&graph),
        "strong" => strongly_connected_components(&graph),
        _ => return Err(format!("unsupported graph component mode `{mode}`")),
    };
    Ok(serde_json::json!({
        "kind": kind_name(kind),
        "mode": mode,
        "componentCount": components.len(),
        "components": components.into_iter().map(|component| serde_json::json!({
            "nodes": component.nodes,
            "edgeCount": component.edge_count,
            "totalWeight": component.total_weight
        })).collect::<Vec<_>>()
    }))
}

fn shortest_path_value(request: ShortestPathRequest) -> Result<serde_json::Value, String> {
    let kind = parse_kind(&request.kind)?;
    let graph = graph_from_parts(kind, request.nodes, request.edges)?;
    let path = shortest_path(&graph, &request.source, &request.target)
        .map_err(|error| error.to_string())?;
    let mut value = serde_json::json!({
        "source": request.source,
        "target": request.target,
        "reachable": path.is_some()
    });
    if let Some(path) = path {
        value["path"] = serde_json::json!(path.nodes);
        value["totalWeight"] = serde_json::json!(path.total_weight);
    }
    Ok(value)
}

fn validate_tree_value(request: TreeRequest) -> Result<serde_json::Value, String> {
    let kind = parse_kind(&request.kind)?;
    let graph = graph_from_parts(kind, request.nodes, request.edges)?;
    let analysis =
        analyze_tree(&graph, request.root.as_deref()).map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "isTree": analysis.is_tree,
        "isForest": analysis.is_forest,
        "hasCycle": analysis.has_cycle,
        "componentCount": analysis.component_count,
        "roots": analysis.roots,
        "leaves": analysis.leaves,
        "depths": analysis.depths,
        "parents": analysis.parents,
        "traversalOrder": analysis.traversal_order
    }))
}

fn graph_from_parts(
    kind: GraphKind,
    nodes: Vec<String>,
    edges: Vec<EdgeRequest>,
) -> Result<Graph, String> {
    let mut graph = Graph::new(kind);
    for node in nodes {
        graph.add_node(node).map_err(|error| error.to_string())?;
    }
    for edge in edges {
        graph
            .add_edge(edge_from_request(edge)?)
            .map_err(|error| error.to_string())?;
    }
    Ok(graph)
}

fn edge_from_request(request: EdgeRequest) -> Result<GraphEdge, String> {
    match request.weight {
        Some(weight) => GraphEdge::weighted(request.source, request.target, weight),
        None => GraphEdge::new(request.source, request.target),
    }
    .map_err(|error| error.to_string())
}

fn parse_kind(kind: &str) -> Result<GraphKind, String> {
    match kind {
        "directed" => Ok(GraphKind::Directed),
        "undirected" => Ok(GraphKind::Undirected),
        _ => Err(format!("unsupported graph kind `{kind}`")),
    }
}

fn kind_name(kind: GraphKind) -> &'static str {
    match kind {
        GraphKind::Directed => "directed",
        GraphKind::Undirected => "undirected",
    }
}

fn default_component_mode(kind: GraphKind) -> &'static str {
    match kind {
        GraphKind::Directed => "weak",
        GraphKind::Undirected => "connected",
    }
}

fn parse_input<T: for<'de> Deserialize<'de>>(input: serde_json::Value) -> Result<T, String> {
    serde_json::from_value(input).map_err(|error| format!("invalid request: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_surface_lists_graph_operations() {
        let ids = package_surface()
            .operations
            .into_iter()
            .map(|operation| operation.id.0)
            .collect::<Vec<_>>();

        assert!(ids.contains(&"graph.components".to_string()));
        assert!(ids.contains(&"graph.shortestPath".to_string()));
        assert!(ids.contains(&"graph.validateTree".to_string()));
    }

    #[test]
    fn undirected_components_report_two_components() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("graph.components"),
            input: serde_json::json!({
                "kind": "undirected",
                "edges": [
                    {"source": "a", "target": "b"},
                    {"source": "c", "target": "d"}
                ]
            }),
        })
        .expect("components operation");

        assert_eq!(response.value["mode"], "connected");
        assert_eq!(response.value["componentCount"], 2);
    }

    #[test]
    fn shortest_path_returns_expected_path_and_weight() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("graph.shortestPath"),
            input: serde_json::json!({
                "kind": "directed",
                "edges": [
                    {"source": "a", "target": "b", "weight": 2.0},
                    {"source": "b", "target": "c", "weight": 3.0},
                    {"source": "a", "target": "c", "weight": 10.0}
                ],
                "source": "a",
                "target": "c"
            }),
        })
        .expect("shortest path operation");

        assert_eq!(response.value["reachable"], true);
        assert_eq!(response.value["path"], serde_json::json!(["a", "b", "c"]));
        assert_eq!(response.value["totalWeight"], 5.0);
    }

    #[test]
    fn tree_validation_reports_chain_as_tree() {
        let response = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("graph.validateTree"),
            input: serde_json::json!({
                "kind": "undirected",
                "edges": [
                    {"source": "a", "target": "b"},
                    {"source": "b", "target": "c"}
                ],
                "root": "a"
            }),
        })
        .expect("tree validation operation");

        assert_eq!(response.value["isTree"], true);
        assert_eq!(response.value["isForest"], true);
        assert_eq!(response.value["depths"]["c"], 2);
    }

    #[test]
    fn negative_edge_shortest_path_fails_validation() {
        let error = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("graph.shortestPath"),
            input: serde_json::json!({
                "kind": "directed",
                "edges": [{"source": "a", "target": "b", "weight": -1.0}],
                "source": "a",
                "target": "b"
            }),
        })
        .expect_err("negative edge");

        assert!(error.contains("non-negative edge weights"));
    }

    #[test]
    fn invalid_request_parsing_is_reported() {
        let error = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("graph.components"),
            input: serde_json::json!(false),
        })
        .expect_err("invalid request");

        assert!(error.contains("invalid request"));
    }

    #[test]
    fn unsupported_operation_is_reported() {
        let error = run_surface_operation(SurfaceRequest {
            operation: OperationId::new("graph.missing"),
            input: serde_json::json!({}),
        })
        .expect_err("unsupported operation");

        assert!(error.contains("unsupported operation `graph.missing`"));
    }
}
