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

# ドキュメント全体を整形して出力
hjson-edit format config.hjson

# 編集後、変更したコンテナだけ整形して出力
hjson-edit edit config.hjson --set server.host localhost --array-append items newValue
```

### ライブラリ

```rust
use hjson_edit::{FormatTarget, HjsonDocument};

let input = r#"{
    // Configuration
    name: "app",
    port: 8080
}"#;

let document = HjsonDocument::parse(input)?;

let edited = document
    .edit_set("name", "api")?
    .format()?;

let fully_formatted = edited.format(vec![FormatTarget::Document])?;
assert!(fully_formatted.to_string().contains("name: api"));
```

## 設計

- `parse`: lossless tree を作る
- `edit_*`: 構造だけ変更し、整形対象を `EditResult` として返す
- `format`: 全体整形または部分整形を独立実行できる
- `set` / `push` / `insert_at` などの convenience API は内部で `edit -> format` を実行する

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
