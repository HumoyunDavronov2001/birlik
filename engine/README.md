# Engine — libbirlik

The C++20 engine that computes a slice of model layers, with a C ABI ([`include/birlik/birlik.h`](include/birlik/birlik.h))
so that Rust, Kotlin and Swift can call it.

**Stage B0 (now):** the ABI is defined and validates its arguments; `bk_shard_open` returns `BK_ERR_NOT_IMPLEMENTED`.
**Stage B1:** shards are implemented on top of [llama.cpp](https://github.com/ggml-org/llama.cpp), added here as an
unmodified submodule plus a small `patches/` set (layer window, hidden-state input/output, external KV management).

```bash
cmake -S engine -B build/engine
cmake --build build/engine
ctest --test-dir build/engine --output-on-failure
```
