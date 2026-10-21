#!/usr/bin/env bash
# レイヤ C: 実運用コーパス（TMLHonyaku）を hjson-cs と突き合わせる。
set -euo pipefail
cd "$(dirname "$0")/.."

REPO_URL=https://github.com/ExternalLocalizer/TMLHonyaku
COMMIT=26f3d22fc0df2815a81b9537d259c79f5d512bb9
DIR="${CORPUS_DIR:-/tmp/corpus-diff}"

echo "== コーパスを取得: $COMMIT"
if [ ! -d "$DIR/.git" ]; then
  mkdir -p "$DIR"
  git -C "$DIR" init -q
  git -C "$DIR" remote add origin "$REPO_URL"
fi
git -C "$DIR" fetch -q --depth 1 origin "$COMMIT"
git -C "$DIR" checkout -q FETCH_HEAD

tools/cs-oracle/build.sh > /dev/null
echo "== 基準の実行（hjson-cs）"
tools/cs-oracle/run.sh dir "$DIR" > /tmp/corpus-cs.tsv

echo "== 実装の実行"
cargo run --release --quiet --features dev-tools --bin canon -- dir "$DIR" > /tmp/corpus-ours.tsv

echo "== 比較"
python3 tools/compare_canon.py /tmp/corpus-cs.tsv /tmp/corpus-ours.tsv \
  tests/fixtures/hjson-cs/corpus_deviations.tsv || true
