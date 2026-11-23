# CAPCOVER

![version 0.1.0](https://img.shields.io/badge/version-0.1.0-blue) ![zero FN](https://img.shields.io/badge/zero--FN-passing-success) ![CI green](https://img.shields.io/badge/CI-green-success)

Zero-false-negative semantic pre-filter for cosine vector search.

`capcover` answers one question before an ANN search: can any indexed vector be
within cosine similarity `s` of this query? A `false` is safe, so the caller
skips the search. A `true` means the search may still return nothing.

## Guarantee

- The index covers every stored vector with at least one spherical cap.
- `may_contain` accepts every true match for every `s` in [-1, 1].
- Cap angles are measured on the stored direction and rounded upward.
- `verify` re-checks coverage against the stored vectors.
- The same input, budget, and seed produce an identical `.ccv` on the same host and toolchain.

The guarantee is geometric. If a stored vector `x` has `cos(q, x) >= s`, then
`angle(q, x) <= alpha = acos(s)`. Its cap direction `u` satisfies
`angle(q, u) <= alpha + theta` by the triangle inequality. The query test uses
that bound. The stored `theta` is at least the measured cap angle, so the bound
holds after angle quantization.

## Quick start

Requires Rust 1.85 or newer.

```sh
cargo build --release
cargo run --release -- selftest

cargo run --release -- gen --kind clustered --n 20000 --dim 384 --seed 7 \
  --queries 1000 --clusters 32 --sigma 0.05 \
  --out vectors.f32 --queries-out queries.f32

cargo run --release -- build --vectors vectors.f32 --dim 384 \
  --out cap.ccv --budget 2% --seed 7

cargo run --release -- verify --index cap.ccv --vectors vectors.f32 --full

cargo run --release -- check --index cap.ccv --queries queries.f32 \
  --similarity 0.95

cargo run --release -- bench --index cap.ccv --vectors vectors.f32 \
  --queries queries.f32 --similarity 0.95 --out report.json

cargo run --release -- info --index cap.ccv
```

`bench` needs the indexed vectors to compute ground truth and FNR. `--backend
brute` measures the in-process scan. `--backend usearch` builds a USearch HNSW
index and measures the gated search; it requires `--features usearch`.

## Measured result

Synthetic fixture: 20,000 vectors in 384 dimensions, seed 7. Cluster centers are
`normalize(global + 0.7*N(0,1))` per component from one global unit vector;
stored vectors are `normalize(center + (sigma/sqrt(dim))*N(0,1))`. The Quick
start uses `sigma=0.05`, 32 centers, and 1,000 queries: 3 in 5 near-cluster and
2 in 5 uniform. Budget 2% of FP32 bytes. Hardware: macOS aarch64, 8 logical
CPUs. Baseline is the measured brute-force scan in the same run. Build is the
fresh in-memory rebuild reported by `bench`.

| Fixture | n | `s` | `k` | File bytes | Build | `phi` | FNR | FPR | Filter p50 | Baseline | Speedup |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| clustered | 20,000 | 0.95 | 347 | 613,944 | 0.68 s | 0.400 | 0 | 0 | 8.125 us | 5.562 ms | 1.646x |
| clustered | 20,000 | 0.99 | 347 | 613,944 | 0.63 s | 0.400 | 0 | 0 | 8.333 us | 5.574 ms | 1.646x |
| uniform control | 5,000 | 0.95 | 86 | 152,524 | 0.11 s | 0.000 | 0 | 1.0 | 0.792 us | 1.390 ms | 0.999x |

Reports: `result/clustered20k-95.json`, `result/clustered20k-99.json`, and
`result/uniform5k-95.json`.

Real corpora, 250,000 vectors each, seed 7:

| Corpus | `d` | `s` | Budget | `k` | File bytes | Build | `phi` | FNR | FPR | Filter p50 | Baseline | Peak RSS |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| SIFT | 128 | 0.95 | 2% | 3,034 | 2,559,732 | 3.07 s | 0.000 | 0 | 1.0 | 0.375 us | 17.67 ms | 266 MB |
| GloVe 6B.50d | 50 | 0.95 | 5% | 7,424 | 2,499,904 | 1.14 s | 0.000 | 0 | 1.0 | 1.875 us | 4.97 ms | 118 MB |

`verify --full` reports zero uncovered vectors on both; the committed outputs
are in `result/verify-sift.txt` and `result/verify-glove.txt`. SIFT at
`s=0.99` and GloVe at `s=0.99` and `s=0.999` also give zero FNR, zero uncovered,
and `phi=0`. GloVe has `tp=0` at both higher thresholds, so its FNR is vacuous:
no query had a true match. This is the honest limit: unstructured embeddings do
not form tight caps at 2-5% memory, so the filter accepts every query. The
zero-FN guarantee still holds; clustered data is where the filter pays.

`phi` is the share of searches skipped. `fnr` must be zero. The uniform control
is the honest negative result: when data covers the sphere, the filter cannot
reject, and it costs one scan. Timings are one release run on the stated host;
build and latency vary by about 10 percent across runs.

Reproduce with the Quick start commands and the uniform control:

```sh
cargo run --release -- gen --kind uniform --n 5000 --dim 384 --seed 7 \
  --queries 500 --out uv.f32 --queries-out uq.f32
cargo run --release -- build --vectors uv.f32 --dim 384 \
  --out u.ccv --budget 2% --seed 7
cargo run --release -- bench --index u.ccv --vectors uv.f32 \
  --queries uq.f32 --similarity 0.95
```

## Format and API

The `.ccv` header, section offsets, angle units, checksum, and the Rust API are
specified in `src/README.md`. `check --format bitmap` emits one byte per query
for callers that cannot link Rust. The C ABI is `include/capcover.h`;
`scripts/capi_test.sh` builds the shared library and runs a C caller.

## Limits

- Static index, cosine only, f32 vectors normalized on load, `d <= 1024`, `n <= 250k`.
- f32 cap directions. PQ and sq8 encodings are not in v1.0.
- Uniform and unstructured real data give no benefit at these budgets. The
  control and real-corpus rows show it.
- 250k is measured on SIFT and GloVe; committed synthetic reports cover 20k clustered and 5k uniform.
- The `usearch` backend is feature-gated; the default build uses brute force.
- An open `.ccv` must not be modified or truncated in place. Replace it
  atomically; `build` does.
- No insert, delete, dynamic or incremental rebuild, GPU, or non-cosine metric.

Raw reports and exact reproduction commands are in `result/README.md`.

## Development

```sh
cargo test --release
cargo test --release --features usearch
cargo clippy --all-targets --features usearch -- -D warnings
cargo fmt --check
bash scripts/capi_test.sh
bash scripts/real_corpus.sh
```

## Documentation

| File | Contents |
|---|---|
| `src/README.md` | `.ccv` format, build, query, API, C ABI |
| `tests/README.md` | Fixtures, zero-FN matrix, commands |
| `result/README.md` | 250k and USearch reports |
| `scripts/README.md` | Corpus and C ABI scripts |
| `include/README.md` | C ABI contract |
| `examples/README.md` | Gate-then-search example and bitmap handoff |

## Licence

Apache-2.0. See `LICENSE`.
