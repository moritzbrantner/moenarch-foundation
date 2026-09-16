use criterion::{black_box, criterion_group, criterion_main, Criterion};
use graph_analysis_core::{minimum_spanning_tree, page_rank, Graph, PageRankConfig};

const NODES: usize = 2_048;

fn directed_graph() -> Graph {
    let mut graph = Graph::directed();
    for index in 0..NODES {
        graph.add_node(format!("n{index}")).unwrap();
    }
    for index in 0..NODES {
        for offset in [1_usize, 7, 31] {
            graph
                .connect(
                    format!("n{index}"),
                    format!("n{}", (index + offset) % NODES),
                )
                .unwrap();
        }
    }
    graph
}

fn undirected_graph() -> Graph {
    let mut graph = Graph::undirected();
    for index in 0..NODES {
        graph.add_node(format!("n{index}")).unwrap();
    }
    for index in 0..NODES {
        for offset in [1_usize, 7, 31, 127] {
            let target = (index + offset) % NODES;
            let weight = ((index * 37 + offset * 13) % 1_000) as f64 / 10.0 + 1.0;
            graph
                .connect_weighted(format!("n{index}"), format!("n{target}"), weight)
                .unwrap();
        }
    }
    graph
}

fn graph_benchmarks(c: &mut Criterion) {
    let directed = directed_graph();
    c.bench_function("graph/page_rank_2048_bounded_40", |b| {
        b.iter(|| {
            let report = page_rank(
                black_box(&directed),
                PageRankConfig {
                    damping: 0.85,
                    tolerance: 0.0,
                    max_iterations: 40,
                },
            )
            .unwrap();
            black_box((report.iterations, report.residual, report.scores));
        });
    });

    let undirected = undirected_graph();
    c.bench_function("graph/minimum_spanning_tree_2048", |b| {
        b.iter(|| {
            let forest = minimum_spanning_tree(black_box(&undirected)).unwrap();
            black_box((forest.component_count, forest.total_weight, forest.edges));
        });
    });
}

criterion_group!(benches, graph_benchmarks);
criterion_main!(benches);
