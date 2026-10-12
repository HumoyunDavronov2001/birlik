#!/usr/bin/env bash
# The accuracy gate of stage B1 (PLAN.md §4, §11) on the three gate models: every model is split three ways and each
# split is compared with the whole model on wikitext-2 (mean KL <= 0.01, perplexity within 0.5%). The hidden state
# travels as f16, as it does between devices.
#
#   bench/b1/gate.sh MODEL_DIR [BACKEND]      # BACKEND: auto (default), cpu, cuda, vulkan, metal
#
# MODEL_DIR holds the files listed in bench/b1/models.txt (bench/b1/download.sh fetches them; OMNARC_GATE_MODELS
# names another list). Results go to bench/b1/results/gate-<host>.txt. The whole
# model runs once per model and backend; its logits are kept in build/data (about 5 GB per model) for the splits.
set -euo pipefail

dir=${1:?usage: gate.sh MODEL_DIR [BACKEND]}
backend=${2:-auto}
chunks=${OMNARC_GATE_CHUNKS:-10}   # chunks of 512 wikitext-2 tokens; a 70B model on a laptop CPU takes minutes per chunk
root=$(cd "$(dirname "$0")/../.." && pwd)
gate="$root/build/engine/omnarc-gate"
out="$root/bench/b1/results"
mkdir -p "$out" "$root/build/data"

text="$root/build/data/wikitext-2-raw/wiki.test.raw"
if [ ! -f "$text" ]; then
  curl -sSfL -o "$root/build/data/wikitext-2-raw-v1.zip" \
    https://huggingface.co/datasets/ggml-org/ci/resolve/main/wikitext-2-raw-v1.zip
  (cd "$root/build/data" && unzip -q -o wikitext-2-raw-v1.zip)
fi

report="$out/gate-$(hostname).txt"
log="$root/build/data/gate-$(hostname).log"   # the engine's own messages, for when something fails
: > "$log"
{
  echo "# B1 accuracy gate on $(hostname), backend $backend, $chunks chunks of 512 tokens, $(date -u +%Y-%m-%dT%H:%MZ)"
  echo "# engine $(git -C "$root" rev-parse --short HEAD), llama.cpp $(git -C "$root/engine/third_party/llama.cpp" rev-parse --short HEAD)"
} > "$report"

status=0
while read -r repo file bytes sha256; do
  [[ -z "$repo" || "$repo" == \#* ]] && continue
  model="$dir/$file"
  if [ ! -f "$model" ]; then
    echo "missing $model: bench/b1/download.sh $dir" | tee -a "$report"
    status=1
    continue
  fi
  # the whole model runs once; its logits are kept and every split is compared with them
  name=$(basename "$file" .gguf)
  ref="$root/build/data/reference-$name-$backend-$chunks.bin"
  if [ ! -f "$ref" ]; then
    echo "== $file, the whole model on one device" | tee -a "$report"
    if ! "$gate" --model "$model" --split none --text "$text" --ctx 512 --chunks "$chunks" --backend "$backend" \
        --reference-out "$ref.part" 2>>"$log" | tee -a "$report"; then
      echo "failed; see $log" | tee -a "$report"
      status=1
      continue
    fi
    mv "$ref.part" "$ref"
  fi
  # two halves (the second with the output head); two and four windows with the output head alone, as on a ring
  for split in halves even:2 even:4; do
    args=()
    [ "$split" != halves ] && args=(--split "$split")
    echo "== $file, split $split" | tee -a "$report"
    if ! "$gate" --model "$model" "${args[@]}" --reference "$ref" --ctx 512 --wire f16 \
        --backend "$backend" 2>>"$log" | tee -a "$report"; then
      echo "failed; see $log" | tee -a "$report"
      status=1
    fi
  done
done < "${OMNARC_GATE_MODELS:-$root/bench/b1/models.txt}"
exit $status
