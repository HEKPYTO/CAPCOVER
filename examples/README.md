# examples

The filter returns a decision, never a result set. Gate the search, then run the
search only when the decision is `true`.

```rust
use capcover::CapCover;

fn search_if_possible(
    filter: &CapCover,
    query: &[f32],
    similarity: f32,
    search: impl FnOnce(&[f32]) -> Vec<u32>,
) -> capcover::Result<Vec<u32>> {
    if !filter.may_contain(query, similarity)? {
        return Ok(Vec::new());
    }
    Ok(search(query))
}
```

For a caller that cannot link Rust, emit a decision bitmap and pass it to the
backend as an ID selector.

```sh
capcover check --index cap.ccv --queries q.f32 --similarity 0.95 \
  --format bitmap --out decisions.bin
```

Each byte is `0` or `1` in query order. The caller skips a query when its byte is
`0`. A `0` is final. A `1` still needs the ANN search.
