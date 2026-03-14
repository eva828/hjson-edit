# アーキテクチャ概要

`hjson-edit` は、**コメント・空白・改行などのフォーマット情報を保持したまま HJSON を構造編集する**ための Rust ライブラリ / CLI です。  
設計の中心は、`rowan` を使った **lossless な構文木（Green / Red Tree）** と、**構造編集** と **整形** を分離した API にあります。

---

## 設計目標

- HJSON を **lossless** に扱う
  - コメント
  - 空白
  - 改行
  - 引用符なしキー / 値
  - 複数行文字列
- 編集時に **不要な全体再整形を避ける**
- 高レベル API では `path` ベースで簡潔に扱える
- CLI とライブラリで同じ編集エンジンを共有する
- 将来的な拡張に耐えるよう、**構文解析 / 構文表現 / 編集 / 整形** を分離する

---

## 全体構成

主要モジュールは次の通りです。

- `src/parser`
  - 字句解析 (`lexer.rs`)
  - 構文解析 (`parser.rs`)
- `src/syntax`
  - `SyntaxKind`
  - Green Tree / Red Tree ヘルパ
- `src/edit`
  - 低レベル編集 API (`api.rs`)
  - 高レベル編集 API (`high_level.rs`)
  - コメント編集 (`comments.rs`)
- `src/format`
  - 整形ターゲット定義
  - 整形エンジン (`engine.rs`)
- `src/lib.rs`
  - 公開 API の再エクスポート
- `src/main.rs`
  - CLI エントリポイント

---

## データフロー

基本的な処理の流れは以下です。

1. 入力文字列を字句解析する
2. トークン列から lossless な構文木を構築する
3. 高レベル API がパスを解決し、対象ノードを取得する
4. 編集処理は新しい Green Tree を再構築する
5. 編集結果として「どこを整形すべきか」を `EditResult` に保持する
6. 必要に応じて整形エンジンを呼び出す
7. `Display` 実装経由で文字列に戻す

概念図:

```/dev/null/architecture.txt#L1-17
input text
  ↓
lexer
  ↓
parser
  ↓
Green Tree
  ↓
Red Tree view
  ↓
high-level edit / low-level edit
  ↓
EditResult(dirty targets)
  ↓
formatter
  ↓
output text
```

---

## Green Tree / Red Tree の役割

### Green Tree

Green Tree は **不変** な構文木です。

- ノードとトークンの種類を持つ
- テキスト情報を保持する
- 親ポインタを持たない
- 再利用しやすい
- 編集時は既存ノードを破壊せず、新しい木を再構築する

このプロジェクトでは、フォーマット保持の中心が Green Tree にあります。

### Red Tree

Red Tree は Green Tree の上に構築される **参照ビュー** です。

- 親子関係をたどれる
- パス解決や局所的な探索に向く
- 編集ロジックや整形ロジックのナビゲーションに使う

実際の永続データは Green Tree 側が中心で、Red Tree は「探索用の見え方」として使います。

---

## パーサ層

### `src/parser/lexer.rs`

責務:

- 入力文字列をトークン列に分解する
- HJSON 特有の要素を識別する
  - 複数行文字列
  - 行コメント
  - ハッシュコメント
  - ブロックコメント
  - unquoted string
  - 数値
  - 真偽値 / null
- 行・列ベースのエラー位置を計算する

### `src/parser/parser.rs`

責務:

- トークン列から構文木を構築する
- `Object`, `Array`, `Pair`, `Value` などの構文ノードを作る
- recoverable なエラーを蓄積する
- lossless にトークンを木へ載せる

重要なポイント:

- 構文木は「意味だけ」ではなく「元の書式」も保持する
- エラーがあっても可能な範囲で継続し、利用者に複数エラーを返せるようにしている

---

## 構文表現層

### `src/syntax/kind.rs`

責務:

- `SyntaxKind` を定義する
- `rowan` と相互変換できるようにする

この列挙はシステム全体の土台です。  
編集や整形は `SyntaxKind` を前提に分岐します。

### `src/syntax/green.rs`

責務:

- Green Tree の構築補助
- Green Node / Green Element 操作用のユーティリティ

