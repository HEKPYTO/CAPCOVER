use crate::bench::{hardware, peak_rss_mb, Hardware};
use crate::error::{Error, Result};
use crate::index::CapCover;
use crate::query::alpha_from_similarity;
use crate::vector::{dot, Vectors};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use usearch::{new_index, Index, IndexOptions, MetricKind, ScalarKind};

pub struct UsearchGate {
    index: Index,
    count: usize,
}

impl UsearchGate {
    pub fn build(vectors: &Vectors) -> Result<Self> {
        vectors.validate()?;
        let options = IndexOptions {
            dimensions: vectors.dim,
            metric: MetricKind::Cos,
            quantization: ScalarKind::F32,
            connectivity: 0,
            expansion_add: 0,
            expansion_search: 0,
            multi: false,
        };
        let index = new_index(&options)
            .map_err(|error| Error::InvalidData(format!("usearch init: {error}")))?;
        index
            .reserve(vectors.n)
            .map_err(|error| Error::InvalidData(format!("usearch reserve: {error}")))?;
        for position in 0..vectors.n {
            index
                .add(position as u64, vectors.row(position))
                .map_err(|error| Error::InvalidData(format!("usearch add: {error}")))?;
        }
        Ok(Self {
            index,
            count: vectors.n,
        })
    }

    pub fn search(&self, query: &[f32], count: usize) -> Result<Vec<u64>> {
        let matches = self
            .index
            .search(query, count)
            .map_err(|error| Error::InvalidData(format!("usearch search: {error}")))?;
        Ok(matches.keys)
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UsearchReport {
    pub n: usize,
    pub d: usize,
    pub k: usize,
    pub top_k: usize,
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
    pub gate_recall: f64,
    pub search_recall: f64,
    pub usearch_build_s: f64,
    pub filter_p50_us: f64,
    pub usearch_p50_us: f64,
    pub qps_base: f64,
    pub qps_gated: f64,
    pub speedup: f64,
    pub peak_rss_mb: f64,
    pub seed: u64,
    pub hardware: Hardware,
}

pub fn run(
    index: &CapCover,
    vectors: &Vectors,
    queries: &Vectors,
    similarity: f32,
    top_k: usize,
) -> Result<UsearchReport> {
    if top_k == 0 {
        return Err(Error::InvalidData("top_k must be positive".into()));
    }
    if vectors.dim != index.dim() || vectors.n != index.n() {
        return Err(Error::InvalidData(
            "vectors do not match the index shape".into(),
        ));
    }
    if queries.dim != index.dim() || queries.n == 0 {
        return Err(Error::InvalidData(
            "queries do not match the index shape".into(),
        ));
    }
    vectors.validate()?;
    queries.validate()?;
    let alpha = alpha_from_similarity(similarity)?;
    let build_start = Instant::now();
    let gate = UsearchGate::build(vectors)?;
    let usearch_build_s = build_start.elapsed().as_secs_f64();
    let mut filter_us = Vec::with_capacity(queries.n);
    let mut decisions = vec![false; queries.n];
    for (query_index, decision) in decisions.iter_mut().enumerate() {
        let start = Instant::now();
        *decision = index.may_contain(queries.row(query_index), similarity)?;
        filter_us.push(start.elapsed().as_secs_f64() * 1e6);
    }
    let mut truth = vec![false; queries.n];
    let mut best_keys = vec![0u64; queries.n];
    for query_index in 0..queries.n {
        let (best, key) =
            brute_nearest(vectors, queries.row(query_index), queries.norm(query_index));
        best_keys[query_index] = key;
        truth[query_index] = best >= similarity as f64 - 1e-12;
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
    let mut ungated_us = Vec::with_capacity(queries.n);
    let mut search_hits = 0u64;
    let mut search_total = 0u64;
    for query_index in 0..queries.n {
        let start = Instant::now();
        let keys = gate.search(queries.row(query_index), top_k)?;
        ungated_us.push(start.elapsed().as_secs_f64() * 1e6);
        if truth[query_index] {
            search_total += 1;
            if keys.contains(&best_keys[query_index]) {
                search_hits += 1;
            }
        }
    }
    let mut gated_us = Vec::with_capacity(queries.n);
    for (query_index, decision) in decisions.iter().enumerate() {
        if *decision {
            let start = Instant::now();
            let _ = gate.search(queries.row(query_index), top_k)?;
            gated_us.push(start.elapsed().as_secs_f64() * 1e6);
        }
    }
    let query_count = queries.n as f64;
    let filter_total: f64 = filter_us.iter().sum();
    let ungated_total: f64 = ungated_us.iter().sum();
    let gated_total: f64 = filter_total + gated_us.iter().sum::<f64>();
    let mut sorted_filter = filter_us;
    sorted_filter.sort_by(|left, right| left.partial_cmp(right).unwrap());
    let mut sorted_usearch = ungated_us;
    sorted_usearch.sort_by(|left, right| left.partial_cmp(right).unwrap());
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
    let search_recall = if search_total > 0 {
        search_hits as f64 / search_total as f64
    } else {
        0.0
    };
    Ok(UsearchReport {
        n: index.n(),
        d: index.dim(),
        k: index.k(),
        top_k,
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
        gate_recall: 1.0 - fnr,
        search_recall,
        usearch_build_s,
        filter_p50_us: percentile(&sorted_filter, 0.50),
        usearch_p50_us: percentile(&sorted_usearch, 0.50),
        qps_base: query_count / (ungated_total / 1e6),
        qps_gated: query_count / (gated_total / 1e6),
        speedup: ungated_total / gated_total,
        peak_rss_mb: peak_rss_mb(),
        seed: index.seed(),
        hardware: hardware(),
    })
}

fn brute_nearest(vectors: &Vectors, query: &[f32], query_norm: f64) -> (f64, u64) {
    let mut best = -1.0;
    let mut best_key = 0u64;
    for position in 0..vectors.n {
        let value = dot(query, vectors.row(position)) / (query_norm * vectors.norm(position));
        if value > best {
            best = value;
            best_key = position as u64;
        }
    }
    (best.clamp(-1.0, 1.0), best_key)
}

fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let position = ((sorted.len() - 1) as f64 * fraction).round() as usize;
    sorted[position]
}
