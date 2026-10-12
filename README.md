# hjson-edit

位置情報とコメントを保持しつつ、構造編集と整形を分離して扱えるRust製HJSONエディタ。

## インストール

```bash
cargo install --git https://github.com/eva828/hjson-edit
```

## 使い方

### CLI

```bash
# パースして確認
hjson-edit parse config.hjson

# ASTを木表示
hjson-edit parse --ast config.hjson

# ドキュメント全体を整形して出力
hjson-edit format config.hjson

# 編集後、変更したコンテナだけ整形して出力
hjson-edit edit config.hjson --set server.host localhost --set items[-] newValue
```

### ライブラリ

```rust
use hjson_edit::{HjsonDocument, HjsonDocumentObjectEdit, HjsonObjectPath};

let input = r#"{
    // Configuration
    name: "app",
    port: 8080
}"#;

let document = HjsonDocument::parse(input)?;

let edited = document.set(HjsonObjectPath::parse("name")?, "api")?;
assert!(edited.to_string().contains("name: api"));
```

## 制限

- コンテナ（オブジェクト・配列）のネストは `MAX_NESTING_DEPTH`（128）まで。
  超過した入力はスタックオーバーフローを避けるため `HjsonEditError` を返す。
- 不正な入力は暗黙に補正せず、行・列付きのエラーとして返す。

## hjson-cs 準拠

基準実装は [hjson/hjson-cs](https://github.com/hjson/hjson-cs)（`777ec1a`）とし、
**基準を実行して採取した期待値**と突き合わせるテストを持つ。

```bash
# ケース（オフライン）: cs 固有 93 件 + 逸脱リストの整合検証
cargo test --test conformance_cs

# 公式テスト資産 78 件（ネットワーク必須）
cargo test --features net-conformance --test conformance_assets -- --ignored

# 実運用コーパス 542 件を基準と突き合わせる（ネットワークと .NET SDK）
tools/corpus_diff.sh

# 種固定の差分ファズ
tools/diff_fuzz.sh
```

期待値の再生成は `tools/gen_fixtures.sh`（hjson-cs を固定コミットで取得して実行する）。
差分は `tests/fixtures/hjson-cs/deviations.tsv`（ケース）と
`asset_deviations.tsv` / `corpus_deviations.tsv`（資産・コーパス）に明示し、
**実装の実差分と一致しなければテストが失敗する**（無言の逸脱を許さない）。

現在の逸脱:

| 対象 | 内容 | 判断 |
|---|---|---|
| 空文書（`comment_only` / `ws_only`） | 基準はエラー、実装は空オブジェクトとして受理 | 許容 |
| コーパス 3 件 | CRLF と複数行文字列の組み合わせでパースに失敗（要調査） | 修正予定 |

