//! Issue #160 candidate consumer: registry-shaped use of the optional `hnsw`
//! feature beside vector-analysis-core, as nlp-stack packages it.

use vector_analysis_core::DenseVector;
use vector_analysis_index::{
    hnsw::{CosineHnswConfig, CosineHnswIndex, CosineHnswSearchConfig},
    VectorRecord,
};

fn main() {
    let records = [
        ("east", [1.0_f32, 0.0, 0.0]),
        ("north", [0.0, 1.0, 0.0]),
        ("up", [0.0, 0.0, 1.0]),
        ("north-east", [1.0, 1.0, 0.0]),
    ]
    .into_iter()
    .map(|(id, values)| VectorRecord::new(id, DenseVector::new(values.to_vec()).unwrap()));
    let index = CosineHnswIndex::from_records(CosineHnswConfig::default(), records).unwrap();
    let query = DenseVector::new(vec![0.9, 0.1, 0.0]).unwrap();
    let report = index
        .search(
            &query,
            CosineHnswSearchConfig {
                limit: 2,
                ef_search: 8,
            },
        )
        .unwrap();
    let ids: Vec<&str> = report.results.iter().map(|result| result.id.as_str()).collect();
    assert_eq!(ids, ["east", "north-east"], "unexpected HNSW ranking");
    assert_eq!(report.total_record_count, 4);
    println!("hnsw candidate consumer ranked {ids:?}");
}
