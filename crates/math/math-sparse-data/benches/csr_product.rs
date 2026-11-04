//! Canonical CSR×CSR composition: Foundation (`mul_csr`, row-local ordered
//! accumulator) against the dev-only `sprs` SMMP oracle on named,
//! deterministic workloads.
//!
//! Before Criterion runs, a work report is printed per workload: candidate
//! scalar products and output nnz (Foundation's structural statistics,
//! cross-checked against an independent count), the explicit zeros `sprs`
//! retains, and heap allocation count/bytes of one product for each
//! implementation, measured with a counting global allocator. Allocation and
//! structural evidence is deterministic; wall-clock latency is not, and the two
//! are reported separately.
//!
//! Conversions to `sprs` happen outside the measured body; both sides measure
//! the product only. Run:
//! cargo bench --locked -p moenarch-math-sparse-data --bench csr_product

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use math_sparse_data::{CooMatrix, CsrMatrix};
use sprs::CsMat;
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct Counting;

static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every call is forwarded unchanged to the system allocator; the
// wrapper only updates relaxed counters.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        // SAFETY: forwarded with the caller's layout contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` was allocated by `System` with this layout.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
            BYTES.fetch_add(new_size, Ordering::Relaxed);
        }
        // SAFETY: forwarded with the caller's realloc contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn counted<T>(work: impl FnOnce() -> T) -> (T, usize, usize) {
    ALLOCATIONS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::Relaxed);
    let value = work();
    COUNTING.store(false, Ordering::Relaxed);
    (
        value,
        ALLOCATIONS.load(Ordering::Relaxed),
        BYTES.load(Ordering::Relaxed),
    )
}

struct Stream(u64);

impl Stream {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    fn value(&mut self) -> f32 {
        let magnitude = (self.below(16) + 1) as f32 * 0.25;
        if self.next() & 1 == 0 {
            magnitude
        } else {
            -magnitude
        }
    }
}

fn generated(seed: u64, rows: usize, cols: usize, degree: impl Fn(usize) -> usize) -> CsrMatrix {
    let mut stream = Stream(seed);
    let mut entries = Vec::new();
    for row in 0..rows {
        for _ in 0..degree(row) {
            entries.push((row, stream.below(cols), stream.value()));
        }
    }
    CooMatrix::new(rows, cols, entries)
        .unwrap()
        .to_csr()
        .unwrap()
}

fn to_sprs(matrix: &CsrMatrix) -> CsMat<f32> {
    let mut indptr = vec![0usize];
    let mut indices = Vec::new();
    let mut data = Vec::new();
    for row in matrix.rows_iter() {
        indices.extend_from_slice(row.indices());
        data.extend_from_slice(row.values());
        indptr.push(indices.len());
    }
    CsMat::new((matrix.rows(), matrix.cols()), indptr, indices, data)
}

fn workloads() -> Vec<(&'static str, CsrMatrix, CsrMatrix)> {
    let skew = |row: usize| if row.is_multiple_of(100) { 1_000 } else { 3 };
    let documents = generated(16, 2_000, 50_000, |_| 40);
    let documents_t = documents.transpose().unwrap();
    vec![
        (
            "tiny_4x4",
            generated(1, 4, 4, |_| 2),
            generated(2, 4, 4, |_| 2),
        ),
        (
            "highly_sparse_10k",
            generated(3, 10_000, 10_000, |_| 2),
            generated(4, 10_000, 10_000, |_| 2),
        ),
        (
            "thin_wide_16x4096_x_4096x1m",
            generated(5, 16, 4_096, |_| 32),
            generated(6, 4_096, 1_000_000, |_| 4),
        ),
        (
            "dense_rows_256",
            generated(7, 256, 256, |_| 64),
            generated(8, 256, 256, |_| 64),
        ),
        (
            "skewed_degree_5k",
            generated(9, 5_000, 5_000, skew),
            generated(10, 5_000, 5_000, skew),
        ),
        ("doc_term_gram_rows_2000x50k", documents, documents_t),
    ]
}

fn report(workloads: &[(&'static str, CsrMatrix, CsrMatrix)]) {
    println!(
        "workload,left_nnz,right_nnz,candidate_products,output_nnz,sprs_explicit_zeros,\
         foundation_allocs,foundation_alloc_bytes,sprs_allocs,sprs_alloc_bytes"
    );
    for (name, left, right) in workloads {
        let ((product, stats), f_allocs, f_bytes) =
            counted(|| left.mul_csr_with_stats(right).unwrap());
        let (left_s, right_s) = (to_sprs(left), to_sprs(right));
        let (oracle, s_allocs, s_bytes) = counted(|| &left_s * &right_s);
        let explicit_zeros = oracle.data().iter().filter(|value| **value == 0.0).count();
        let right_nnz = right.row_nnz();
        let independent: usize = left
            .rows_iter()
            .flat_map(|row| row.indices().to_vec())
            .map(|shared| right_nnz[shared])
            .sum();
        assert_eq!(stats.candidate_products, independent);
        assert_eq!(stats.output_nnz, oracle.nnz() - explicit_zeros);
        let left_nnz: usize = left.row_nnz().iter().sum();
        println!(
            "{name},{left_nnz},{},{},{},{explicit_zeros},{f_allocs},{f_bytes},{s_allocs},{s_bytes}",
            right_nnz.iter().sum::<usize>(),
            stats.candidate_products,
            product.row_nnz().iter().sum::<usize>(),
        );
    }
}

fn csr_product(c: &mut Criterion) {
    let workloads = workloads();
    report(&workloads);
    let mut group = c.benchmark_group("csr_mul_csr");
    for (name, left, right) in &workloads {
        group.bench_with_input(BenchmarkId::new("foundation", name), &(), |b, ()| {
            b.iter(|| black_box(black_box(left).mul_csr(black_box(right)).unwrap()))
        });
        let (left_s, right_s) = (to_sprs(left), to_sprs(right));
        group.bench_with_input(BenchmarkId::new("sprs", name), &(), |b, ()| {
            b.iter(|| black_box(black_box(&left_s) * black_box(&right_s)))
        });
    }
    group.finish();
}

criterion_group!(benches, csr_product);
criterion_main!(benches);
