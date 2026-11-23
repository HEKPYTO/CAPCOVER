# tests

| File | Proof |
|---|---|
| `oracle.rs` | Five hand-computed cosines, threshold conversion, outward rounding, filter decisions |
| `zero_fn.rs` | 5 seeds x 4 thresholds x 4 fixtures, stored vectors and brute-force true matches |
| `similarity.rs` | Low and negative similarity sweep, antipodal cap, true matches accepted |
| `hostile.rs` | NaN, infinity, zero, wrong dimension, truncated file, bad magic, bad checksum, bad budget |
| `loader.rs` | `.fvecs` and `.f32` loading, truncated fvecs |
| `determinism.rs` | Same seed produces byte-identical `.ccv` |
| `format.rs` | Header and section sizes match `info` and the file |
| `bench.rs` | Bench counts sum to query count, FNR is zero, JSON carries `fn` |
| `quality.rs` | Clustered fixture reaches `phi >= 0.25` at 2% with zero FN |
| `cli.rs` | `gen`, `build`, `verify`, `check`, `info`, `bench`, `selftest` round trip, bitmap values |
| `usearch_gate.rs` | USearch gate never skips a true match; feature-gated |
| `capi.c` | C caller opens the index, checks shape, runs `may_contain` and bitmap |

## Zero-FN matrix

```
kinds      clustered, duplicates, uniform, edge
seeds      0, 1, 2, 3, 4
thresholds 0.80, 0.90, 0.95, 0.99
```

Each run builds with 10% of FP32 bytes, verifies full coverage, requires every
stored vector to be accepted, and compares every query against a brute-force
maximum cosine. A query with a true match above the threshold must be accepted.

## Fixtures

`fixture.rs` generates data from a seed. It never reads a blob.

- `clustered`: centers within 0.7 rad of one global mean, angular spread 0.05.
- `duplicates`: distinct vectors repeated five times.
- `uniform`: random unit vectors, the negative control.
- `edge`: basis vectors, repeats, and negated queries.

## Commands

```sh
cargo test --release
cargo test --release --test zero_fn
cargo test --release --features usearch
cargo clippy --all-targets -- -D warnings
cargo fmt --check
bash scripts/capi_test.sh
bash scripts/real_corpus.sh
```
