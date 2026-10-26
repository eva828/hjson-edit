# hjson-edit

位置情報とコメントを保持しつつ、構造編集と整形を分離して扱える Rust 製 HJSON エディタ。

- **編集**: `set` / `remove` / コメント操作 / `merge_from` は最小限のテキスト変更だけを行う
- **検証**: 編集結果は毎回、再パース・値の一致・コメントの保存を検査する（崩れた結果はエラー）
- **整形**: 全体整形（`format`）と、指定したコンテナだけの部分整形（`format_container`）、
  1 要素だけの整形（`format_slot`）。`format` のインデントと改行は文書から推定する
- **CLI**: `edit` は既定で最小限の変更だけを行い、`--format` で変更したコンテナを整形する

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

# 編集して出力（体裁はそのまま）
hjson-edit edit config.hjson --set server.host localhost --set items[-] newValue

# 変更したコンテナを整形して出力
hjson-edit edit config.hjson --format --set server.host localhost
```

`edit` は既定で**最小限のテキスト変更**だけを行う。変更した行と挿入した行の字下げを揃え、
空行・インラインの `{...}` / `[...]`・既存の体裁はそのまま残す。
`--format` を付けると、変更したコンテナ（複数行に整形する）を文書の体裁に合わせて整形する。
`--indent-style` / `--indent-width` / `--newline` を指定した場合も整形を行う
（指定は整形対象にだけ作用する）。文書全体を整形する場合は `format` を使う。

```bash
# 入力 in.hjson が {a: 1} のとき
hjson-edit edit in.hjson --set b hello
# {a: 1, b: hello
# }

hjson-edit edit in.hjson --format --set b hello
# {
#   a: 1
#   b: hello
# }

# 配列要素にコメントを付ける（`:before` は要素の前の行、`:inline` は値の後ろ）
hjson-edit edit in.hjson --comment-set 'items[0]:inline' '先頭の要素'

# コンテナの閉じ区切りの直前（文書全体の footer は `:footer`）
hjson-edit edit in.hjson --comment-set 'server:footer' 'TODO: タイムアウトを見直す'

# 値は HJSON の値として解釈される
hjson-edit edit in.hjson --set name '"true"'   # 文字列 "true"
hjson-edit edit in.hjson --set flag true       # 真偽値
hjson-edit edit in.hjson --set items '[1, 2]'  # 配列
```

| コマンド | 主なオプション |
|---|---|
| `parse <file>` | `--ast`, `--show-trivia`, `--simple`, `--color <always\|auto\|never>` |
| `format <file>` | `--indent-style <spaces\|tabs>`, `--indent-width <1-8>`, `--newline <lf\|crlf>`（未指定の項目は文書から推定） |
| `edit <file>` | `--set <PATH> <VALUE>`, `--remove <PATH>`, `--comment-set` / `--comment-append` / `--comment-remove`, `--comment-style <line\|hash\|block>`, `--string-style <preserve\|double\|block\|unquoted>`, `--format`, `--no-verify`, `--in-place`, `--output <file>`, `--indent-style`, `--indent-width`, `--newline` |

`PATH` は `key`、`server.host`、`items[0]`、`items[-]` / `items[+]`（配列の末尾）、
`items[+2]`（0 始まりの index 2 へ挿入）の形式。`--set` は中間のコンテナが無ければ作成する
（`{}` に `--set a.b 1` で `a` と `b` を作る）。中間がスカラーの場合はエラーになる。

`VALUE` は **HJSON の値**として解釈する（文書と同じ文法）。`hello` は引用符なし文字列、
`8080` は数値、`true` / `null` はキーワード、`"true"` は文字列、`[1, 2]` は配列、
`{a: 1}` はオブジェクト、`"a\nb"` は `\n` を解釈した文字列になる。空のテキストはエラーなので、
空文字列を設定する場合は `""` を渡す。値テキストに引用符を書いた場合はその値だけに適用され、
それ以外は `--string-style` が使われる。

コメントの `PATH` は `key:before` / `key:inline`（要素の前の行／値の後ろ）に加えて
配列要素（`items[0]:inline`）も指定できる。`:header` / `:footer` は文書全体のコメント、
`obj:footer` はそのコンテナの閉じ区切りの直前のコメント（要素の後の行）になる。
文書直下のコンテナは `.` で指す（`.:footer` は `}` の直前、`:footer` は `}` の後ろ）。
引用符なし文字列は行末までを飲み込むため、その直後への `:inline` はエラーになる。

`--comment-style` と `--string-style` はその実行全体に適用する。
`--no-verify` は編集ごとの検証（再パース・値の一致・コメントの保存）を無効化する。

### ライブラリ

```rust
use hjson_edit::{HjsonDocument, HjsonDocumentEdit, HjsonObjectPath};

