# scripts

| Script | Purpose |
|---|---|
| `capi_test.sh` | Build the shared library, compile `tests/capi.c`, and run the C ABI test |
| `real_corpus.sh` | Download SIFT and GloVe, build 250k indexes, verify, and bench |
| `text_to_f32.rs` | Convert whitespace text embeddings to little-endian f32 |

Run from the repository root:

```sh
bash scripts/capi_test.sh
```
