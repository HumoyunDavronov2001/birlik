#!/usr/bin/env bash
# Builds prima.cpp at the commit BIRLIK studied (UPSTREAM.md) for the TPOT comparison of the stage B1 gate
# (bench/b1/README.md). prima.cpp needs ZeroMQ from the system (Ubuntu: sudo apt install libzmq3-dev; macOS:
# brew install zeromq); HiGHS, which its head device needs, is built here without root.
#
#   bench/b1/prima.sh            # CPU (and Metal on a Mac)
#   bench/b1/prima.sh cuda       # with CUDA
#
# The result is build/prima.cpp/llama-cli.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
commit=288df6a2cc2120387e12d6f40798f30cc525c9ea
highs_version=v1.15.1
src="$root/build/prima.cpp"
highs="$root/build/highs"
jobs=$(getconf _NPROCESSORS_ONLN)

if ! printf '#include <zmq.h>\nint main(void) { return 0; }\n' | cc -x c - -lzmq -o /dev/null 2>/dev/null; then
  echo "prima.cpp needs ZeroMQ: sudo apt install libzmq3-dev (Ubuntu), brew install zeromq (macOS)" >&2
  exit 1
fi

if [ ! -f "$highs/lib/libhighs.a" ]; then
  rm -rf "$root/build/highs-src"
  git clone -q --depth 1 --branch "$highs_version" https://github.com/ERGO-Code/HiGHS.git "$root/build/highs-src"
  cmake -S "$root/build/highs-src" -B "$root/build/highs-src/build" -DCMAKE_BUILD_TYPE=Release \
    -DBUILD_SHARED_LIBS=OFF -DZLIB=OFF -DBUILD_TESTING=OFF -DCMAKE_INSTALL_PREFIX="$highs" > /dev/null
  cmake --build "$root/build/highs-src/build" --parallel "$jobs"
  cmake --install "$root/build/highs-src/build" > /dev/null
  [ -f "$highs/lib/libhighs.a" ] || { echo "HiGHS did not install to $highs" >&2; exit 1; }
fi

if [ ! -d "$src/.git" ]; then
  git clone -q https://github.com/OpenCPIL/prima.cpp.git "$src"
fi
git -C "$src" fetch -q --depth 1 origin "$commit"
git -C "$src" checkout -q "$commit"

cuda=()
[ "${1:-}" = cuda ] && cuda=(GGML_CUDA=1)
# HiGHS is linked statically from build/highs; the variables override the Makefile's /usr/local paths
make -C "$src" -j"$jobs" USE_HIGHS=1 "${cuda[@]}" \
  HIGHS_CPPFLAGS="-isystem $highs/include/highs" HIGHS_LDFLAGS="-L$highs/lib -lhighs" llama-cli
echo "built $src/llama-cli (prima.cpp ${commit:0:12})"
