# graph-analysis-core

Deterministic graph and tree analysis primitives for `moritzbrantner-video-analysis`.

## Feature flags

- No optional feature flags today.

## Example

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
use graph_analysis_core::{
    find_cycle, minimum_spanning_tree, shortest_path, strongly_connected_components, Graph,
};

let mut graph = Graph::undirected();
graph.connect_weighted("a", "b", 1.0)?;
graph.connect_weighted("b", "c", 2.0)?;
graph.connect_weighted("a", "c", 4.0)?;

let path = shortest_path(&graph, "a", "c")?.unwrap();
assert_eq!(path.nodes, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
assert_eq!(minimum_spanning_tree(&graph)?.total_weight, 3.0);
assert!(find_cycle(&graph).is_some());

let mut directed = Graph::directed();
directed.connect("x", "y")?;
directed.connect("y", "x")?;
assert_eq!(strongly_connected_components(&directed).len(), 1);
# Ok(())
# }
```

## Package surface

Primary workflow: `graph.components`.

Workflow operations:

- `graph.components`: Returns connected, weakly connected, or strongly connected graph components.
- `graph.shortestPath`: Returns the shortest weighted path between two graph nodes when reachable.

Debug operations:

- `describe`: inspect package metadata and runtime support.
- `graph.validateTree`: Analyzes an undirected graph as a tree or forest.

Runtime support: library, CLI, server, and WASM wrappers expose these operations.

Run the primary workflow through the CLI:

```bash
cargo run -p moritzbrantner-graph-analysis-core-cli -- run \
  --operation graph.components \
  --json '{"edges":[{"source":"a","target":"b"},{"source":"c","target":"d"}],"kind":"undirected","mode":"connected"}'
```

Successful responses use the shared package-surface shape with `operation`,
`title`, `message`, `summary`, and `result`. Default surface calls are
deterministic, local-first, and do not download models, write persistent files,
or execute external tools unless an operation explicitly documents native or
external-tool execution.

## Related crates

- `dense-data`
- `numbers-core`
- `vector-analysis-core`
