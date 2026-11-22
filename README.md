# hjson-edit

位置情報とコメントを保持しつつ、構造編集と整形を分離して扱える Rust 製 HJSON エディタ。

- **編集**: `set` / `remove` / コメント操作 / `merge_from` は最小限のテキスト変更だけを行う
- **検証**: 編集結果は毎回、再パース・値の一致・コメントの保存を検査する（崩れた結果はエラー）
- **整形**: 全体整形（`format`）と、指定したコンテナ・要素だけの部分整形を分けて行える
- **CLI**: `edit` は既定で最小限の変更だけを行い、`--format` で変更したコンテナを整形する

## インストール

```bash
cargo install --git https://github.com/eva828/hjson-edit
```

## 使い方

```bash
hjson-edit parse config.hjson                                      # パースして確認（--ast, --show-trivia）
hjson-edit format config.hjson                                     # 文書全体を整形
hjson-edit edit config.hjson --set server.host localhost           # 編集（体裁はそのまま）
hjson-edit edit config.hjson --format --set server.host localhost  # 変更したコンテナも整形
hjson-edit edit in.hjson --comment-set 'server:footer' 'TODO: …'   # コメント操作
hjson-edit get in.hjson server.port                                # 値を JSON で読む
```

全オプションは `hjson-edit <command> --help`。

| コマンド | 内容 |
|---|---|
| `parse <file>` | パースして木／AST を表示（`--ast`, `--show-trivia`, `--simple`, `--color`） |
| `format <file>` | 文書全体を整形（`--indent-style`, `--indent-width`, `--newline`, `--in-place`, `--output`, `--no-verify`） |
| `edit <file>` | 値を編集・コメントを操作（`--set`, `--remove`, `--comment-*`, `--string-style`, `--format`, `--no-verify`, `--indent-style`, `--indent-width`, `--newline`, `--in-place`, `--output`） |
| `get <file> <PATH>` | パスの値を JSON で出力（`--raw`） |

`--set` の `VALUE` は HJSON の値として解釈する。`hello` は文字列、`8080` は数値、`"true"` は
文字列、`[1, 2]` は配列になる。ファイル名に `-` を渡すと標準入力から読む。
`--in-place` と `--output` は `edit` と `format` で使える。書き込みは一時ファイルを経て置き換えるので、
失敗しても元のファイルは壊れない。`edit` の出力は入力の末尾改行を踏襲する。

### PATH

| 記法 | 指す位置 |
|---|---|
| `key` / `server.host` | メンバーの値 |
| `items[0]` | 配列の要素 |
| `items[+2]` | 配列の index 2（書き込みは挿入） |
| `items[-]` / `items[+]` | 末尾（書き込みは追加） |
| （空文字列） | 文書全体 |

`[n]` の `n` は 0 始まり。コメントは要素のパスに接尾辞を付ける（`items[0]:inline` も可）。

| 記法 | 指す位置 |
|---|---|
| `server:before` | 要素の前の行（接尾辞なしも同じ） |
| `server:inline` | 値と同じ行の後ろ |
| `server:footer` | コンテナの閉じ区切りの直前 |
| `:header` / `:footer` | 文書全体の先頭 / 末尾 |

## ライブラリ

```rust
use hjson_edit::{HjsonDocument, HjsonDocumentEdit, HjsonObjectPath};

let input = r#"{
    // Configuration
    name: "app",
    port: 8080
}"#;

// 編集はテキストを最小限だけ変え、結果は再パース・値・コメントで検証される。
let edited = HjsonDocument::parse(input)
    .unwrap()
    .set(HjsonObjectPath::parse("name").unwrap(), "api")
    .unwrap();
assert!(edited.to_string().contains("name: \"api\""));

// 変更したコンテナだけを、文書から推定した体裁で整形する。
let config = edited.current_format_config();
let mut formatted = edited.clone();
for container in edited.touched_containers().to_vec() {
    formatted = formatted.format_container(&container, &config).unwrap();
}
assert_eq!(formatted.to_string(), "{\n    // Configuration\n    name: \"api\"\n    port: 8080\n}");
```

細部は `cargo doc --open` の rustdoc を参照する。`PATH` の制限、コメント位置の規則、整形の仕様、
検証、上限、エラーを説明している。

## 開発

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --doc
```

CI（`.github/workflows/ci.yml`）は同じコマンドを flake の devShell（`nix develop -c …`）で回す。
あわせて `cargo doc`（警告はエラー）・`cargo package`・`nix flake check`・`nix build` も検証する。
hjson-cs との準拠検証は 4 層ある。master への push と手動実行のときだけ別ジョブで回る
（`tools/*.sh` と `tests/fixtures/hjson-cs/`）。

## ライセンス

MIT（`LICENSE` を参照）
