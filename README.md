# hjson-edit

位置情報とコメントを保持しつつ、構造編集と整形を分離して扱える Rust 製 HJSON エディタ。

- **編集**: `set` / `remove` / コメント操作は最小限のテキスト変更だけを行う
- **整形**: 全体整形（`format`）と、指定したコンテナだけの部分整形（`format_container`）
- **CLI**: `edit` は編集後、変更したコンテナだけを文書の体裁に合わせて整形して出力する

## インストール

```bash
cargo install --git https://github.com/eva828/hjson-edit
```

## 使い方

### CLI

```bash
# パースして確認
hjson-edit parse config.hjson

# 意味ASTを木表示（--show-trivia で空白・改行・コメントも表示）
hjson-edit parse --ast config.hjson

# ドキュメント全体を整形して出力
hjson-edit format config.hjson

# 編集後、変更したコンテナだけ整形して出力
hjson-edit edit config.hjson --set server.host localhost --set items[-] newValue
```

`edit` は変更したコンテナだけを整形する。インデントと改行は文書から推定し、
`--indent-style` / `--indent-width` / `--newline` で上書きできる。
これらのオプションは整形対象（変更したコンテナ）にだけ作用する。文書全体を整形する場合は
`format` を使う。ルート直下のキーを編集した場合は文書全体が整形対象になる。

```bash
# 入力 in.hjson が {a: 1} のとき
hjson-edit edit in.hjson --set b hello
# {
#   a: 1
#   b: hello
# }
```

| コマンド | 主なオプション |
|---|---|
| `parse <file>` | `--ast`, `--show-trivia`, `--simple`, `--color <always\|auto\|never>` |
| `format <file>` | `--indent-style <spaces\|tabs>`, `--indent-width <1-8>`, `--newline <lf\|crlf>` |
| `edit <file>` | `--set <PATH> <VALUE>`, `--remove <PATH>`, `--comment-set` / `--comment-append` / `--comment-remove`, `--comment-style <line\|hash\|block>`, `--string-style <preserve\|double\|block\|unquoted>`, `--in-place`, `--output <file>`, `--indent-style`, `--indent-width`, `--newline` |

`PATH` は `key`、`server.host`、`items[0]`、`items[-]`（配列の末尾へ追加）、
`items[+2]`（0 始まりの index 2 へ挿入）の形式。`--set` は中間のコンテナが無ければ作成する
（`{}` に `--set a.b 1` で `a` と `b` を作る）。中間がスカラーの場合はエラーになる。
`VALUE` は数値・`true` / `false` / `null`・`"..."` / `'''...'''`（引用スタイルを指定）・
それ以外は引用符なし文字列として扱う。`--string-style` は後続の `--set` に適用され、
値テキストで引用スタイルを指定した場合はそちらが優先される。

### ライブラリ

```rust
use hjson_edit::{HjsonDocument, HjsonDocumentObjectEdit, HjsonObjectPath};

let input = r#"{
    // Configuration
    name: "app",
    port: 8080
}"#;

let document = HjsonDocument::parse(input)?;

// 編集はテキストを最小限だけ変更する（既存の引用スタイルを保つ）
let edited = document.set(HjsonObjectPath::parse("name")?, "api")?;
assert!(edited.to_string().contains("name: \"api\""));

// 体裁を整えたいときは、変更したコンテナだけを整形する
// （インデント・改行は文書から推定する）
let root = HjsonObjectPath::parse("")?;
let formatted = edited.format_container(&root, &edited.detected_format_config())?;
assert_eq!(
    formatted.to_string(),
    "{\n    // Configuration\n    name: \"api\"\n    port: 8080\n}"
);
```

## 制限と挙動

| 項目 | 挙動 |
|---|---|
| 入力長 | `MAX_INPUT_BYTES`（`u32::MAX` バイト）まで。超過は `HjsonEditError::InputTooLong` |
| ネスト | `MAX_NESTING_DEPTH`（128）まで。パース時も編集時も `NestingTooDeep` を返す |
| 不正な入力 | 暗黙に補正せず、行・列付きのエラーとして返す |
| 未終端のブロックコメント `/*` | 基準実装と同じく読み飛ばし、`warnings()` に記録する（CLI は stderr に警告を出す） |
| 重複キー | 値の層では後勝ち。lossless な木と `to_string()` は元のテキストを保持する |
| BOM | パーサーは通常の文字として扱い、CLI がファイル読み込み時に除去する |
| 編集結果の妥当性 | 行末でしか終端できない値（引用符なし文字列など）を挿入する場合は改行を補い、常に再パースできるテキストを返す |

## hjson-cs 準拠

基準実装は [hjson/hjson-cs](https://github.com/hjson/hjson-cs)（`777ec1a`, v4.0.0-rc.1）とし、
**基準を実行して採取した期待値**と突き合わせるテストを持つ。

| 対象 | 件数 | 結果 |
|---|---|---|
| cs 固有ケース（オフライン） | 96 | 一致 93 / 逸脱 3（いずれも許容） |
| 公式テスト資産（`--features net-conformance`） | 78 | 逸脱 0 |
| 実運用コーパス（`tools/corpus_diff.sh`） | 542 | 逸脱 0（最終記録時。`corpus_deviations.tsv` は空） |
| 種固定の差分ファズ（`tools/diff_fuzz.sh`） | `SEED` / `COUNT` で指定 | 逸脱 1（許容、`fuzz0132`） |

```bash
# ケース（オフライン）: cs 固有ケース + 逸脱リストの整合検証
cargo test --test conformance_cs

# 公式テスト資産 78 件（ネットワーク必須）
cargo test --features net-conformance --test conformance_assets -- --ignored

# 実運用コーパス 542 件を基準と突き合わせる（ネットワークと .NET SDK が必要）
tools/corpus_diff.sh

# 種固定の差分ファズ
tools/diff_fuzz.sh
```

期待値の再生成は `tools/gen_fixtures.sh`（hjson-cs を固定コミットで取得して実行する）。
差分は `tests/fixtures/hjson-cs/deviations.tsv`（ケース）と
`asset_deviations.tsv` / `corpus_deviations.tsv` / `fuzz_deviations.tsv`（資産・コーパス・ファズ）に明示し、
**実装の実差分と一致しなければテストが失敗する**（無言の逸脱を許さない）。

現在許容している逸脱:

| 対象 | 内容 | 判断 |
|---|---|---|
| `comment_only` / `ws_only` | 空文書を空オブジェクトとして受理する（基準は `Incomplete input` でエラー） | 許容 |
| `ml_root_indent` | 文書がカラム 0 の `'''` で始まる場合、基準は根の解釈やり直しの経路で 3 文字分のインデントを除去する。HJSON の仕様と hjson-js は `'''` のカラム（= 0）に従うため除去しない | 許容 |
| `fuzz0132` | 上記と同じ（カラム 0 の複数行文字列） | 許容 |

## ライセンス

MIT（`LICENSE` を参照）
