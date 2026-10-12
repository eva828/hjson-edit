#!/usr/bin/env python3
"""正準形 TSV を突き合わせる（corpus_diff.sh / diff_fuzz.sh から使う）。

使い方: compare_canon.py <基準TSV> <実装TSV> [逸脱リスト]
- 行形式: id<TAB>状態(OK|ERR|ERRV)<TAB>値|メッセージ
- 逸脱リスト: id<TAB>判断<TAB>理由（許可された差分）
出力: 一致件数と、逸脱リストに無い差分の一覧。
"""
import sys

def load(path):
    rows = {}
    for line in open(path, encoding="utf-8"):
        line = line.rstrip("\n")
        if not line or line.startswith("#"):
            continue
        parts = line.split("\t")
        rows[parts[0]] = (parts[1] if len(parts) > 1 else "", parts[2] if len(parts) > 2 else "")
    return rows

def main():
    base, ours = load(sys.argv[1]), load(sys.argv[2])
    declared = set()
    if len(sys.argv) > 3:
        for line in open(sys.argv[3], encoding="utf-8"):
            line = line.rstrip("\n")
            if line and not line.startswith("#"):
                declared.add(line.split("\t")[0])

    same = 0
    diffs = []
    for key in sorted(set(base) | set(ours)):
        b, o = base.get(key), ours.get(key)
        if b is None or o is None:
            diffs.append((key, b, o))
            continue
        if b == o or (b[0] != "OK" and o[0] != "OK"):
            same += 1
            continue
        diffs.append((key, b, o))

    undeclared = [d for d in diffs if d[0] not in declared]
    print(f"対象 {len(set(base) | set(ours))} 件: 一致 {same} 件 / 宣言済みの逸脱 {len(diffs) - len(undeclared)} 件 / 未宣言の差分 {len(undeclared)} 件")
    for key, b, o in undeclared[:40]:
        print(f"  {key}\n      基準  : {b}\n      実装  : {o}")
    if len(undeclared) > 40:
        print(f"  ... 他 {len(undeclared) - 40} 件")
    return 1 if undeclared else 0

if __name__ == "__main__":
    sys.exit(main())
