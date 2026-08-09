#![doc = include_str!("../README.md")]

pub mod surface;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use media_core::{DetectError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Variants describing graph kind.
pub enum GraphKind {
    /// The directed variant.
    Directed,
    /// The undirected variant.
    Undirected,
}

#[derive(Debug, Clone, PartialEq)]
/// Data type for graph edge.
pub struct GraphEdge {
    /// The source value.
    pub source: String,
    /// The target value.
    pub target: String,
    /// The weight value.
    pub weight: f64,
}

impl GraphEdge {
    /// Creates a new value.
    pub fn new(source: impl Into<String>, target: impl Into<String>) -> Result<Self> {
        Self::weighted(source, target, 1.0)
    }

    /// Returns weighted.
    pub fn weighted(
        source: impl Into<String>,
        target: impl Into<String>,
        weight: f64,
    ) -> Result<Self> {
        let edge = Self {
            source: source.into(),
            target: target.into(),
            weight,
        };
        edge.validate()?;
        Ok(edge)
    }

    /// Validates this value.
    pub fn validate(&self) -> Result<()> {
        if self.source.is_empty() {
            return Err(invalid_argument("edge source must not be empty"));
        }
        if self.target.is_empty() {
            return Err(invalid_argument("edge target must not be empty"));
        }
        if !self.weight.is_finite() {
            return Err(invalid_argument("edge weight must be finite"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Data type for graph.
pub struct Graph {
    kind: GraphKind,
    nodes: BTreeSet<String>,
    edges: Vec<GraphEdge>,
}

impl Graph {
    /// Creates a new value.
    pub fn new(kind: GraphKind) -> Self {
        Self {
            kind,
            nodes: BTreeSet::new(),
            edges: Vec::new(),
        }
    }

    /// Returns directed.
    pub fn directed() -> Self {
        Self::new(GraphKind::Directed)
    }

    /// Returns undirected.
    pub fn undirected() -> Self {
        Self::new(GraphKind::Undirected)
    }

    /// Builds this value from edges.
    pub fn from_edges(kind: GraphKind, edges: impl IntoIterator<Item = GraphEdge>) -> Result<Self> {
        let mut graph = Self::new(kind);
        for edge in edges {
            graph.add_edge(edge)?;
        }
        Ok(graph)
    }

    /// Returns kind.
    pub fn kind(&self) -> GraphKind {
        self.kind
    }

    /// Adds add node to this value.
    pub fn add_node(&mut self, node_id: impl Into<String>) -> Result<()> {
        let node_id = node_id.into();
        if node_id.is_empty() {
            return Err(invalid_argument("node id must not be empty"));
        }
        self.nodes.insert(node_id);
        Ok(())
    }

    /// Adds add edge to this value.
    pub fn add_edge(&mut self, edge: GraphEdge) -> Result<()> {
        let edge = normalize_edge(self.kind, edge)?;
        self.nodes.insert(edge.source.clone());
        self.nodes.insert(edge.target.clone());
        self.edges.push(edge);
        Ok(())
    }

    /// Returns connect.
    pub fn connect(&mut self, source: impl Into<String>, target: impl Into<String>) -> Result<()> {
        self.add_edge(GraphEdge::new(source, target)?)
    }

    /// Returns connect weighted.
    pub fn connect_weighted(
        &mut self,
        source: impl Into<String>,
        target: impl Into<String>,
        weight: f64,
    ) -> Result<()> {
        self.add_edge(GraphEdge::weighted(source, target, weight)?)
    }

    /// Returns contains node.
    pub fn contains_node(&self, node_id: &str) -> bool {
        self.nodes.contains(node_id)
    }

    /// Returns node count.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns edge count.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Returns whether is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns node identifiers.
    pub fn node_ids(&self) -> impl Iterator<Item = &str> {
        self.nodes.iter().map(String::as_str)
    }

    /// Returns edges.
    pub fn edges(&self) -> &[GraphEdge] {
        &self.edges
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Data type for graph component.
pub struct GraphComponent {
    /// The nodes value.
    pub nodes: Vec<String>,
    /// The edge count value.
    pub edge_count: usize,
    /// The total weight value.
    pub total_weight: f64,
}

#[derive(Debug, Clone, PartialEq)]
/// Data type for graph cycle.
pub struct GraphCycle {
    /// The nodes value.
    pub nodes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
/// Data type for shortest path.
pub struct ShortestPath {
    /// The nodes value.
    pub nodes: Vec<String>,
    /// The total weight value.
    pub total_weight: f64,
}

#[derive(Debug, Clone, PartialEq)]
/// Data type for shortest path tree.
pub struct ShortestPathTree {
    /// The source value.
    pub source: String,
    /// The distances value.
    pub distances: BTreeMap<String, f64>,
    /// The predecessors value.
    pub predecessors: BTreeMap<String, Option<String>>,
}

impl ShortestPathTree {
    /// Returns path to.
    pub fn path_to(&self, target: &str) -> Option<ShortestPath> {
        let total_weight = *self.distances.get(target)?;
        let mut nodes = Vec::new();
        let mut current = target;
        loop {
            nodes.push(current.to_string());
            let parent = self.predecessors.get(current)?;
            match parent {
                Some(parent) => current = parent,
                None => break,
            }
        }
        nodes.reverse();
        Some(ShortestPath {
            nodes,
            total_weight,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Data type for spanning forest.
pub struct SpanningForest {
    /// The nodes value.
    pub nodes: Vec<String>,
    /// The component count value.
    pub component_count: usize,
    /// The total weight value.
    pub total_weight: f64,
    /// The edges value.
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, PartialEq)]
/// Data type for tree analysis.
pub struct TreeAnalysis {
    /// The roots value.
    pub roots: Vec<String>,
    /// The is tree value.
    pub is_tree: bool,
    /// The is forest value.
    pub is_forest: bool,
    /// The has cycle value.
    pub has_cycle: bool,
    /// The component count value.
    pub component_count: usize,
    /// The leaves value.
    pub leaves: Vec<String>,
    /// The traversal order value.
    pub traversal_order: Vec<String>,
    /// The depths value.
    pub depths: BTreeMap<String, usize>,
    /// The parents value.
    pub parents: BTreeMap<String, Option<String>>,
}

/// Returns connected components.
pub fn connected_components(graph: &Graph) -> Result<Vec<GraphComponent>> {
    if graph.kind != GraphKind::Undirected {
        return Err(invalid_argument(
            "connected components require an undirected graph; use weakly_connected_components or strongly_connected_components for directed graphs",
        ));
    }
    Ok(weakly_connected_components(graph))
}

/// Returns weakly connected components.
pub fn weakly_connected_components(graph: &Graph) -> Vec<GraphComponent> {
    let (nodes, index_by_node) = node_index(graph);
    let adjacency = build_adjacency(graph, &index_by_node, TraversalMode::Undirected);
    component_search(graph, &nodes, &index_by_node, &adjacency)
}

/// Returns strongly connected components.
pub fn strongly_connected_components(graph: &Graph) -> Vec<GraphComponent> {
    let (nodes, index_by_node) = node_index(graph);
    let adjacency = build_adjacency(graph, &index_by_node, TraversalMode::Native);
    let reverse = build_reverse_adjacency(graph, &index_by_node);
    let mut visited = vec![false; nodes.len()];
    let mut order = Vec::with_capacity(nodes.len());

    for index in 0..nodes.len() {
        if !visited[index] {
            finish_order(index, &adjacency, &mut visited, &mut order);
        }
    }

    let mut assigned = vec![false; nodes.len()];
    let mut components = Vec::new();
    while let Some(index) = order.pop() {
        if assigned[index] {
            continue;
        }
        let mut component = Vec::new();
        collect_component(index, &reverse, &mut assigned, &mut component);
        component.sort_unstable();
        components.push(build_component(graph, &nodes, &index_by_node, component));
    }
    components.sort_by(|left, right| left.nodes.cmp(&right.nodes));
    components
}

/// Returns whether is connected.
pub fn is_connected(graph: &Graph) -> Result<bool> {
    Ok(connected_components(graph)?.len() <= 1)
}

/// Returns whether is weakly connected.
pub fn is_weakly_connected(graph: &Graph) -> bool {
    weakly_connected_components(graph).len() <= 1
}

/// Returns whether is strongly connected.
pub fn is_strongly_connected(graph: &Graph) -> bool {
    strongly_connected_components(graph).len() <= 1
}

/// Returns find cycle.
pub fn find_cycle(graph: &Graph) -> Option<GraphCycle> {
    let (nodes, index_by_node) = node_index(graph);
    let adjacency = build_adjacency(graph, &index_by_node, TraversalMode::Native);
    let cycle = match graph.kind {
        GraphKind::Directed => find_directed_cycle(&adjacency),
        GraphKind::Undirected => find_undirected_cycle(&adjacency),
    }?;
    Some(GraphCycle {
        nodes: cycle
            .into_iter()
            .map(|index| nodes[index].clone())
            .collect(),
    })
}

/// Returns whether has cycle.
pub fn has_cycle(graph: &Graph) -> bool {
    find_cycle(graph).is_some()
}

/// Returns shortest paths from.
pub fn shortest_paths_from(graph: &Graph, source: &str) -> Result<ShortestPathTree> {
    ensure_node_exists(graph, source)?;
    if graph.edges.iter().any(|edge| edge.weight < 0.0) {
        return Err(invalid_argument(
            "shortest path requires non-negative edge weights",
        ));
    }

    let (nodes, index_by_node) = node_index(graph);
    let adjacency = build_adjacency(graph, &index_by_node, TraversalMode::Native);
    let source_index = index_by_node[source];
    let mut distances = vec![f64::INFINITY; nodes.len()];
    let mut predecessors = vec![None; nodes.len()];
    let mut visited = vec![false; nodes.len()];
    distances[source_index] = 0.0;

    loop {
        let next = distances
            .iter()
            .enumerate()
            .filter(|(index, _)| !visited[*index])
            .min_by(|left, right| left.1.partial_cmp(right.1).unwrap());
        let Some((node_index, distance)) = next else {
            break;
        };
        if !distance.is_finite() {
            break;
        }
        visited[node_index] = true;
        for &(neighbor, weight) in &adjacency[node_index] {
            let candidate = distances[node_index] + weight;
            if candidate < distances[neighbor] {
                distances[neighbor] = candidate;
                predecessors[neighbor] = Some(node_index);
            }
        }
    }

    let mut reachable_distances = BTreeMap::new();
    let mut reachable_predecessors = BTreeMap::new();
    for (index, node) in nodes.iter().enumerate() {
        if !distances[index].is_finite() {
            continue;
        }
        reachable_distances.insert(node.clone(), distances[index]);
        reachable_predecessors.insert(
            node.clone(),
            predecessors[index].map(|parent| nodes[parent].clone()),
        );
    }

    Ok(ShortestPathTree {
        source: source.to_string(),
        distances: reachable_distances,
        predecessors: reachable_predecessors,
    })
}

/// Returns shortest path.
pub fn shortest_path(graph: &Graph, source: &str, target: &str) -> Result<Option<ShortestPath>> {
    ensure_node_exists(graph, target)?;
    Ok(shortest_paths_from(graph, source)?.path_to(target))
}

/// Returns minimum spanning forest.
pub fn minimum_spanning_forest(graph: &Graph) -> Result<SpanningForest> {
    if graph.kind != GraphKind::Undirected {
        return Err(invalid_argument(
            "minimum spanning forest requires an undirected graph",
        ));
    }

    let (nodes, index_by_node) = node_index(graph);
    let mut edges = graph.edges.clone();
    edges.sort_by(|left, right| {
        left.weight
            .partial_cmp(&right.weight)
            .unwrap()
            .then_with(|| left.source.cmp(&right.source))
            .then_with(|| left.target.cmp(&right.target))
    });

    let mut disjoint = DisjointSet::new(nodes.len());
    let mut selected = Vec::new();
    let mut total_weight = 0.0;
    for edge in edges {
        let source = index_by_node[edge.source.as_str()];
        let target = index_by_node[edge.target.as_str()];
        if disjoint.union(source, target) {
            total_weight += edge.weight;
            selected.push(edge);
        }
    }

    Ok(SpanningForest {
        nodes,
        component_count: disjoint.components(),
        total_weight,
        edges: selected,
    })
}

/// Returns minimum spanning tree.
pub fn minimum_spanning_tree(graph: &Graph) -> Result<SpanningForest> {
    let forest = minimum_spanning_forest(graph)?;
    if !forest.nodes.is_empty() && forest.component_count != 1 {
        return Err(invalid_argument(
            "minimum spanning tree requires a connected graph",
        ));
    }
    Ok(forest)
}

/// Returns analyze tree.
pub fn analyze_tree(graph: &Graph, root: Option<&str>) -> Result<TreeAnalysis> {
    if graph.kind != GraphKind::Undirected {
        return Err(invalid_argument(
            "tree analysis requires an undirected graph",
        ));
    }
    if let Some(root) = root {
        ensure_node_exists(graph, root)?;
    }

    let (nodes, index_by_node) = node_index(graph);
    let adjacency = build_adjacency(graph, &index_by_node, TraversalMode::Native);
    let mut visited = vec![false; nodes.len()];
    let mut roots = Vec::new();
    let mut traversal_order = Vec::new();
    let mut depths = BTreeMap::new();
    let mut parents = BTreeMap::new();

    let preferred_root = root.map(|root| index_by_node[root]);
    let mut root_queue = Vec::new();
    if let Some(root_index) = preferred_root {
        root_queue.push(root_index);
    }
    for index in 0..nodes.len() {
        if Some(index) != preferred_root {
            root_queue.push(index);
        }
    }

    for root_index in root_queue {
        if visited[root_index] {
            continue;
        }
        roots.push(nodes[root_index].clone());
        let mut queue = VecDeque::from([(root_index, 0_usize, None::<usize>)]);
        visited[root_index] = true;
        while let Some((node_index, depth, parent_index)) = queue.pop_front() {
            let node = nodes[node_index].clone();
            traversal_order.push(node.clone());
            depths.insert(node.clone(), depth);
            parents.insert(
                node.clone(),
                parent_index.map(|parent| nodes[parent].clone()),
            );
            for &(neighbor, _) in &adjacency[node_index] {
                if visited[neighbor] {
                    continue;
                }
                visited[neighbor] = true;
                queue.push_back((neighbor, depth + 1, Some(node_index)));
            }
        }
    }

    let component_count = weakly_connected_components(graph).len();
    let has_cycle = has_cycle(graph);
    let leaves = nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| (adjacency[index].len() <= 1).then_some(node.clone()))
        .collect::<Vec<_>>();

    Ok(TreeAnalysis {
        roots,
        is_tree: !nodes.is_empty() && component_count == 1 && !has_cycle,
        is_forest: !has_cycle,
        has_cycle,
        component_count,
        leaves,
        traversal_order,
        depths,
        parents,
    })
}

/// Returns whether is tree.
pub fn is_tree(graph: &Graph) -> Result<bool> {
    Ok(analyze_tree(graph, None)?.is_tree)
}

/// Returns whether is forest.
pub fn is_forest(graph: &Graph) -> Result<bool> {
    Ok(analyze_tree(graph, None)?.is_forest)
}

fn normalize_edge(kind: GraphKind, mut edge: GraphEdge) -> Result<GraphEdge> {
    edge.validate()?;
    if kind == GraphKind::Undirected && edge.source > edge.target {
        std::mem::swap(&mut edge.source, &mut edge.target);
    }
    Ok(edge)
}

fn ensure_node_exists(graph: &Graph, node_id: &str) -> Result<()> {
    if graph.contains_node(node_id) {
        return Ok(());
    }
    Err(invalid_argument(format!("unknown node `{node_id}`")))
}

fn node_index(graph: &Graph) -> (Vec<String>, BTreeMap<String, usize>) {
    let nodes = graph.node_ids().map(str::to_string).collect::<Vec<_>>();
    let index_by_node = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.clone(), index))
        .collect::<BTreeMap<_, _>>();
    (nodes, index_by_node)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TraversalMode {
    Native,
    Undirected,
}

fn build_adjacency(
    graph: &Graph,
    index_by_node: &BTreeMap<String, usize>,
    mode: TraversalMode,
) -> Vec<Vec<(usize, f64)>> {
    let mut adjacency = vec![Vec::new(); index_by_node.len()];
    for edge in &graph.edges {
        let source = index_by_node[edge.source.as_str()];
        let target = index_by_node[edge.target.as_str()];
        adjacency[source].push((target, edge.weight));
        if (graph.kind == GraphKind::Undirected || mode == TraversalMode::Undirected)
            && source != target
        {
            adjacency[target].push((source, edge.weight));
        }
    }
    for neighbors in &mut adjacency {
        neighbors.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.partial_cmp(&right.1).unwrap())
        });
    }
    adjacency
}

fn build_reverse_adjacency(
    graph: &Graph,
    index_by_node: &BTreeMap<String, usize>,
) -> Vec<Vec<(usize, f64)>> {
    if graph.kind == GraphKind::Undirected {
        return build_adjacency(graph, index_by_node, TraversalMode::Native);
    }

    let mut adjacency = vec![Vec::new(); index_by_node.len()];
    for edge in &graph.edges {
        let source = index_by_node[edge.source.as_str()];
        let target = index_by_node[edge.target.as_str()];
        adjacency[target].push((source, edge.weight));
    }
    for neighbors in &mut adjacency {
        neighbors.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.partial_cmp(&right.1).unwrap())
        });
    }
    adjacency
}

fn component_search(
    graph: &Graph,
    nodes: &[String],
    index_by_node: &BTreeMap<String, usize>,
    adjacency: &[Vec<(usize, f64)>],
) -> Vec<GraphComponent> {
    let mut visited = vec![false; nodes.len()];
    let mut components = Vec::new();
    for start in 0..nodes.len() {
        if visited[start] {
            continue;
        }
        let mut stack = vec![start];
        let mut component = Vec::new();
        visited[start] = true;
        while let Some(node) = stack.pop() {
            component.push(node);
            for &(neighbor, _) in adjacency[node].iter().rev() {
                if visited[neighbor] {
                    continue;
                }
                visited[neighbor] = true;
                stack.push(neighbor);
            }
        }
        component.sort_unstable();
        components.push(build_component(graph, nodes, index_by_node, component));
    }
    components.sort_by(|left, right| left.nodes.cmp(&right.nodes));
    components
}

fn build_component(
    graph: &Graph,
    nodes: &[String],
    index_by_node: &BTreeMap<String, usize>,
    component: Vec<usize>,
) -> GraphComponent {
    let node_set = component.iter().copied().collect::<BTreeSet<_>>();
    let component_nodes = component
        .iter()
        .map(|index| nodes[*index].clone())
        .collect::<Vec<_>>();
    let mut edge_count = 0;
    let mut total_weight = 0.0;
    for edge in &graph.edges {
        let source = index_by_node[edge.source.as_str()];
        let target = index_by_node[edge.target.as_str()];
        if node_set.contains(&source) && node_set.contains(&target) {
            edge_count += 1;
            total_weight += edge.weight;
        }
    }
    GraphComponent {
        nodes: component_nodes,
        edge_count,
        total_weight,
    }
}

fn finish_order(
    node: usize,
    adjacency: &[Vec<(usize, f64)>],
    visited: &mut [bool],
    order: &mut Vec<usize>,
) {
    visited[node] = true;
    for &(neighbor, _) in &adjacency[node] {
        if !visited[neighbor] {
            finish_order(neighbor, adjacency, visited, order);
        }
    }
    order.push(node);
}

fn collect_component(
    node: usize,
    adjacency: &[Vec<(usize, f64)>],
    visited: &mut [bool],
    component: &mut Vec<usize>,
) {
    visited[node] = true;
    component.push(node);
    for &(neighbor, _) in &adjacency[node] {
        if !visited[neighbor] {
            collect_component(neighbor, adjacency, visited, component);
        }
    }
}

fn find_directed_cycle(adjacency: &[Vec<(usize, f64)>]) -> Option<Vec<usize>> {
    let mut state = vec![0_u8; adjacency.len()];
    let mut stack = Vec::new();
    let mut positions = vec![None; adjacency.len()];
    for node in 0..adjacency.len() {
        if state[node] == 0 {
            if let Some(cycle) =
                find_directed_cycle_from(node, adjacency, &mut state, &mut stack, &mut positions)
            {
                return Some(cycle);
            }
        }
    }
    None
}

fn find_directed_cycle_from(
    node: usize,
    adjacency: &[Vec<(usize, f64)>],
    state: &mut [u8],
    stack: &mut Vec<usize>,
    positions: &mut [Option<usize>],
) -> Option<Vec<usize>> {
    state[node] = 1;
    positions[node] = Some(stack.len());
    stack.push(node);
    for &(neighbor, _) in &adjacency[node] {
        match state[neighbor] {
            0 => {
                if let Some(cycle) =
                    find_directed_cycle_from(neighbor, adjacency, state, stack, positions)
                {
                    return Some(cycle);
                }
            }
            1 => {
                let start = positions[neighbor].expect("active stack position must exist");
                let mut cycle = stack[start..].to_vec();
                cycle.push(neighbor);
                return Some(cycle);
            }
            _ => {}
        }
    }
    stack.pop();
    positions[node] = None;
    state[node] = 2;
    None
}

fn find_undirected_cycle(adjacency: &[Vec<(usize, f64)>]) -> Option<Vec<usize>> {
    let mut visited = vec![false; adjacency.len()];
    let mut stack = Vec::new();
    let mut positions = vec![None; adjacency.len()];
    for node in 0..adjacency.len() {
        if !visited[node] {
            if let Some(cycle) = find_undirected_cycle_from(
                node,
                None,
                adjacency,
                &mut visited,
                &mut stack,
                &mut positions,
            ) {
                return Some(cycle);
            }
        }
    }
    None
}

fn find_undirected_cycle_from(
    node: usize,
    parent: Option<usize>,
    adjacency: &[Vec<(usize, f64)>],
    visited: &mut [bool],
    stack: &mut Vec<usize>,
    positions: &mut [Option<usize>],
) -> Option<Vec<usize>> {
    visited[node] = true;
    positions[node] = Some(stack.len());
    stack.push(node);
    for &(neighbor, _) in &adjacency[node] {
        if Some(neighbor) == parent {
            continue;
        }
        if !visited[neighbor] {
            if let Some(cycle) = find_undirected_cycle_from(
                neighbor,
                Some(node),
                adjacency,
                visited,
                stack,
                positions,
            ) {
                return Some(cycle);
            }
            continue;
        }
        if let Some(start) = positions[neighbor] {
            let mut cycle = stack[start..].to_vec();
            cycle.push(neighbor);
            return Some(cycle);
        }
    }
    stack.pop();
    positions[node] = None;
    None
}

#[derive(Debug, Clone)]
struct DisjointSet {
    parents: Vec<usize>,
    ranks: Vec<usize>,
    components: usize,
}

impl DisjointSet {
    fn new(size: usize) -> Self {
        Self {
            parents: (0..size).collect(),
            ranks: vec![0; size],
            components: size,
        }
    }

    fn find(&mut self, index: usize) -> usize {
        if self.parents[index] != index {
            let parent = self.parents[index];
            self.parents[index] = self.find(parent);
        }
        self.parents[index]
    }

    fn union(&mut self, left: usize, right: usize) -> bool {
        let left_root = self.find(left);
        let right_root = self.find(right);
        if left_root == right_root {
            return false;
        }

        if self.ranks[left_root] < self.ranks[right_root] {
            self.parents[left_root] = right_root;
        } else if self.ranks[left_root] > self.ranks[right_root] {
            self.parents[right_root] = left_root;
        } else {
            self.parents[right_root] = left_root;
            self.ranks[left_root] += 1;
        }
        self.components -= 1;
        true
    }

    fn components(&self) -> usize {
        self.components
    }
}

fn invalid_argument(message: impl Into<String>) -> DetectError {
    DetectError::InvalidArgument(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_directed_cycle() {
        let graph = Graph::from_edges(
            GraphKind::Directed,
            [
                GraphEdge::new("a", "b").unwrap(),
                GraphEdge::new("b", "c").unwrap(),
                GraphEdge::new("c", "a").unwrap(),
            ],
        )
        .unwrap();

        let cycle = find_cycle(&graph).unwrap();
        assert_eq!(
            cycle.nodes,
            vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "a".to_string()
            ]
        );
    }

    #[test]
    fn distinguishes_weak_and_strong_connectivity() {
        let graph = Graph::from_edges(
            GraphKind::Directed,
            [
                GraphEdge::new("a", "b").unwrap(),
                GraphEdge::new("b", "c").unwrap(),
            ],
        )
        .unwrap();

        assert!(is_weakly_connected(&graph));
        assert!(!is_strongly_connected(&graph));
        assert_eq!(strongly_connected_components(&graph).len(), 3);
    }

    #[test]
    fn computes_shortest_path() {
        let graph = Graph::from_edges(
            GraphKind::Undirected,
            [
                GraphEdge::weighted("a", "b", 1.0).unwrap(),
                GraphEdge::weighted("b", "c", 2.0).unwrap(),
                GraphEdge::weighted("a", "c", 5.0).unwrap(),
            ],
        )
        .unwrap();

        let path = shortest_path(&graph, "a", "c").unwrap().unwrap();
        assert_eq!(
            path.nodes,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
        assert_eq!(path.total_weight, 3.0);
    }

    #[test]
    fn rejects_negative_edge_weights_for_shortest_path() {
        let graph = Graph::from_edges(
            GraphKind::Directed,
            [GraphEdge::weighted("a", "b", -1.0).unwrap()],
        )
        .unwrap();

        let error = shortest_paths_from(&graph, "a").unwrap_err();
        assert!(matches!(error, DetectError::InvalidArgument(_)));
    }

    #[test]
    fn computes_minimum_spanning_tree() {
        let graph = Graph::from_edges(
            GraphKind::Undirected,
            [
                GraphEdge::weighted("a", "b", 4.0).unwrap(),
                GraphEdge::weighted("a", "c", 1.0).unwrap(),
                GraphEdge::weighted("b", "c", 2.0).unwrap(),
                GraphEdge::weighted("b", "d", 3.0).unwrap(),
                GraphEdge::weighted("c", "d", 5.0).unwrap(),
            ],
        )
        .unwrap();

        let tree = minimum_spanning_tree(&graph).unwrap();
        assert_eq!(tree.total_weight, 6.0);
        assert_eq!(tree.edges.len(), 3);
    }

    #[test]
    fn analyzes_tree_shape() {
        let graph = Graph::from_edges(
            GraphKind::Undirected,
            [
                GraphEdge::new("root", "left").unwrap(),
                GraphEdge::new("root", "right").unwrap(),
                GraphEdge::new("left", "leaf").unwrap(),
            ],
        )
        .unwrap();

        let analysis = analyze_tree(&graph, Some("root")).unwrap();
        assert!(analysis.is_tree);
        assert_eq!(analysis.roots, vec!["root".to_string()]);
        assert_eq!(analysis.depths["leaf"], 2);
        assert_eq!(
            analysis.leaves,
            vec!["leaf".to_string(), "right".to_string()]
        );
    }

    #[test]
    fn normalizes_undirected_edges() {
        let mut graph = Graph::undirected();
        graph.connect_weighted("z", "a", 2.0).unwrap();

        assert_eq!(graph.edges()[0].source, "a");
        assert_eq!(graph.edges()[0].target, "z");
    }

    #[test]
    fn connected_components_require_undirected_graphs() {
        let graph = Graph::from_edges(
            GraphKind::Directed,
            [
                GraphEdge::new("a", "b").unwrap(),
                GraphEdge::new("c", "d").unwrap(),
            ],
        )
        .unwrap();

        let error = connected_components(&graph).unwrap_err();
        assert!(matches!(error, DetectError::InvalidArgument(_)));
    }
}
