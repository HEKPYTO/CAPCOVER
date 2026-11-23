#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

cargo build --release --quiet
dir="target/capi"
mkdir -p "$dir"

target/release/capcover gen --kind clustered --n 64 --dim 8 --seed 1 \
  --queries 4 --out "$dir/v.f32" --queries-out "$dir/q.f32"
target/release/capcover build --vectors "$dir/v.f32" --dim 8 \
  --out "$dir/c.ccv" --budget 50% --seed 1

cc -Iinclude tests/capi.c -Ltarget/release -lcapcover -o "$dir/capi_test"

if [ "$(uname -s)" = "Darwin" ]; then
  export DYLD_LIBRARY_PATH="$root/target/release${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}"
else
  export LD_LIBRARY_PATH="$root/target/release${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
fi

"$dir/capi_test" "$dir/c.ccv" "$dir/q.f32"