let input = r#"{
    // Configuration
    name: "app",
    port: 8080
}"#;

let document = HjsonDocument::parse(input).unwrap();

// 編集はテキストを最小限だけ変更する（既存の引用スタイルを保つ）。
// 編集結果は毎回、再パースと値の一致、コメントの保存が検査される。
let edited = document
    .set(HjsonObjectPath::parse("name").unwrap(), "api")
    .unwrap();
assert!(edited.to_string().contains("name: \"api\""));

// 変更したコンテナは `touched_containers` で分かる。
// 体裁を整えたいときは、そのコンテナだけを整形する（インデント・改行は文書から推定する）。
let config = edited.current_format_config();
let mut formatted = edited.clone();
for container in edited.touched_containers().to_vec() {
    formatted = formatted.format_container(&container, &config).unwrap();
}
assert_eq!(
    formatted.to_string(),
    "{\n    // Configuration\n    name: \"api\"\n    port: 8080\n}"
);

// スタイルと検証は文書ごとに設定できる
let document = HjsonDocument::parse(input)
    .unwrap()
    .string_style(hjson_edit::Prefer::Explicit(hjson_edit::ast::QuoteStyle::DoubleQuoted))
    .verify_edits(false);
assert!(document.to_string().contains("port: 8080"));
```

## 制限と挙動

| 項目 | 挙動 |
|---|---|
| 入力長 | `MAX_INPUT_BYTES`（`u32::MAX` バイト）まで。超過は `HjsonEditError::InputTooLong` |
| ネスト | `MAX_NESTING_DEPTH`（128）まで。パース時も編集時も `NestingTooDeep` を返す |
| 不正な入力 | 暗黙に補正せず、行・列付きのエラーとして返す（1 始まり。列は行頭からの文字数で、タブも全角も 1 文字。端末の表示幅とは一致しない） |
| 未終端のブロックコメント `/*` | 基準実装と同じく読み飛ばし、`warnings()` に記録する（CLI は stderr に警告を出す） |
| 重複キー | 値の層では後勝ち。lossless な木と `to_string()` は元のテキストを保持する |
| BOM | パーサーは通常の文字として扱い、CLI がファイル読み込み時に除去する |
| キーと `:` の間の空白 | lossless な木にトリビアとして残し、`to_string()` は元のテキストを再現する。`format` は値の前後の空白だけを正規化する |
| 編集結果の妥当性 | 行末でしか終端できない値（引用符なし文字列など）を挿入する場合は改行を補い、常に再パースできるテキストを返す |
| 編集の検証 | 各編集の後に再パース・編集した値と**文書全体の値**の一致（`HjsonValue` を基準にする）・対象外コメントの保存を検査し、崩れた結果は `HjsonEditError::InvalidEdit` として返す。整形（`format` / `format_container` / `format_slot`）も値を変えないことを検査する（`verify_edits(false)` で無効化） |
| 挿入位置 | 新しい要素は末尾の要素の後ろ、閉じ区切りの直前にあるコメント（footer）の手前に入る。字下げは既存の要素に揃える（空のコンテナではコンテナの行の字下げ＋1 段） |
| 複数行文字列の整形 | `'''` の桁が変わるときは本文の字下げも同じだけ動かし、値を変えない。1 行の `'''...'''` は複数行になるときだけ整形する |
| `edit` の既定 | 変更した行と挿入した行だけを整える。空行・インラインの `{...}` / `[...]`・既存の体裁はそのまま残す。`--format`（または整形オプションの指定）で変更したコンテナを整形する |
| 整形と空行 | `format` は空行を保つ（要素間・コメントの前後・閉じ区切りの前・文書末尾）。空行はレイアウトであり、整形の対象にしない |
| インライン位置のコンテナ | `--format` で整形するとき、対象が行頭になければ行頭になる祖先コンテナまで遡って整形する（親のレイアウトと混ざらないようにする） |
| 書けない値 | HJSON で表現できない値（NaN・無限大、指定スタイルで書けない文字列）は `HjsonEditError::ValueNotRepresentable`。`Prefer::Preserve` で書けない文字列は二重引用符へ切り替える |
| 配列の作成 | 途中のコンテナを作るとき、`[n]` / `[+n]` は `n = 0` だけを受け付ける（それ以外は `ArrayIndexOutOfBounds`） |
| `--set` の値のコメント | 値テキストにコメントを書くと、値に含まれないためエラーにする（引用符なし文字列は行末までを値として飲み込むので、コメントにはならない） |
| 引用符なし文字列への `:inline` | 値が行末までを飲み込むため付けられない。`HjsonEditError::InvalidTarget` を返す（`:before` か引用符付きの値を使う） |
| `merge_from` のコメント | 新規メンバーは先頭コメント行と行末コメントごとコピーする。文書全体のヘッダー/フッターは対象外 |
| `merge_from` の置き換え | 後勝ちで値だけを差し替え、キーとコメントはホスト側を保つ |
| 配列の追加 | `[-]` と `[+]` はどちらも末尾を指す（読み取りは末尾要素、書き込みは末尾への追加） |
| コンテナの footer | `<path>:footer` は閉じ区切りの直前のコメント。1 行で書かれたコンテナに付ける場合は行を分ける。文書直下は `.` で指定する（`.:footer`） |
| パスで区別できないキー | `a.b` のような引用符付きキーや重複キーはパスでもコメント位置の検証でも同じ位置として扱う（編集自体は成功する）。重複キーの整形はすべての出現に適用し、検証は値の層（後勝ち）で行う |

## hjson-cs 準拠

基準実装は [hjson/hjson-cs](https://github.com/hjson/hjson-cs)（`777ec1a`, v4.0.0-rc.1）とし、
**基準を実行して採取した期待値**と突き合わせるテストを持つ。

| 対象 | 件数 | 結果 |
|---|---|---|
| cs 固有ケース（オフライン） | 96 | 一致 93 / 逸脱 3（いずれも許容） |
| 公式テスト資産（`--features net-conformance`） | 78 | 逸脱 0 |
| 実運用コーパス（`tools/corpus_diff.sh`） | 542 | 逸脱 0（最終記録時。`corpus_deviations.tsv` は空） |
| 種固定の差分ファズ（`tools/diff_fuzz.sh`） | `SEED=20260101` / `COUNT=2000` | 逸脱 5（許容、`fuzz0132` / `fuzz0292` / `fuzz0316` / `fuzz0760` / `fuzz0901`） |

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
| `ml_root_indent` | 文書がカラム 0 の `'''` で始まり、本文に `:` を含む場合、基準は根の解釈やり直しの経路（ブレースレス解釈で `''' ... :` をキーとして読む）で 3 文字分のインデントを除去する。HJSON の仕様と hjson-js は `'''` のカラム（= 0）に従うため除去しない | 許容 |
| `fuzz0132` | 上記と同じ（カラム 0 の複数行文字列。本文に `:` を含む） | 許容 |
| `fuzz0292` | 上記と同じ（本文の `:` がトリガ） | 許容 |
| `fuzz0316` | 上記と同じ（本文の `:` がトリガ） | 許容 |
| `fuzz0760` | 上記と同じ（本文の `:` がトリガ） | 許容 |
| `fuzz0901` | 上記と同じ（本文の `:` がトリガ） | 許容 |

## 開発

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --features net-conformance --test conformance_assets -- --ignored
```

`tools/` には hjson-cs との突き合わせ用のスクリプトがある（`src/bin/canon.rs` は
`--features dev-tools --bin canon` でビルドする）。CI はネットワーク必須のテストを除いて
同じコマンドを実行する。

## ライセンス

MIT（`LICENSE` を参照）
