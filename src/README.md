# src

| Module | Role |
|---|---|
| `error.rs` | Error type and `Result` alias |
| `format.rs` | `.ccv` header, layout, angle units, checksum |
| `vector.rs` | f32/fvecs loading, validation, normalization, cosine |
| `rng.rs` | Seeded SplitMix64 and normal samples |
| `build.rs` | Cap budget, spherical tree, angle measurement, save |
| `index.rs` | mmap open, validation, query, coverage |
| `query.rs` | Similarity to expansion angle |
| `verify.rs` | Verification report |
| `fixture.rs` | Deterministic clustered, duplicate, uniform, edge data |
| `bench.rs` | Ground truth, metrics, JSON |
| `capi.rs` | C ABI over the mmap index |
| `usearch.rs` | Optional USearch gate and top-k bench |
| `main.rs` | `build`, `check`, `verify`, `bench`, `info`, `selftest`, `gen` |

## Build

The cap budget is solved before construction.

```
available = budget - 256 - 4n - 8
k = clamp(available / (4d + 2), 1, n)
```

The eight spare bytes absorb the angle-section padding. `build` then checks the
exact file size against the budget and fails if it does not fit.

The tree starts with one leaf holding every index. It repeatedly splits the
largest leaf with at least two members. Each split:

1. Picks a seeded random member.
2. Finds the farthest member from it, then the farthest from that member.
3. Runs four nearest-direction assignment passes.
4. Repairs an empty side by moving the worst-assigned member.
5. Recenters on the normalized mean.

The final leaf center is the normalized mean of its members. The leaf angle is
the maximum angle from that center to a member. Every vector belongs to one
leaf, so coverage is true before any query.

## Format

The index is one little-endian mmap file.

| Offset | Size | Field |
|---:|---:|---|
| 0 | 4 | magic `CCV1` |
| 4 | 2 | version, currently 1 |
| 6 | 2 | header length, 256 |
| 8 | 4 | dimension `d` |
| 12 | 4 | cap count `k` |
| 16 | 8 | vector count `n` |
| 24 | 8 | build seed |
| 32 | 8 | budget bytes |
| 40 | 8 | file bytes |
| 48 | 4 | encoding, 0 = f32 directions |
| 52 | 4 | flags |
| 56 | 8 | xxh3-64 checksum of the payload |
| 64 | 192 | reserved, zero |
| 256 | `4 * k * d` | cap directions, f32 |
| 256 + `4 * k * d` | `2 * k` | cap angles, u16 |
| next 4-byte boundary | 0 to 3 | zero padding |
| next | `4 * n` | per-vector cap assignment, u32 |

Angles use units of `pi / 65535` and are rounded up. The checksum covers every
byte after the header. `open` validates the magic, version, dimensions, section
sizes, checksum, direction norms, and assignment bounds before serving a query.

Do not modify or truncate an open `.ccv` in place. Replace the path atomically;
`build` writes a temp file and renames it. Existing mappings keep the old inode.

## API

```rust
use capcover::{build, BuildConfig, CapCover, Vectors};
use std::path::Path;

let vectors = Vectors::load(Path::new("vectors.f32"), Some(384))?;
let built = build(&vectors, &BuildConfig { budget_bytes: 2_460_000, seed: 7 })?;
built.save(Path::new("cap.ccv"))?;

let index = CapCover::open(Path::new("cap.ccv"))?;
let decision = index.may_contain(&query, 0.95)?;
```

`may_contain_batch` and `verify` are the batch and audit paths. `Vectors` is
constructed by the loaders or `Vectors::from_data`; it is not a public literal.

## C ABI

`include/capcover.h` exposes an opaque handle and the same decision path:

```c
CapCover *capcover_open(const char *path);
int capcover_may_contain(const CapCover *index, const float *query,
                         size_t len, float similarity);
```

`1` means true, `0` false, `-1` error. `capcover_check_bitmap` writes one byte
per query. `capcover_last_error` is thread-local and valid until the next
failing call on that thread. The handle must not be used after close or from two
threads at once. `scripts/capi_test.sh` compiles `tests/capi.c` against the
release shared library.

## USearch

`--features usearch` adds `bench --backend usearch --top-k K`. The gate runs
`may_contain` first, then searches only accepted queries. The report includes
gate FNR, gate recall, top-k recall against brute force, and gated QPS.

## Angle rounding

```
raw = ceil(theta / (pi / 65535))
theta_stored = raw * (pi / 65535)
```

The ceiling makes `theta_stored >= theta`. `raw = 65535` is decoded as exactly
`pi`. A downward rounding would shrink a cap and create a false negative.

## Query

```
alpha = acos(similarity)
accept if cos(q, u_i) / (||q|| ||u_i||) >= cos(min(theta_i + alpha, pi)) - 1e-12
```

The cosine is accumulated in `f64` from the stored `f32` values. The `pi` clamp
handles a cap whose expansion covers the sphere. The `1e-12` term only widens
acceptance. At `similarity = -1`, every query is accepted, because every cosine
is at least `-1`.

## Verify

`verify` checks the recorded cap for every vector. `verify --full` scans every
cap. Both use the same `f64` cosine and widening as the query path. An uncovered
vector is a hard failure.

## Determinism

- Seeded SplitMix64 drives split starts.
- Member order is the input order.
- Floating-point sums run in that order.
- No hash-map iteration and no unordered parallel reduction.
- The checksum covers the exact payload bytes.

## Errors

Loading rejects non-finite values, zero norms, dimension mismatches, truncated
files, bad magic, bad versions, bad section sizes, checksum mismatches,
non-unit cap directions, and out-of-range assignments. A query rejects a wrong
dimension, a non-finite component, a zero norm, or a similarity outside
[-1, 1].

## Complexity

| Path | Cost |
|---|---|
| Build | `O(n d log k)` |
| Query | `O(k d)` |
| Verify | `O(n d)` with assignment, `O(n k d)` with `--full` |
| File | `256 + 4kd + pad4(2k) + 4n` bytes |
