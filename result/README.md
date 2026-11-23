# Results — CAPCOVER v0.1.0

Host: macOS aarch64, 8 logical CPUs. All runs use seed 7 and a release build.

## Real corpora

`verify --full` reports `uncovered=0` on both 250,000-vector indexes. The
commands and outputs are committed in `verify-sift.txt` and `verify-glove.txt`.

| Report | Corpus | `d` | Budget | `k` | File bytes | Build | `phi` | FNR | FPR | Filter p50 | Baseline | Peak RSS |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `sift250-95.json` | SIFT | 128 | 2% | 3,034 | 2,559,732 | 3.07 s | 0.000 | 0 | 1.0 | 0.375 us | 17.67 ms | 266 MB |
| `sift250-99.json` | SIFT | 128 | 2% | 3,034 | 2,559,732 | 2.76 s | 0.000 | 0 | 1.0 | 1.291 us | 17.50 ms | 266 MB |
| `glove250-95.json` | GloVe 6B.50d | 50 | 5% | 7,424 | 2,499,904 | 1.14 s | 0.000 | 0 | 1.0 | 1.875 us | 4.97 ms | 118 MB |
| `glove250-99.json` | GloVe 6B.50d | 50 | 5% | 7,424 | 2,499,904 | 1.19 s | 0.000 | 0 | 1.0 | 5.083 us | 5.14 ms | 118 MB |
| `glove250-999.json` | GloVe 6B.50d | 50 | 5% | 7,424 | 2,499,904 | 1.17 s | 0.000 | 0 | 1.0 | 8.167 us | 5.03 ms | 118 MB |

GloVe has `tp=0` at `s=0.99` and `s=0.999`, so its zero FNR is vacuous: no
query had a true match there.

The real corpora accept every query at these budgets. FNR and uncovered vectors
are still zero. The cap cover pays only when the data has tight clusters.

`scripts/real_corpus.sh` downloads and prepares both corpora.

## USearch

| Report | Corpus | `n` | `d` | `phi` | FNR | FPR | Gate recall | Search recall | Speedup | Filter p50 | USearch p50 |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `usearch-clustered.json` | clustered | 20,000 | 384 | 0.400 | 0 | 0 | 1.000 | 0.998 | 1.213x | 8.208 us | 184.834 us |
| `usearch-sift20.json` | SIFT | 20,000 | 128 | 0.000 | 0 | 1.0 | 1.000 | 1.000 | 1.000x | 0.417 us | 80.625 us |

`fnr` is the gate false-negative rate. `search_recall` is the share of queries
with a true match (`cos >= s - 1e-12` against the stored vectors) whose
brute-force best key appears in the USearch top-k. The gate recall is 1.0 in
both runs.

## Synthetic

| Report | Fixture | n | `s` | Build | Filter p50 | Baseline |
|---|---|---:|---:|---:|---:|---:|
| `clustered20k-95.json` | clustered | 20,000 | 0.95 | 0.68 s | 8.125 us | 5.562 ms |
| `clustered20k-99.json` | clustered | 20,000 | 0.99 | 0.63 s | 8.333 us | 5.574 ms |
| `uniform5k-95.json` | uniform | 5,000 | 0.95 | 0.11 s | 0.792 us | 1.390 ms |

## C ABI

`capi.txt` is the output of `bash scripts/capi_test.sh`:

```
capi ok dim=8 n=64 k=14 decision=1 bitmap=1110 version=0.1.0
```

## Reproduce

```sh
bash scripts/real_corpus.sh
bash scripts/capi_test.sh
```

`result/cli-gate.txt` records the CLI harness output for `--version`, `selftest`, `gen`, `build`, `verify`, `check`, `info`, and `bench`.

USearch runs:

```sh
cargo build --release --features usearch
cargo run --release -- gen --kind clustered --n 20000 --dim 384 --seed 7 \
  --queries 1000 --clusters 32 --sigma 0.05 \
  --out target/real/usearch_v.f32 --queries-out target/real/usearch_q.f32
cargo run --release -- build --vectors target/real/usearch_v.f32 --dim 384 \
  --out target/real/usearch.ccv --budget 2% --seed 7
cargo run --release --features usearch -- bench --index target/real/usearch.ccv \
  --vectors target/real/usearch_v.f32 --queries target/real/usearch_q.f32 \
  --similarity 0.95 --backend usearch --top-k 10
```

SIFT20 USearch run:

```sh
head -c 10320000 target/real/sift_base.fvecs > target/real/sift20.fvecs
head -c 103200 target/real/sift_query.fvecs > target/real/sift_query200.fvecs
cargo run --release -- build --vectors target/real/sift20.fvecs \
  --out target/real/sift20.ccv --budget 2% --seed 7
cargo run --release --features usearch -- bench --index target/real/sift20.ccv \
  --vectors target/real/sift20.fvecs --queries target/real/sift_query200.fvecs \
  --similarity 0.95 --backend usearch --top-k 10
```
