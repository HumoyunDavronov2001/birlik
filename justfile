# Run every check that CI runs (Linux subset).
default: check

check: design protocol rust engine web

# Tokens for every platform, the contrast gate, and the design concept page.
design:
    python3 design/colors.py
    python3 design/build.py

protocol:
    cd protocol && buf lint

rust:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo test --workspace

# The engine on llama.cpp, its accuracy gate and the engine in its own process (needs Python with numpy).
engine:
    git submodule update --init
    cmake -S engine -B build/engine -DCMAKE_BUILD_TYPE=Release
    cmake --build build/engine --parallel
    ctest --test-dir build/engine --output-on-failure

# The accuracy gate on real weights: stories15M (27 MB) from Hugging Face.
engine-real:
    mkdir -p build/models
    test -f build/models/stories15M-q8_0.gguf || curl -sSfL -o build/models/stories15M-q8_0.gguf https://huggingface.co/ggml-org/models-moved/resolve/main/tinyllamas/stories15M-q8_0.gguf
    cmake -S engine -B build/engine -DBIRLIK_TEST_MODEL="$PWD/build/models/stories15M-q8_0.gguf"
    cmake --build build/engine --parallel
    ctest --test-dir build/engine --output-on-failure
    BIRLIK_ENGINE="$PWD/build/engine/birlik-engine" BIRLIK_TEST_MODEL="$PWD/build/models/stories15M-q8_0.gguf" cargo test -p birlik-engine-host --features engine-tests

web:
    cd web && pnpm install --frozen-lockfile && pnpm check && pnpm test && pnpm build-storybook && pnpm test:visual

# Describe this device the way the agent announces it to a cluster.
info:
    cargo run -q -p birlik-node -- info
