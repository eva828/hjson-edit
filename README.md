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
