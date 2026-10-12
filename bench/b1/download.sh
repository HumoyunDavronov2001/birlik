#!/usr/bin/env bash
# Downloads the models of the stage B1 gate (bench/b1/models.txt) from Hugging Face. A broken download resumes where
# it stopped; every file is checked against its SHA-256 before it is used.
#
#   bench/b1/download.sh MODEL_DIR [8b|32b|70b|all]      # default: all (67 GB)
set -euo pipefail

dir=${1:?usage: download.sh MODEL_DIR [8b|32b|70b|all]}
which=${2:-all}
root=$(cd "$(dirname "$0")/../.." && pwd)
mkdir -p "$dir"

sha256() {
  if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

size() {
  if [ -f "$1" ]; then echo $(( $(wc -c < "$1") )); else echo 0; fi
}

free_bytes() {
  df -Pk "$1" | awk 'NR == 2 { print $4 * 1024 }'
}

selected() {
  case "$which" in
    all) return 0 ;;
    8b) [[ "$1" == *8B* ]] ;;
    32b) [[ "$1" == *32B* ]] ;;
    70b) [[ "$1" == *70B* ]] ;;
    *) echo "unknown model: $which (use 8b, 32b, 70b or all)" >&2; exit 2 ;;
  esac
}

# room for everything that is still missing
need=0
while read -r repo file bytes sha; do
  [[ -z "$repo" || "$repo" == \#* ]] && continue
  selected "$file" || continue
  need=$((need + bytes - $(size "$dir/$file")))
done < "${OMNARC_GATE_MODELS:-$root/bench/b1/models.txt}"
free=$(free_bytes "$dir")
if [ "$need" -gt "$free" ]; then
  echo "not enough disk space in $dir: $((need / 1000000000)) GB more needed, $((free / 1000000000)) GB free" >&2
  exit 1
fi

status=0
while read -r repo file bytes sha; do
  [[ -z "$repo" || "$repo" == \#* ]] && continue
  selected "$file" || continue
  target="$dir/$file"
  if [ "$(size "$target")" -eq "$bytes" ]; then
    if [ "$(sha256 "$target")" = "$sha" ]; then
      echo "ok       $file"
      continue
    fi
    echo "corrupt  $file: downloading it again"
    rm -f "$target"
  fi
  echo "fetching $file ($((bytes / 1000000000)) GB) from huggingface.co/$repo"
  # -C - resumes a partial file; --retry rides out short network drops
  curl -fL -# --create-dirs --retry 20 --retry-delay 5 --retry-all-errors -C - -o "$target" \
    "https://huggingface.co/$repo/resolve/main/$file"
  if [ "$(sha256 "$target")" = "$sha" ]; then
    echo "ok       $file"
  else
    echo "BAD      $file: the SHA-256 does not match; delete it and run again" >&2
    status=1
  fi
done < "${OMNARC_GATE_MODELS:-$root/bench/b1/models.txt}"
exit $status
