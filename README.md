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
