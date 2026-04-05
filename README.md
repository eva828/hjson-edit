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
use hjson_edit::{HjsonDocument, HjsonDocumentObjectEdit};

let input = r#"{
    // Configuration
    name: "app",
    port: 8080
}"#;

let document = HjsonDocument::parse(input)?;

let edited = document.set("name", "api")?;
assert!(edited.to_string().contains("name: api"));
```

## 設計

詳細な設計・アーキテクチャは `docs/ARCHITECTURE.md` を参照してください。

- `parse`: lossless tree を作る
- `set` / `push` / `insert_at` などの高レベルAPIで構造編集する
- `comment_set` / `comment_append` / `comment_get` でコメントを操作する
- `format`: 全体整形または部分整形を独立実行できる

### 設計改善の現状

反映済みの改善:

- `docs/ARCHITECTURE.md` を追加し、モジュール責務・データフロー・設計方針を明文化
- CLI 実装を `main` から分離し、起動処理と引数処理の責務を整理
- エラー定義を整理し、実装の重複を削減
- 差分計算で不要な文字列再構築を減らし、マルチバイト文字を含む行計算の安全性を改善
- 整形とラウンドトリップの安定性を確認する追加テストを導入

進行中の改善:

- `high_level` 周辺の責務分割
- `format` エンジンのモジュール分割
- 公開 API のドキュメント拡充

## Hjsonメモ

### Multiline string

```hjson
message:
  '''
  first line
    indented line
  '''
```

- 先頭行の空白は無視される
- closing `'''` の列までのインデントが各行から取り除かれる
- 末尾の改行は値に含まれない

この例の値は `first line\n  indented line` として解釈される。

### Quoteless string

```hjson
duration: 7 # minutes
windowsPath: C:\Users\eva828\Documents\Develop
```

- `7 # minutes` は `7` + コメントとして解釈される
- Windows path のようなバックスラッシュをエスケープなしで書ける
- `#` を含む正規表現風文字列の完全対応は未完で、現状は inline hash comment との両立を優先している
