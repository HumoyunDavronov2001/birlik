# llama.cpp patches

The engine builds on [llama.cpp](https://github.com/ggml-org/llama.cpp) as an unmodified submodule
(`engine/third_party/llama.cpp`) plus the patches in this directory. CMake copies the parts of the submodule that
the library needs into the build directory and applies the patches there (`engine/cmake/llama.cmake`), so the
submodule itself never changes. Every patch is meant to go upstream as a pull request; the goal is to need none.

| Patch | What it adds |
|---|---|
| [`0001-llama-add-layer-windows.patch`](0001-llama-add-layer-windows.patch) | Layer windows: a model loads only the layers `[layer_begin, layer_end)` and a context computes a window of them, with hidden states in and out and a KV cache for the window's layers only. Architectures: llama, qwen3. |

KV cache management per request (dropping a request, truncating it after rejected tokens) needs no patch: it uses
llama.cpp's `llama_memory_seq_rm`.

## Changing a patch

The patches are kept as commits on top of the submodule's upstream commit and exported with `git format-patch`.

```bash
# a working clone with the patches as commits
upstream=$(git -C engine/third_party/llama.cpp rev-parse HEAD)
git clone engine/third_party/llama.cpp build/llama-dev
git -C build/llama-dev am "$PWD"/engine/patches/*.patch

# edit, then commit (amend the patch you are changing or add a new commit)
git -C build/llama-dev commit -a --amend

# build the engine against the working clone while you iterate
cmake -S engine -B build/engine-dev -DOMNARC_LLAMA_DIR="$PWD/build/llama-dev"

# export the series back here
rm engine/patches/*.patch
git -C build/llama-dev format-patch --zero-commit --no-signature -o "$PWD/engine/patches" "$upstream..HEAD"
```

## Moving to a newer llama.cpp

Update the submodule, re-apply the series with `git am` in a fresh working clone, resolve conflicts, export the
series again and run the accuracy gate (`ctest --test-dir build/engine`). The weekly CI job does the same and
reports a patch that no longer applies.
