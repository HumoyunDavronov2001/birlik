# Parallel compile jobs: one per 2 GB of memory and at most one per core. A C++ or Rust compiler can take up to ~2 GB,
# and an unlimited `make -j` (what a bare `cmake --build --parallel` runs) freezes a machine with little memory.
# OMNARC_JOBS overrides it.
jobs := env_var_or_default("OMNARC_JOBS", `m=$(awk '/MemTotal/ { print int($2 / 2097152) }' /proc/meminfo 2>/dev/null || echo $(( $(sysctl -n hw.memsize) / 2147483648 ))); c=$(getconf _NPROCESSORS_ONLN); j=$(( m < c ? m : c )); echo $(( j < 1 ? 1 : j ))`)
export CARGO_BUILD_JOBS := jobs

# Run every check that CI runs (Linux subset).
default: check

check: design docs protocol rust simulate engine web

# Tokens for every platform, the contrast gate, and the design concept page.
design:
    python3 design/colors.py
    python3 design/build.py

# The count at the top of SPEC.md matches its tables.
docs:
    python3 .github/scripts/spec_count.py SPEC.md --check

protocol:
    cd protocol && buf lint

rust:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo test --workspace --exclude omnarc-simulation

# The cluster's agreed state through 1,000 simulated scenarios of crashes, partitions and lost messages (~10 min).
simulate:
    cargo test -p omnarc-simulation --release -- --nocapture

# The engine on llama.cpp, its accuracy gate and the engine in its own process (needs Python with numpy).
engine:
    git submodule update --init
    cmake -S engine -B build/engine -DCMAKE_BUILD_TYPE=Release
    cmake --build build/engine --parallel {{jobs}}
    ctest --test-dir build/engine --output-on-failure

# The accuracy gate on real weights: stories15M (27 MB) from Hugging Face.
engine-real:
    mkdir -p build/models
    test -f build/models/stories15M-q8_0.gguf || curl -sSfL -o build/models/stories15M-q8_0.gguf https://huggingface.co/ggml-org/models-moved/resolve/main/tinyllamas/stories15M-q8_0.gguf
    cmake -S engine -B build/engine -DOMNARC_TEST_MODEL="$PWD/build/models/stories15M-q8_0.gguf"
    cmake --build build/engine --parallel {{jobs}}
    ctest --test-dir build/engine --output-on-failure
    OMNARC_ENGINE="$PWD/build/engine/omnarc-engine" OMNARC_TEST_MODEL="$PWD/build/models/stories15M-q8_0.gguf" cargo test -p omnarc-engine-host --features engine-tests
    OMNARC_ENGINE="$PWD/build/engine/omnarc-engine" OMNARC_TEST_MODEL="$PWD/build/models/stories15M-q8_0.gguf" cargo test -p omnarc-node --features engine-tests

web:
    cd web && pnpm install --frozen-lockfile && pnpm check && pnpm test && pnpm build && pnpm build-storybook && pnpm test:visual

# Describe this device the way the agent announces it to a cluster.
info:
    cargo run -q -p omnarc-node -- info
