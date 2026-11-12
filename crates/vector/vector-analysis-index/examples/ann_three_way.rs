//! Three-way exact / cosine LSH / external HNSW parity and cost report (#119).
//!
//! ```text
//! cargo run --release -p moenarch-vector-analysis-index --features hnsw --example ann_three_way
//! ```
//!
//! The workload is deterministic: clustered Gaussian vectors from a fixed
//! seed, queries drawn from the same distribution but not indexed, cosine
//! metric, top-10, single-threaded. Recall and ordering numbers are exact and
//! repeatable; wall-clock and heap numbers describe the machine that ran it
//! and are not a blocking gate (BENCH-003).

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use vector_analysis_core::{DenseVector, VectorMetric};
use vector_analysis_index::{
    approximate::{CosineLshConfig, CosineLshIndex, CosineLshSearchConfig},
    hnsw::{CosineHnswConfig, CosineHnswIndex, CosineHnswSearchConfig},
    SearchConfig, SearchResult, VectorRecord, VectorSearchIndex,
};

struct CountingAllocator;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            let current = CURRENT.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(current, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        CURRENT.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const DIMENSIONS: usize = 128;
const CLUSTERS: usize = 64;
const QUERIES: usize = 200;
const TOP_K: usize = 10;
const SEED: u64 = 0x1190_a11e;

struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn unit(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    fn gaussian(&mut self) -> f32 {
        let radius = (-2.0 * self.unit().ln()).sqrt();
        (radius * (std::f64::consts::TAU * self.unit()).cos()) as f32
    }
}

struct Workload {
    records: Vec<VectorRecord>,
    queries: Vec<DenseVector>,
    input_hash: u64,
}

fn workload(corpus_size: usize) -> Workload {
    let mut rng = Rng(SEED);
    let centers = (0..CLUSTERS)
        .map(|_| (0..DIMENSIONS).map(|_| rng.gaussian()).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let sample = |rng: &mut Rng| {
        let center = &centers[(rng.next_u64() % CLUSTERS as u64) as usize];
        center
            .iter()
            .map(|value| value + 0.6 * rng.gaussian())
            .collect::<Vec<_>>()
    };
    let mut input_hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut hash = |values: &[f32]| {
        for value in values {
            for byte in value.to_bits().to_le_bytes() {
                input_hash = (input_hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
            }
        }
    };
    let records = (0..corpus_size)
        .map(|index| {
            let values = sample(&mut rng);
            hash(&values);
            VectorRecord::new(
                format!("record-{index:06}"),
                DenseVector::new(values).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let queries = (0..QUERIES)
        .map(|_| {
            let values = sample(&mut rng);
            hash(&values);
            DenseVector::new(values).unwrap()
        })
        .collect::<Vec<_>>();
    Workload {
        records,
        queries,
        input_hash,
    }
}

struct Row {
    path: String,
    recall: f64,
    top1: f64,
    exact_order: f64,
    build_ms: f64,
    query_us: f64,
    scored: f64,
    heap_mib: f64,
}

fn measure_build<T>(build: impl FnOnce() -> T) -> (T, f64, f64) {
    let baseline = CURRENT.load(Ordering::Relaxed);
    PEAK.store(baseline, Ordering::Relaxed);
    let started = Instant::now();
    let index = build();
    let build_ms = started.elapsed().as_secs_f64() * 1e3;
    let heap_mib = (PEAK.load(Ordering::Relaxed) - baseline) as f64 / (1024.0 * 1024.0);
    (index, build_ms, heap_mib)
}

fn row(
    path: String,
    exact: &[Vec<SearchResult>],
    build_ms: f64,
    heap_mib: f64,
    queries: &[DenseVector],
    mut search: impl FnMut(&DenseVector) -> (Vec<SearchResult>, usize),
) -> Row {
    let mut hits = 0usize;
    let mut top1 = 0usize;
    let mut exact_order = 0usize;
    let mut scored = 0usize;
    let started = Instant::now();
    let answers = queries.iter().map(&mut search).collect::<Vec<_>>();
    let query_us = started.elapsed().as_secs_f64() * 1e6 / queries.len() as f64;
    for ((results, candidates), expected) in answers.iter().zip(exact) {
        let expected_ids = expected
            .iter()
            .map(|result| result.id.as_str())
            .collect::<BTreeSet<_>>();
        hits += results
            .iter()
            .filter(|result| expected_ids.contains(result.id.as_str()))
            .count();
        top1 += usize::from(results.first().map(|r| &r.id) == expected.first().map(|r| &r.id));
        exact_order += usize::from(results == expected);
        scored += candidates;
    }
    let count = queries.len() as f64;
    Row {
        path,
        recall: hits as f64 / (count * TOP_K as f64),
        top1: top1 as f64 / count,
        exact_order: exact_order as f64 / count,
        build_ms,
        query_us,
        scored: scored as f64 / count,
        heap_mib,
    }
}

fn main() {
    let scales = std::env::args()
        .skip(1)
        .map(|value| value.parse::<usize>().expect("corpus sizes are integers"))
        .collect::<Vec<_>>();
    let scales = if scales.is_empty() {
        vec![2_048, 8_192, 32_768]
    } else {
        scales
    };
    println!(
        "workload: clustered gaussian, D={DIMENSIONS}, clusters={CLUSTERS}, queries={QUERIES}, cosine, top-{TOP_K}, seed={SEED:#x}, single thread"
    );
    for corpus_size in scales {
        let workload = workload(corpus_size);
        println!(
            "\nN={corpus_size} input_fnv1a64={:#018x}",
            workload.input_hash
        );
        println!(
            "{:<34} {:>8} {:>6} {:>7} {:>10} {:>10} {:>9} {:>9}",
            "path", "recall", "top1", "order", "build_ms", "query_us", "scored", "heap_MiB"
        );
        let cosine = SearchConfig {
            metric: VectorMetric::Cosine,
            limit: TOP_K,
        };

        let (exact, build_ms, heap_mib) =
            measure_build(|| VectorSearchIndex::from_records(workload.records.clone()).unwrap());
        let exact_answers = workload
            .queries
            .iter()
            .map(|query| exact.search(query, cosine).unwrap())
            .collect::<Vec<_>>();
        let mut rows = vec![row(
            "exact".into(),
            &exact_answers,
            build_ms,
            heap_mib,
            &workload.queries,
            |query| (exact.search(query, cosine).unwrap(), corpus_size),
        )];

        for (hash_bits, probe_radius, max_candidates) in
            [(16, 1, 256), (12, 2, 1_024), (8, 2, 2_048)]
        {
            let config = CosineLshConfig {
                hash_bits,
                ..CosineLshConfig::default()
            };
            let (lsh, build_ms, heap_mib) = measure_build(|| {
                CosineLshIndex::from_records(config, workload.records.clone()).unwrap()
            });
            let search = CosineLshSearchConfig {
                limit: TOP_K,
                probe_radius,
                max_candidates,
            };
            rows.push(row(
                format!("lsh bits={hash_bits} r={probe_radius} C={max_candidates}"),
                &exact_answers,
                build_ms,
                heap_mib,
                &workload.queries,
                |query| {
                    let report = lsh.search(query, search).unwrap();
                    (report.results, report.candidate_count)
                },
            ));
        }

        let config = CosineHnswConfig::default();
        let (hnsw, build_ms, heap_mib) = measure_build(|| {
            CosineHnswIndex::from_records(config, workload.records.clone()).unwrap()
        });
        let rebuilt = CosineHnswIndex::from_records(config, workload.records.clone()).unwrap();
        for ef_search in [32, 64, 128] {
            let search = CosineHnswSearchConfig {
                limit: TOP_K,
                ef_search,
            };
            let deterministic = workload.queries.iter().all(|query| {
                hnsw.search(query, search).unwrap() == rebuilt.search(query, search).unwrap()
            });
            assert!(
                deterministic,
                "HNSW rebuild changed results at ef={ef_search}"
            );
            rows.push(row(
                format!(
                    "hnsw M={} efC={} ef={ef_search}",
                    config.max_connections, config.ef_construction
                ),
                &exact_answers,
                build_ms,
                heap_mib,
                &workload.queries,
                |query| {
                    let report = hnsw.search(query, search).unwrap();
                    (report.results, report.candidate_count)
                },
            ));
        }

        for row in rows {
            println!(
                "{:<34} {:>8.4} {:>6.3} {:>7.3} {:>10.1} {:>10.1} {:>9.1} {:>9.2}",
                row.path,
                row.recall,
                row.top1,
                row.exact_order,
                row.build_ms,
                row.query_us,
                row.scored,
                row.heap_mib
            );
        }
        println!("hnsw rebuild determinism: identical results for every query and ef");
    }
}
