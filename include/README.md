# include

`capcover.h` is the C ABI for callers that cannot link Rust.

| Function | Contract |
|---|---|
| `capcover_open` | Returns an opaque handle or `NULL` |
| `capcover_close` | Releases the handle |
| `capcover_dim`, `capcover_n`, `capcover_k`, `capcover_file_bytes` | Read-only shape and size |
| `capcover_may_contain` | `1` true, `0` false, `-1` error |
| `capcover_check_bitmap` | Writes one byte per query, `0` or `1`; returns `0` or `-1` |
| `capcover_version` | Static version string |
| `capcover_last_error` | Thread-local message; valid until the next failing call on the thread |

`capcover_dim`, `capcover_n`, `capcover_k`, and `capcover_file_bytes` return `0`
when passed a `NULL` handle.

`capcover_check_bitmap` rejects `count > 250000` with `-1`; the error message is
available through `capcover_last_error`.

The query buffer must contain `count * dim` floats in native host f32 order.
The output bitmap must hold `count` bytes. A handle must not be used after
`capcover_close`, and one handle must not be used from two threads at once.
The index file must not be modified or truncated in place while a handle is
open.

Build the shared library with `cargo build --release`. The output is
`target/release/libcapcover.dylib` on macOS or `target/release/libcapcover.so`
on Linux.
