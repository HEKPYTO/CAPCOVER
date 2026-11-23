# Evidence — CAPCOVER v1.0.0

Host: macOS aarch64, 8 logical CPUs. All runs use seed 7 and a release build.

## Real corpora

`verify --full` reports `uncovered=0` on both 250,000-vector indexes.

| Report | Corpus | `d` | Budget | `k` | File bytes | Build | `phi` | FNR | FPR | Filter p50 | Baseline | Peak RSS |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `sift250-95.json` | SIFT | 128 | 2% | 3,034 | 2,559,732 | 3.07 s | 0.000 | 0 | 1.0 | 0.375 us | 17.67 ms | 266 MB |
| `sift250-99.json` | SIFT | 128 | 2% | 3,034 | 2,559,732 | 2.76 s | 0.000 | 0 | 1.0 | 1.291 us | 17.50 ms | 266 MB |
| `glove250-95.json` | GloVe 6B.50d | 50 | 5% | 7,424 | 2,499,904 | 1.14 s | 0.000 | 0 | 1.0 | 1.875 us | 4.97 ms | 118 MB |
| `glove250-99.json` | GloVe 6B.50d | 50 | 5% | 7,424 | 2,499,904 | 1.14 s | 0.000 | 0 | 1.0 | 5.083 us | 4.97 ms | 118 MB |
| `glove250-999.json` | GloVe 6B.50d | 50 | 5% | 7,424 | 2,499,904 | 1.14 s | 0.000 | 0 | 1.0 | 8.167 us | 4.97 ms | 118 MB |

The real corpora accept every query at these budgets. FNR and uncovered vectors
are still zero. The cap cover pays only when the data has tight clusters.

`scripts/real_corpus.sh` downloads and prepares both corpora.

## USearch

| Report | Corpus | `n` | `d` | `phi` | FNR | FPR | Gate recall | Search recall | Speedup | Filter p50 | USearch p50 |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `usearch-clustered.json` | clustered | 20,000 | 384 | 0.400 | 0 | 0 | 1.000 | 0.998 | 1.213x | 8.208 us | 184.834 us |
| `usearch-sift20.json` | SIFT | 20,000 | 128 | 0.000 | 0 | 1.0 | 1.000 | 1.000 | 1.000x | 0.417 us | 80.625 us |

`fnr` is the gate false-negative rate. `search_recall` is USearch top-10 recall
against brute force. The gate recall is 1.0 in both runs.

## C ABI

`capi.txt` is the output of `bash scripts/capi_test.sh`:

```
capi ok dim=8 n=64 k=14 decision=1 bitmap=1110 version=1.0.0
```

## Reproduce

```sh
bash scripts/real_corpus.sh
bash scripts/capi_test.sh
```

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
