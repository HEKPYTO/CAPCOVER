#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

real=target/real
mkdir -p "$real"

if [ ! -f "$real/sift_base.fvecs" ]; then
  curl -L --fail --proto '=https' --tlsv1.2 \
    -o "$real/sift.tar.gz" \
    https://ftp.irisa.fr/local/texmex/corpus/sift.tar.gz || \
  curl -L --fail --proto '=ftp' \
    -o "$real/sift.tar.gz" \
    ftp://ftp.irisa.fr/local/texmex/corpus/sift.tar.gz
  tar -xzf "$real/sift.tar.gz" -C "$real" \
    --no-same-owner --no-same-permissions
  mv "$real/sift/sift_base.fvecs" "$real/sift_base.fvecs"
  mv "$real/sift/sift_query.fvecs" "$real/sift_query.fvecs"
  rm -rf "$real/sift" "$real/sift.tar.gz"
fi

head -c 129000000 "$real/sift_base.fvecs" > "$real/sift_base250.fvecs"
head -c 516000 "$real/sift_query.fvecs" > "$real/sift_query1000.fvecs"

if [ ! -f "$real/glove.6B.50d.txt" ]; then
  curl -L --fail -o "$real/glove.6B.50d.txt" \
    https://huggingface.co/datasets/antokun/glove.6B.50d/resolve/main/glove.6B.50d.txt
fi

rustc -O scripts/text_to_f32.rs -o "$real/text_to_f32"
"$real/text_to_f32" "$real/glove.6B.50d.txt" 50 "$real/glove.f32"
head -c 50000000 "$real/glove.f32" > "$real/glove_base250.f32"
dd if="$real/glove.f32" of="$real/glove_query1000.f32" \
  bs=4 skip=12500000 count=50000 2>/dev/null

cargo build --release
bin=target/release/capcover

"$bin" build --vectors "$real/sift_base250.fvecs" \
  --out "$real/sift250.ccv" --budget 2% --seed 7
"$bin" verify --index "$real/sift250.ccv" \
  --vectors "$real/sift_base250.fvecs" --full
for s in 0.95 0.99; do
  "$bin" bench --index "$real/sift250.ccv" \
    --vectors "$real/sift_base250.fvecs" \
    --queries "$real/sift_query1000.fvecs" \
    --similarity "$s" --out "$real/sift250-$s.json"
done

"$bin" build --vectors "$real/glove_base250.f32" --dim 50 \
  --out "$real/glove250.ccv" --budget 5% --seed 7
"$bin" verify --index "$real/glove250.ccv" \
  --vectors "$real/glove_base250.f32" --full
for s in 0.95 0.99 0.999; do
  "$bin" bench --index "$real/glove250.ccv" \
    --vectors "$real/glove_base250.f32" \
    --queries "$real/glove_query1000.f32" \
    --similarity "$s" --out "$real/glove250-$s.json"
done

echo "reports in $real"
