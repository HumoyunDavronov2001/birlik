# Engine — libomnarc

The C++20 engine that computes a slice of a model's layers, with a C ABI
([`include/omnarc/omnarc.h`](include/omnarc/omnarc.h)) so that Rust, Kotlin and Swift can call it. It is built on
[llama.cpp](https://github.com/ggml-org/llama.cpp): an unmodified submodule in `third_party/llama.cpp` plus the
small patch set in [`patches/`](patches/README.md), applied at build time.

## Windows of layers

A model is split into **windows** of consecutive layers. The repeating layers are numbered `0 … n − 1` and the
output head (output norm and the matrix that gives logits) is layer `n`, as in llama.cpp's `n_gpu_layers`; the whole
model is the window `[0, n + 1)`. A **shard** is one window loaded on one device:

- the shard that starts at layer 0 takes **token ids**, every other shard takes the **hidden state** of the layer
  before it;
- the shard with the output head gives **logits**, every other shard gives **hidden states**;
- only the window's weights are read from disk, and the KV cache holds the window's layers only.

prima.cpp's ring keeps the first layers and the output on the device that serves the request; here that device opens
two shards, `[0, a)` and the output head alone, `[n, n + 1)`, and the other devices hold the windows in between.

```
tokens → [0, a) → hidden → [a, b) → hidden → … → [z, n) → hidden → [n, n + 1) → logits
```

`om_shard_prefetch` starts reading a window's weights from disk in the background (prima.cpp's prefetching), so a
device whose windows do not all fit in memory reads the next one while the rest of the ring computes.

## The accuracy gate

`omnarc-gate` runs a text through the whole model and through the windows in turn, rounding the hidden state to the
wire format (`f32`, `f16`, `bf16`) between windows, and compares the two runs: mean KL divergence of the next-token
distributions ≤ 0.01 and perplexity within 0.5% (PLAN.md §4). On the CPU with an `f32` wire, the split run is bit for
bit the whole run.

```bash
omnarc-gate --model model.gguf --split 0:40,40:80,80:81 --text text.txt --wire f16
omnarc-gate --model model.gguf --split even:4 --text text.txt   # 4 equal windows and the output head alone
```

## The engine in its own process

On computers and servers `omnarc-node` runs the engine as a separate process, `omnarc-engine`, and talks to it over
the process's stdin and stdout with the bulk data in shared memory ([`HOST.md`](HOST.md)): if the engine crashes,
the agent keeps running. The Rust side is [`node/crates/omnarc-engine-host`](../node/crates/omnarc-engine-host).
On phones the engine runs inside the app, because iOS does not allow child processes.

## Build and test

```bash
git submodule update --init                       # llama.cpp
cmake -S engine -B build/engine -DCMAKE_BUILD_TYPE=Release
cmake --build build/engine --parallel 4             # one job per 2 GB of memory; a bare --parallel runs make -j without a limit
ctest --test-dir build/engine --output-on-failure
```

Backends are llama.cpp's own options: `-DGGML_CUDA=ON`, `-DGGML_VULKAN=ON`, `-DGGML_HIP=ON`; Metal is on by default
on Apple. The tests write tiny random-weight models of every supported architecture with llama.cpp's `gguf-py`
(Python with `numpy`), and with `-DOMNARC_TEST_MODEL=stories15M-q8_0.gguf` they also run the gate on real weights.