### `src/syntax/red.rs`

責務:

- Red Node / Red Token の型エイリアス
- 探索ヘルパ
  - `child_by_kind`
  - `token_by_kind`
  - `children_nodes`
  - `line_range`
- Red → Green 変換補助

---

## 編集層

編集層は **低レベル** と **高レベル** に分かれます。

### 低レベル編集 API (`src/edit/api.rs`)

責務:

- Green Node に対する直接編集
  - `replace_child`
  - `insert_child`
  - `remove_child`
  - `apply_edits`
- 編集操作の最小単位を提供する

この層は構造をよく理解した内部実装向けです。  
インデックスベースで動作するため柔軟ですが、利用者にはやや扱いづらいです。

### 高レベル編集 API (`src/edit/high_level.rs`)

責務:

- `HjsonDocument` を中心とした利用者向け API
- ドットパスや配列インデックスで対象を解決する
- 編集後に必要な整形対象を決める
- コメントや文字列スタイルも含めた高レベルな編集を提供する

代表的な API:

- `set`
- `set_with_style`
- `remove`
- `insert`
- `push`
- `insert_at`
- `remove_at`
- `comment_set`
- `comment_append`
- `comment_remove`

### コメント編集 (`src/edit/comments.rs`)

責務:

- ヘッダコメント / フッタコメント / キー直前コメント / インラインコメントを扱う
- コメントの配置ルールを壊さずに更新する

---

## `HjsonDocument` の位置づけ

`HjsonDocument` はこのライブラリの主要エントリです。

役割:

- 文字列からドキュメントを生成する
- Green Tree を保持する
- 高レベル編集 API を公開する
- 整形 API を公開する
- 最終的な文字列化を担当する

概念的には次のような責務分担です。

```/dev/null/hjson_document.txt#L1-11
HjsonDocument
- parse する
- Green Tree を保持する
- path を解決する
- 編集する
- どこを整形するべきかを決める
- 出力文字列へ戻す
```

---

## パスシステム

高レベル API は `PathSegment` の列で内部表現されます。

- `Key(String)`
- `Index(usize)`

利用者は主に次の形式を使えます。

- ドット記法
  - `server.host`
- 配列インデックス
  - `items[0]`
- 組み合わせ
  - `servers[1].host`

この抽象化により、編集 API は CLI / ライブラリどちらからも同じロジックを使えます。

---

## 編集と整形を分離している理由

このプロジェクトでは、**編集は構造を変えるだけ**、**整形は見た目を整えるだけ**、という方針を取っています。

### その理由

1. 変更範囲を最小化しやすい
2. 既存レイアウトを必要以上に壊さずに済む
3. 「まず編集だけ」「後でまとめて整形」ができる
4. API の責務が明確になる

### `EditResult`

高レベル編集メソッドはすぐに最終整形済み文字列を返すのではなく、内部的には `EditResult` を返せます。

`EditResult` は以下を持ちます。

- 編集後の `HjsonDocument`
- 整形が必要なターゲット一覧 (`dirty_targets`)

この設計により、必要最小限の整形だけを実行できます。

---

## 整形層

### `src/format/mod.rs`

責務:

- `FormatTarget` を定義する
- ターゲットの正規化を行う

整形対象は次の単位で指定できます。

- ドキュメント全体
- ノードパス
- ドットパス

### `src/format/engine.rs`

責務:

- ドキュメント全体または部分ノードを整形する
- 改行スタイルを推定する
- インデント単位を推定する
- `Object` / `Array` / `Pair` / `Value` ごとの整形を行う

整形エンジンは次のような情報を元に動きます。

- 既存の改行スタイル
  - `\n`
  - `\r\n`
- 既存のインデント幅
- 対象ノードの文脈
- コメントや blank line の位置

重要なのは、**常に全体を再整形するのではなく、部分整形にも対応している**ことです。

---

## Object / Array の内部モデル

高レベル編集と整形では、単純に子要素を並べ替えるのではなく、いったん中間表現へ落とします。

### Object 側

- `ObjectEntry`
- `StandaloneTriviaBlock`
- `ObjectItem`

### Array 側

- `ArrayEntry`
- `StandaloneTriviaBlock`
- `ArrayItem`

