use criterion::{black_box, criterion_group, criterion_main, Criterion};
use vector_analysis_core::{DenseVector, VectorMetric};
use vector_analysis_index::{
    SearchConfig, VectorRecord, VectorRecordMetadata, VectorSearchFilter, VectorSearchIndex,
};

fn vector(dimensions: usize, seed: usize) -> DenseVector {
    DenseVector::new(
        (0..dimensions)
            .map(|index| {
                let value = (index as f32 * 0.031) + (seed as f32 * 0.013);
                value.sin() * 0.4 + value.cos() * 0.2
            })
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

fn index(record_count: usize, dimensions: usize) -> VectorSearchIndex {
    VectorSearchIndex::from_records((0..record_count).map(|record_index| {
        let payload = VectorRecordMetadata {
            tags: vec![format!("bucket-{}", record_index % 8)],
            metadata: [(
                String::from("kind"),
                if record_index % 3 == 0 {
                    "caption"
                } else {
                    "frame"
                }
                .to_string(),
            )]
            .into_iter()
            .collect(),
        };
        VectorRecord::with_payload(
            format!("record-{record_index:05}"),
            vector(dimensions, record_index),
            payload,
        )
    }))
    .unwrap()
}

fn bench_search(c: &mut Criterion) {
    let index = index(4_096, 128);
    let query = vector(128, 17);
    let query_slice = query.as_slice().to_vec();
    let cosine = SearchConfig {
        metric: VectorMetric::Cosine,
        limit: 10,
    };
    let euclidean = SearchConfig {
        metric: VectorMetric::Euclidean,
        limit: 10,
    };
    let filter = VectorSearchFilter {
        required_tags: vec![String::from("bucket-3")],
        metadata_equals: [(String::from("kind"), String::from("caption"))]
            .into_iter()
            .collect(),
    };

    c.bench_function("search_cosine_4k_x_128", |b| {
        b.iter(|| index.search(black_box(&query), black_box(cosine)).unwrap())
    });

    c.bench_function("search_euclidean_4k_x_128", |b| {
        b.iter(|| {
            index
                .search(black_box(&query), black_box(euclidean))
                .unwrap()
        })
    });

    c.bench_function("search_filtered_4k_x_128", |b| {
        b.iter(|| {
            index
                .search_filtered(
                    black_box(&query_slice),
                    black_box(10),
                    black_box(Some(&filter)),
                )
                .unwrap()
        })
    });
}

criterion_group!(benches, bench_search);
criterion_main!(benches);
