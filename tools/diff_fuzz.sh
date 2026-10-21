#!/usr/bin/env bash
# レイヤ D: 種固定の生成入力で hjson-cs と突き合わせる。
set -euo pipefail
cd "$(dirname "$0")/.."

SEED="${SEED:-20260101}"
COUNT="${COUNT:-300}"

python3 tools/gen_fuzz_cases.py "$SEED" "$COUNT" > /tmp/fuzz-inputs.tsv
tools/cs-oracle/build.sh > /dev/null
echo "== 基準の実行（hjson-cs, seed=$SEED count=$COUNT）"
tools/cs-oracle/run.sh cases /tmp/fuzz-inputs.tsv > /tmp/fuzz-cs.tsv
echo "== 実装の実行"
cargo run --release --quiet --features dev-tools --bin canon -- cases /tmp/fuzz-inputs.tsv > /tmp/fuzz-ours.tsv
echo "== 比較"
python3 tools/compare_canon.py /tmp/fuzz-cs.tsv /tmp/fuzz-ours.tsv \
  tests/fixtures/hjson-cs/fuzz_deviations.tsv || true
