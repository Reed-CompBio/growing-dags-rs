# artificial benchmark

runs on a large dataset that's mainly used to 'visually' see the performance of this Growing DAGs method.

the source of this data is from the original Growing DAGs codebase, with an unknown source.

```
RUST_LOG=info cargo run --release -- -k 100 folder bench/wnt
```