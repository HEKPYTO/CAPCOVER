use crate::build::{build, BuildConfig};
use crate::error::{Error, Result};
use crate::index::CapCover;
use crate::query::alpha_from_similarity;
use crate::vector::{dot, Vectors};
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hardware {
    pub os: String,
    pub arch: String,
    pub cpus: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchReport {
    pub n: usize,
    pub d: usize,
    pub k: usize,
    pub encoding: String,
    pub file_bytes: u64,
    pub bytes_per_vector: f64,
    pub budget_ratio: f64,
    pub build_s: f64,
    pub peak_rss_mb: f64,
    pub similarity: f32,
    pub tau: f32,
    pub alpha_rad: f64,
    pub tp: u64,
    pub fp: u64,
    pub tn: u64,
    #[serde(rename = "fn")]
    pub fn_: u64,
    pub fnr: f64,
    pub fpr: f64,
    pub phi: f64,
    pub filter_p50_us: f64,
    pub filter_p99_us: f64,
    pub brute_ms: f64,
    pub baseline_ms: f64,
    pub qps_base: f64,
    pub qps_filtered: f64,
    pub speedup: f64,
    pub seed: u64,
    pub hardware: Hardware,
}

pub fn run(
    index: &CapCover,
    vectors: &Vectors,
    queries: &Vectors,
    similarity: f32,
    baseline_ms: Option<f64>,
) -> Result<BenchReport> {
    if vectors.dim != index.dim() {
        return Err(Error::InvalidData(format!(
            "vector dimension {} does not match index dimension {}",
            vectors.dim,
            index.dim()
        )));
    }
    if vectors.n != index.n() {
        return Err(Error::InvalidData(format!(
            "vector count {} does not match index count {}",
            vectors.n,
            index.n()
        )));
    }
    if queries.dim != index.dim() {
        return Err(Error::InvalidData(format!(
            "query dimension {} does not match index dimension {}",
            queries.dim,
            index.dim()
        )));
    }
    if queries.n == 0 {
        return Err(Error::InvalidData("bench needs at least one query".into()));
    }
    vectors.validate()?;
    queries.validate()?;
    let alpha = alpha_from_similarity(similarity)?;
    let build_start = Instant::now();
    let rebuilt = build(
        vectors,
        &BuildConfig {
            budget_bytes: index.budget_bytes(),
            seed: index.seed(),
        },
    )?;
    let build_s = build_start.elapsed().as_secs_f64();
    if rebuilt.k != index.k()
        || rebuilt.file_bytes()? as u64 != index.file_bytes()
        || rebuilt.directions.as_slice() != index.directions()
        || rebuilt.angles.as_slice() != index.angles()
        || rebuilt.assignment.as_slice() != index.assignment()
    {
        return Err(Error::InvalidData(
            "index does not match a fresh build of the supplied vectors".into(),
        ));
    }
    let mut filter_us = Vec::with_capacity(queries.n);
    let mut decisions = vec![false; queries.n];
    for (query_index, decision_slot) in decisions.iter_mut().enumerate() {
        let start = Instant::now();
        *decision_slot = index.may_contain(queries.row(query_index), similarity)?;
        filter_us.push(start.elapsed().as_secs_f64() * 1e6);
    }
    let mut brute_us = Vec::with_capacity(queries.n);
    let mut truth = vec![false; queries.n];
    for (query_index, truth_slot) in truth.iter_mut().enumerate() {
        let start = Instant::now();
        let best = brute_max(vectors, queries.row(query_index), queries.norm(query_index));
        brute_us.push(start.elapsed().as_secs_f64() * 1e6);
        *truth_slot = best >= similarity as f64 - 1e-12;
    }
    let mut tp = 0u64;
    let mut fp = 0u64;
    let mut tn = 0u64;
    let mut fn_ = 0u64;
    for query_index in 0..queries.n {
        match (decisions[query_index], truth[query_index]) {
            (true, true) => tp += 1,
            (true, false) => fp += 1,
            (false, false) => tn += 1,
            (false, true) => fn_ += 1,
        }
    }
    let query_count = queries.n as f64;
    let filter_total_us: f64 = filter_us.iter().sum();
    let brute_total_us: f64 = brute_us.iter().sum();
    let measured_baseline_ms = brute_total_us / query_count / 1000.0;
    let baseline_ms = baseline_ms.unwrap_or(measured_baseline_ms);
    if !baseline_ms.is_finite() || baseline_ms <= 0.0 {
        return Err(Error::InvalidData("baseline must be positive".into()));
    }
    let baseline_total_us = baseline_ms * 1000.0 * query_count;
    let accepted = (tp + fp) as f64;
    let filtered_total_us = filter_total_us + accepted * baseline_total_us / query_count;
    let mut sorted = filter_us;
    sorted.sort_by(|left, right| left.partial_cmp(right).unwrap());
    let fnr = if tp + fn_ > 0 {
        fn_ as f64 / (tp + fn_) as f64
    } else {
        0.0
    };
    let fpr = if fp + tn > 0 {
        fp as f64 / (fp + tn) as f64
    } else {
        0.0
    };
    let phi = tn as f64 / query_count;
    let file_bytes = index.file_bytes();
    let budget_ratio = file_bytes as f64 / (index.n() as f64 * index.dim() as f64 * 4.0);
    let bytes_per_vector = file_bytes as f64 / index.n() as f64;
    Ok(BenchReport {
        n: index.n(),
        d: index.dim(),
        k: index.k(),
        encoding: index.info().encoding.to_string(),
        file_bytes,
        bytes_per_vector,
        budget_ratio,
        build_s,
        peak_rss_mb: peak_rss_mb(),
        similarity,
        tau: 1.0 - similarity,
        alpha_rad: alpha,
        tp,
        fp,
        tn,
        fn_,
        fnr,
        fpr,
        phi,
        filter_p50_us: percentile(&sorted, 0.50),
        filter_p99_us: percentile(&sorted, 0.99),
        brute_ms: measured_baseline_ms,
        baseline_ms,
        qps_base: query_count / (baseline_total_us / 1e6),
        qps_filtered: query_count / (filtered_total_us / 1e6),
        speedup: baseline_total_us / filtered_total_us,
        seed: index.seed(),
        hardware: hardware(),
    })
}

fn brute_max(vectors: &Vectors, query: &[f32], query_norm: f64) -> f64 {
    let mut best = -1.0;
    for index in 0..vectors.n {
        let value = dot(query, vectors.row(index)) / (query_norm * vectors.norm(index));
        if value > best {
            best = value;
        }
    }
    best.clamp(-1.0, 1.0)
}

fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let position = ((sorted.len() - 1) as f64 * fraction).round() as usize;
    sorted[position]
}

pub(crate) fn hardware() -> Hardware {
    Hardware {
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        cpus: std::thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(1),
    }
}

#[cfg(unix)]
pub(crate) fn peak_rss_mb() -> f64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    let status = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if status != 0 {
        return 0.0;
    }
    let usage = unsafe { usage.assume_init() };
    let value = usage.ru_maxrss as f64;
    if cfg!(target_os = "macos") {
        value / (1024.0 * 1024.0)
    } else {
        value / 1024.0
    }
}

#[cfg(not(unix))]
pub(crate) fn peak_rss_mb() -> f64 {
    0.0
}
