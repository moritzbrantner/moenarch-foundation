//! Complexity contracts for exact and LSH vector search (`.performance/contract.json`
//! `operations`, #153). Each test builds one deterministic corpus family, changes one dimension
//! at a time, checks results against the exact reference and records the test-only work
//! counters. With `FOUNDATION_WORK_EVIDENCE_DIR` set (`scripts/collect-work-evidence.sh`),
//! samples go to `<operation>.json` for `coding-tooling performance work`; otherwise the test
//! asserts the declared exact bounds itself.
use vector_analysis_core::DenseVector;
use vector_analysis_core::VectorMetric;
use vector_analysis_index::{
    approximate::{CosineLshConfig, CosineLshIndex, CosineLshSearchConfig, CosineLshSearchReport},
    work::{self, Counter},
    SearchConfig, SearchResult, VectorRecord, VectorSearchIndex,
};

// ---- evidence ----

type Pairs = Vec<(&'static str, u64)>;
type Sample = (Pairs, Pairs);

fn evidence_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("FOUNDATION_WORK_EVIDENCE_DIR").map(Into::into)
}

fn write(operation: &str, samples: &[Sample]) {
    let fields = |pairs: &[(&str, u64)]| {
        pairs
            .iter()
            .map(|(k, v)| format!("\"{k}\":{v}"))
            .collect::<Vec<_>>()
            .join(",")
    };
    let body = samples
        .iter()
        .map(|(d, m)| {
            format!(
                "{{\"dimensions\":{{{}}},\"metrics\":{{{}}}}}",
                fields(d),
                fields(m)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let dir = evidence_dir().expect("evidence directory");
    std::fs::write(
        dir.join(format!("{operation}.json")),
        format!("{{\"id\":\"{operation}\",\"samples\":[{body}]}}\n"),
    )
    .expect("writable evidence directory");
}

/// Self-check of the exact classes `constant` and `linear` (the same rules as
/// `coding-tooling performance work`); tolerant classes are judged there. Samples are grouped
/// by every dimension other than the bounded one.
fn judge(operation: &str, samples: &[Sample], bounds: &[(&str, &str, &str)]) {
    if std::env::var_os("WORK_PRINT").is_some() {
        eprintln!("{operation}: {samples:?}");
    }
    if evidence_dir().is_some() {
        write(operation, samples);
        return;
    }
    let value = |s: &Sample, name: &str| {
        s.0.iter()
            .chain(&s.1)
            .find(|(k, _)| *k == name)
            .map(|(_, v)| *v)
            .unwrap_or_else(|| panic!("{operation} sample has no {name}"))
    };
    for &(metric, dimension, bound) in bounds {
        let mut groups: Vec<Vec<&Sample>> = Vec::new();
        for s in samples {
            let key: Vec<_> = s.0.iter().filter(|(k, _)| *k != dimension).collect();
            let same = |g: &Vec<&Sample>| {
                g[0].0
                    .iter()
                    .filter(|(k, _)| *k != dimension)
                    .collect::<Vec<_>>()
                    == key
            };
            match groups.iter_mut().find(|g| same(g)) {
                Some(g) => g.push(s),
                None => groups.push(vec![s]),
            }
        }
        for mut g in groups.into_iter().filter(|g| g.len() > 1) {
            g.sort_by_key(|s| value(s, dimension));
            let x: Vec<i128> = g.iter().map(|s| value(s, dimension) as i128).collect();
            let v: Vec<i128> = g.iter().map(|s| value(s, metric) as i128).collect();
            match bound {
                "constant" => assert!(
                    v.iter().all(|&y| y <= v[0]),
                    "{operation}.{metric} grows with {dimension}: {v:?} at {x:?}"
                ),
                "linear" if g.len() == 2 => assert!(
                    v[1] * x[0] <= v[0] * x[1],
                    "{operation}.{metric} grows faster than linear in {dimension}: {v:?} at {x:?}"
                ),
                "linear" => {
                    let first = (v[1] - v[0]).max(0);
                    for i in 1..v.len() - 1 {
                        assert!(
                            (v[i + 1] - v[i]) * (x[1] - x[0]) <= first * (x[i + 1] - x[i]),
                            "{operation}.{metric} grows faster than linear in {dimension}: {v:?} at {x:?}"
                        );
                    }
                }
                other => panic!("{other} is judged by coding-tooling"),
            }
        }
    }
}

/// Counter values of `f` on this thread.
fn counted<T>(f: impl FnOnce() -> T) -> (T, [u64; 5]) {
    work::reset();
    let out = f();
    let read = [
        Counter::SimilarityEvaluations,
        Counter::SimilarityComponents,
        Counter::ProjectionTerms,
        Counter::SignatureProbes,
        Counter::BucketEntryVisits,
    ]
    .map(work::get);
    (out, read)
}

// ---- deterministic corpora ----

const LIMIT: usize = 10;
const QUERIES: u64 = 16;
const LSH_BITS: u8 = 8;
const CORPUS_SEED: u64 = 0x5eed_0153;
const QUERY_SEED: u64 = 0x0153_9e37;

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

/// Component `j` of stream vector `i`, uniform in [-1, 1). Vector `i` does not depend on how
/// many vectors are drawn, so a larger corpus extends a smaller one.
fn component(seed: u64, i: u64, j: usize) -> f32 {
    let bits =
        splitmix64(seed ^ splitmix64(i) ^ (j as u64 + 1).wrapping_mul(0xd1b5_4a32_d192_ed03));
    ((bits >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
}

fn uniform(seed: u64, i: u64, dimensions: usize) -> DenseVector {
    DenseVector::new(
        (0..dimensions)
            .map(|j| component(seed, i, j))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

fn uniform_records(n: usize, dimensions: usize) -> Vec<VectorRecord> {
    (0..n as u64)
        .map(|i| VectorRecord::new(format!("r{i:06}"), uniform(CORPUS_SEED, i, dimensions)))
        .collect()
}

fn queries(dimensions: usize) -> Vec<DenseVector> {
    (0..QUERIES)
        .map(|q| uniform(QUERY_SEED, q, dimensions))
        .collect()
}

/// Collision-heavy corpus: every record is the same direction plus a 1e-4 perturbation, so the
/// whole corpus shares one LSH bucket.
fn collision_records(n: usize, dimensions: usize) -> Vec<VectorRecord> {
    (0..n as u64)
        .map(|i| {
            let values: Vec<f32> = (0..dimensions)
                .map(|j| component(QUERY_SEED, 0, j) + 1e-4 * component(CORPUS_SEED, i, j))
                .collect();
            VectorRecord::new(format!("c{i:06}"), DenseVector::new(values).unwrap())
        })
        .collect()
}

/// Probe signatures within Hamming radius `r` of `B = LSH_BITS` bits.
fn probes(radius: u8) -> u64 {
    let b = u64::from(LSH_BITS);
    [1, b, b * (b - 1) / 2, b * (b - 1) * (b - 2) / 6][..=usize::from(radius)]
        .iter()
        .sum()
}

fn lsh(records: Vec<VectorRecord>, hash_bits: u8) -> CosineLshIndex {
    CosineLshIndex::from_records(
        CosineLshConfig {
            hash_bits,
            ..CosineLshConfig::default()
        },
        records,
    )
    .unwrap()
}

fn lsh_config(radius: u8, cap: usize) -> CosineLshSearchConfig {
    CosineLshSearchConfig {
        limit: LIMIT,
        probe_radius: radius,
        max_candidates: cap,
    }
}

fn exact(records: Vec<VectorRecord>) -> VectorSearchIndex {
    VectorSearchIndex::from_records(records).unwrap()
}

fn cosine(limit: usize) -> SearchConfig {
    SearchConfig {
        metric: VectorMetric::Cosine,
        limit,
    }
}

/// One LSH query with the issue's invariants checked against its own report.
fn lsh_query(
    index: &CosineLshIndex,
    query: &DenseVector,
    config: CosineLshSearchConfig,
) -> (CosineLshSearchReport, [u64; 5]) {
    let (report, c) = counted(|| index.search(query, config).unwrap());
    let [evaluations, components, _, signature_probes, entries] = c;
    assert!(evaluations <= config.max_candidates as u64, "{c:?}");
    assert_eq!(evaluations, report.candidate_count as u64);
    assert_eq!(
        components,
        evaluations * index.dimensions().unwrap_or(0) as u64
    );
    assert!(signature_probes <= probes(config.probe_radius), "{c:?}");
    assert!(report.probed_bucket_count as u64 <= signature_probes);
    // Each record lives in exactly one bucket: traversal never re-visits a candidate.
    assert_eq!(entries, evaluations);
    (report, c)
}

// ---- operations ----

/// Exact search scores every record once: N evaluations of D components each.
#[test]
fn exact_search_scores_every_record() {
    let points = [256usize, 512, 1024, 2048, 4096]
        .map(|n| (n, 32usize))
        .into_iter()
        .chain([8usize, 16, 64, 128].map(|d| (1024, d)));
    let mut samples = Vec::new();
    for (n, d) in points {
        let index = exact(uniform_records(n, d));
        let mut total = [0u64; 5];
        for query in queries(d) {
            let (results, c) = counted(|| index.search(&query, cosine(LIMIT)).unwrap());
            assert_eq!(results.len(), LIMIT);
            assert_eq!(c[0], n as u64);
            for (t, v) in total.iter_mut().zip(c) {
                *t += v;
            }
        }
        samples.push((
            vec![("records", n as u64), ("dimensions", d as u64)],
            vec![
                ("similarity-evaluations", total[0]),
                ("similarity-components", total[1]),
            ],
        ));
    }
    judge(
        "exact-search",
        &samples,
        &[
            ("similarity-evaluations", "records", "linear"),
            ("similarity-evaluations", "dimensions", "constant"),
            ("similarity-components", "records", "linear"),
            ("similarity-components", "dimensions", "linear"),
        ],
    );
}

fn lsh_sample(n: usize, d: usize, radius: u8, cap: usize, hash_bits: u8) -> Sample {
    let index = lsh(uniform_records(n, d), hash_bits);
    let mut total = [0u64; 5];
    for query in queries(d) {
        let (report, c) = lsh_query(&index, &query, lsh_config(radius, cap));
        assert_eq!(report.total_record_count, n);
        assert!(report.results.len() <= LIMIT);
        for (t, v) in total.iter_mut().zip(c) {
            *t += v;
        }
    }
    (
        vec![
            ("records", n as u64),
            ("dimensions", d as u64),
            ("probes", probes(radius)),
            ("cap", cap as u64),
        ],
        vec![
            ("exact-similarity-evaluations", total[0]),
            ("similarity-components", total[1]),
            ("projection-terms", total[2]),
            ("signature-probes", total[3]),
            ("bucket-entry-visits", total[4]),
        ],
    )
}

/// LSH search on a uniform corpus with `B = 8` bits, once the buckets are dense enough to fill
/// the exact-score cap: scoring follows the cap `C`, not the corpus `N`; signature probes
/// follow `P`; the query signature costs `B * D`.
#[test]
fn lsh_search_scores_at_most_the_cap() {
    let points = [4096usize, 8192, 16384, 32768]
        .map(|n| (n, 32usize, 1u8, 64usize))
        .into_iter()
        .chain([8usize, 16, 64, 128].map(|d| (4096, d, 1, 64)))
        .chain([0u8, 2, 3].map(|r| (4096, 32, r, 64)))
        .chain([16usize, 32, 128, 256].map(|c| (4096, 32, 1, c)));
    let samples: Vec<Sample> = points
        .map(|(n, d, r, c)| lsh_sample(n, d, r, c, LSH_BITS))
        .collect();
    judge(
        "lsh-search",
        &samples,
        &[
            ("exact-similarity-evaluations", "records", "constant"),
            ("exact-similarity-evaluations", "dimensions", "constant"),
            ("exact-similarity-evaluations", "cap", "linear"),
            ("similarity-components", "records", "constant"),
            ("similarity-components", "dimensions", "linear"),
            ("similarity-components", "cap", "linear"),
            ("projection-terms", "records", "constant"),
            ("projection-terms", "dimensions", "linear"),
            ("projection-terms", "probes", "constant"),
            ("projection-terms", "cap", "constant"),
            ("signature-probes", "records", "constant"),
            ("signature-probes", "probes", "linear"),
            ("bucket-entry-visits", "records", "constant"),
            ("bucket-entry-visits", "cap", "linear"),
        ],
    );
}

/// With the cap at the corpus size the LSH traversal is a fraction of the corpus that grows
/// with it: bucket visits are linear in `N`, not sublinear.
#[test]
fn uncapped_lsh_traversal_follows_the_corpus() {
    let samples: Vec<Sample> = [1024usize, 2048, 4096, 8192, 16384]
        .map(|n| {
            let (dimensions, mut metrics) = lsh_sample(n, 32, 1, n, LSH_BITS);
            metrics.retain(|(k, _)| *k == "bucket-entry-visits" || *k == "signature-probes");
            (dimensions[..1].to_vec(), metrics)
        })
        .into();
    judge(
        "lsh-search-uncapped",
        &samples,
        &[("signature-probes", "records", "constant")],
    );
}

/// Collision-heavy buckets: the whole corpus shares the query's bucket, so the first probe
/// fills the cap and scoring stays at `C` however large the bucket is.
#[test]
fn collision_heavy_bucket_stops_at_the_cap() {
    let points = [1024usize, 4096, 16384]
        .map(|n| (n, 64usize))
        .into_iter()
        .chain([16usize, 256, 1024].map(|c| (4096, c)));
    let mut samples = Vec::new();
    for (n, cap) in points {
        let index = lsh(collision_records(n, 32), 16);
        let query = uniform(QUERY_SEED, 0, 32);
        let (report, c) = lsh_query(&index, &query, lsh_config(1, cap));
        assert_eq!(report.probed_bucket_count, 1, "one shared bucket");
        assert_eq!(report.candidate_count, cap);
        samples.push((
            vec![("records", n as u64), ("cap", cap as u64)],
            vec![
                ("exact-similarity-evaluations", c[0]),
                ("signature-probes", c[3]),
                ("bucket-entry-visits", c[4]),
            ],
        ));
    }
    judge(
        "lsh-search-collisions",
        &samples,
        &[
            ("exact-similarity-evaluations", "records", "constant"),
            ("exact-similarity-evaluations", "cap", "linear"),
            ("signature-probes", "records", "constant"),
            ("signature-probes", "cap", "constant"),
            ("bucket-entry-visits", "records", "constant"),
            ("bucket-entry-visits", "cap", "linear"),
        ],
    );
}

// ---- controls ----

#[test]
fn empty_indexes_score_nothing() {
    let query = uniform(QUERY_SEED, 0, 32);
    let (results, c) = counted(|| exact(Vec::new()).search(&query, cosine(LIMIT)).unwrap());
    assert!(results.is_empty());
    assert_eq!(c[0], 0);

    let index = lsh(Vec::new(), LSH_BITS);
    let (report, c) = lsh_query(&index, &query, lsh_config(2, 64));
    assert!(report.results.is_empty());
    assert_eq!((report.candidate_count, report.probed_bucket_count), (0, 0));
    // Every probe signature is looked up and misses.
    assert_eq!(c[3], probes(2));
}

#[test]
fn a_repeated_query_does_identical_work() {
    let lsh_index = lsh(uniform_records(4096, 32), LSH_BITS);
    let exact_index = exact(uniform_records(4096, 32));
    let query = uniform(QUERY_SEED, 3, 32);
    let first = lsh_query(&lsh_index, &query, lsh_config(1, 64));
    let warm = lsh_query(&lsh_index, &query, lsh_config(1, 64));
    assert_eq!(first, warm);
    let first = counted(|| exact_index.search(&query, cosine(LIMIT)).unwrap());
    let warm = counted(|| exact_index.search(&query, cosine(LIMIT)).unwrap());
    assert_eq!(first, warm);
}

fn ids(results: &[SearchResult]) -> Vec<&str> {
    results.iter().map(|r| r.id.as_str()).collect()
}

/// Probing every signature with an uncapped budget is exact search; LSH scores are always the
/// exact cosine distances of the returned ids.
#[test]
fn full_probe_lsh_matches_exact_search() {
    let records = uniform_records(2048, 32);
    let exact_index = exact(records.clone());
    let lsh_index = lsh(records, 3);
    for query in queries(32) {
        let reference = exact_index.search(&query, cosine(LIMIT)).unwrap();
        let (report, c) = lsh_query(&lsh_index, &query, lsh_config(3, 2048));
        assert_eq!(c[0], 2048);
        // The ranking key (distance) and ids are exact search's; LSH reports the cosine
        // similarity directly while exact search derives it from the distance, so the
        // reported scores may differ in the last bit.
        let key = |r: &[SearchResult]| {
            r.iter()
                .map(|r| (r.id.clone(), r.distance.to_bits()))
                .collect::<Vec<_>>()
        };
        assert_eq!(key(&report.results), key(&reference));
    }
}

/// Recall is a measured property of this LSH, not a guarantee: uncapped, a wider probe radius
/// selects a superset of candidates, so recall@10 never drops, and every approximate result
/// carries its exact distance.
#[test]
fn approximate_recall_is_monotone_in_the_probe_radius() {
    let records = uniform_records(4096, 32);
    let exact_index = exact(records.clone());
    let all = exact_index.search(&queries(32)[0], cosine(4096)).unwrap();
    assert_eq!(all.len(), 4096);
    let lsh_index = lsh(records, LSH_BITS);
    let mut recalls = Vec::new();
    for radius in 0..=3 {
        let mut hits = 0;
        for query in queries(32) {
            let reference = exact_index.search(&query, cosine(LIMIT)).unwrap();
            let scored = exact_index.search(&query, cosine(4096)).unwrap();
            let (report, _) = lsh_query(&lsh_index, &query, lsh_config(radius, 4096));
            for result in &report.results {
                let exact = scored.iter().find(|r| r.id == result.id).unwrap();
                assert_eq!(result.distance.to_bits(), exact.distance.to_bits());
            }
            let found = ids(&report.results);
            hits += ids(&reference)
                .iter()
                .filter(|id| found.contains(id))
                .count();
        }
        recalls.push(hits);
    }
    assert!(recalls.windows(2).all(|w| w[0] <= w[1]), "{recalls:?}");
    if std::env::var_os("WORK_PRINT").is_some() {
        eprintln!(
            "recall@{LIMIT} hits of {} by radius: {recalls:?}",
            QUERIES * LIMIT as u64
        );
    }
}
