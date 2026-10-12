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
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

engine:
    cmake -S engine -B build/engine -DCMAKE_BUILD_TYPE=Release
    cmake --build build/engine
    ctest --test-dir build/engine --output-on-failure

web:
    cd web && pnpm install --frozen-lockfile && pnpm check && pnpm test && pnpm build-storybook && pnpm test:visual

# Describe this device the way the agent announces it to a cluster.
info:
    cargo run -q -p birlik-node -- info
