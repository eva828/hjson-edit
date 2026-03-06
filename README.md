# hjson-edit

位置情報とコメントを保持したままHJSONを編集するためのRust製パーサー・エディタ。

## インストール

```bash
cargo install --git https://github.com/eva828/hjson-edit
```

## 使い方

### CLI

```bash
# パースして確認
hjson-edit parse config.hjson

# フォーマット保持して出力
hjson-edit format config.hjson
```

### ライブラリ

```rust
use hjson_edit::{parse_to_red, red_to_string};

let input = r#"{
    // Configuration
    name: "app",
    port: 8080
}"#;

let red = parse_to_red(input)?;
let output = red_to_string(&red);
assert_eq!(output, input);  // コメントと空白が保持される
```
