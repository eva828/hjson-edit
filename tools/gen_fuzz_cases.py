#!/usr/bin/env python3
"""準拠差分ファズ用の入力生成器（種固定）。

使い方: gen_fuzz_cases.py [seed] [count] > 入力TSV
"""
import random
import sys

KEYS = ["a", "key", "k1", "true", "1", '"q k"', "'s k'", "a#b", "a//b", "a/*b*/c", "k:"]
VALUES = [
    "1", "0", "-0", "0.0", "5.", "1e2", "-3.5", "007", "true", "false", "null",
    "x", "x y", "x#y", "x//y", "x/*y", "5#c", "5//c", "5/*c*/", "a: b",
    '"dq"', "'sq'", '"a\\"b"', "'a\\'b'", '"\\u0041"', '"\\uD83D\\uDE00"', '"\\z"',
    "x[1]", "x{1}", "[]", "{}", "'''ml'''", "[1, 2]", "{b: 1}", "C:/*path", "**/*",
]
COMMENTS = ["# c", "// c", "/* c */", "/* unterminated"]
SEPS = [", ", " ", "\n", ",\n", ""]

def value(rnd: random.Random) -> str:
    v = rnd.choice(VALUES)
    if rnd.random() < 0.2:
        v = "'''\n  " + v.replace("'", "") + "\n  '''"
    return v

def member(rnd: random.Random) -> str:
    text = f"{rnd.choice(KEYS)}: {value(rnd)}"
    if rnd.random() < 0.25:
        text = f"{rnd.choice(KEYS)}:{rnd.choice(SEPS[:3])}{value(rnd)}"
    if rnd.random() < 0.2:
        text += " " + rnd.choice(COMMENTS)
    return text

def document(rnd: random.Random) -> str:
    kind = rnd.random()
    if kind < 0.30:  # ブレースレスのルート
        count = rnd.randint(1, 4)
        return "\n".join(member(rnd) for _ in range(count))
    if kind < 0.75:  # 中括弧つきオブジェクト
        count = rnd.randint(0, 4)
        body = rnd.choice(SEPS).join(member(rnd) for _ in range(count))
        trail = "," if count and rnd.random() < 0.2 else ""
        return "{" + rnd.choice(["\n", " ", ""]) + body + trail + rnd.choice(["\n", " ", ""]) + "}"
    if kind < 0.9:  # 配列
        count = rnd.randint(0, 4)
        body = rnd.choice(SEPS).join(value(rnd) for _ in range(count))
        return "[" + body + "]"
    # 単一値
    return value(rnd)

def main() -> None:
    seed = int(sys.argv[1]) if len(sys.argv) > 1 else 20260101
    count = int(sys.argv[2]) if len(sys.argv) > 2 else 300
    rnd = random.Random(seed)
    print("# gen_fuzz_cases.py の生成入力（種固定）")
    for index in range(count):
        text = document(rnd)
        escaped = (text.replace("\\", "\\\\").replace("\n", "\\n")
                       .replace("\r", "\\r").replace("\t", "\\t"))
        print(f"fuzz{index:04d}\t{escaped}\t")
    print(f"# seed={seed} count={count}", file=sys.stderr)

if __name__ == "__main__":
    main()