この中間モデルの目的:

- コメントをどの要素に紐づけるか整理する
- inline trivia と standalone trivia を区別する
- 改行 / カンマ / 末尾終端を明示的に扱う
- 再構築時に lossless 性を保ちやすくする

この層があることで、コメントや空行が多い入力でも編集・整形ロジックを比較的安定して実装できます。

---

## 文字列スタイル戦略

文字列は `StringStyle` によって出力形式を制御できます。

- `Preserve`
- `Auto`
- `DoubleQuoted`
- `Block`
- `Unquoted`

### 方針

- 既存スタイルを維持したいときは `Preserve`
- 文脈に応じて自動選択したいときは `Auto`
- 明示的に style を指定したい場合は固定スタイル

`Auto` では、以下のような観点で決定します。

- 改行を含むなら block string を優先
- 数値や予約語に見える文字列は unquoted を避ける
- HJSON として安全に unquoted にできる場合のみ unquoted を使う

---

## CLI 層

### `src/main.rs`

責務:

- コマンドライン引数を受け取る
- 入出力ファイルを扱う
- ライブラリ API を呼び出す

サブコマンド:

- `parse`
- `format`
- `edit`

CLI の役割はあくまで薄いシェルであり、コアロジックはライブラリ側に置く方針です。  
そのため、CLI とライブラリの挙動差を減らしやすくなっています。

---

## エラー設計

エラーは層ごとに分かれています。

- 字句 / 構文解析エラー
- 低レベル編集エラー
- 高レベル編集エラー
- 整形エラー

この分離により、利用者は「どの層で失敗したか」を理解しやすくなります。

理想的な責務:

- parser 層は位置付きの構文エラーを返す
- edit 層は path / index / node kind の不整合を返す
- format 層は不正ターゲットなどを返す
- CLI は最終的な表示責務だけを持つ

---

## 不変条件

このプロジェクトでは、いくつかの重要な不変条件があります。

1. Green Tree は破壊的変更しない
2. すべての編集は新しい木を返す
3. フォーマット情報は可能な限り保持する
4. 高レベル API は path 解決に失敗したら明示的にエラーを返す
5. 整形は構造を壊さず、見た目だけを調整する
6. コメントは可能な限り元の位置関係を維持する

---

## 設計上のトレードオフ

### 利点

- lossless 編集に強い
- コメント保持に強い
- 部分整形できる
- ライブラリ API と CLI が一貫している

### コスト

- 実装は一般的な JSON 編集器よりかなり複雑
- コメントや空行の扱いのため中間表現が必要
- 単純な文字列置換よりコストが高い
- `high_level` や `format` 周辺は責務が膨らみやすい

---

## 今後の改善ポイント

今後のリファクタ / 改善候補としては以下があります。

- `high_level.rs` の責務分割
  - path
  - value conversion
  - document API
  - container rebuilding
  - string style
- `format/engine.rs` の責務分割
  - object formatting
  - array formatting
  - indent/newline detection
- 差分計算の高速化
  - 不要な `String` 生成削減
  - オフセット計算の改善
- アーキテクチャに沿ったドキュメント拡充
  - 公開 API ごとの利用例
  - 編集・整形・コメント処理の詳細

---

## 典型的な利用例

### ライブラリ利用

```/dev/null/example.rs#L1-13
use hjson_edit::HjsonDocument;

let input = r#"
{
  name: app
}
"#;

let document = HjsonDocument::parse(input)?;
let updated = document.set("name", "api")?;
println!("{}", updated);
```

### CLI 利用

```/dev/null/example.sh#L1-3
hjson-edit format config.hjson
hjson-edit edit config.hjson --set server.host localhost
hjson-edit parse config.hjson
```

---

## まとめ

`hjson-edit` の設計は、単なる JSON 変換器ではなく、**HJSON の人間向けレイアウトを尊重した構造編集器** になっています。

中核となる考え方は次の 3 点です。

1. `rowan` による lossless tree
2. 編集と整形の分離
3. 高レベル API と局所整形による実用性

この方針により、コメントや空白を壊しにくく、かつプログラムから扱いやすい HJSON 編集基盤を実現しています。