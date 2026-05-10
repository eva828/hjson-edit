# 実行計画：high_level リファクタリング

## 方針

- 変更の少なさではなく、実装の綺麗さを重視する
- 破壊的な変更も積極的に行う
- 名前の変更時はエイリアスや薄いラッパーを作らず、利用側を変更に適用する形で実装する
- このプロジェクトは未公開で初期段階であるため、後方互換性を気にせず設計を矯正する

---

## やるべきこと

### [済] 1. スタイル enum の再設計（Preserve と実スタイルの分離）

**`StringStyle` を `Preserve` / `Explicit(QuoteStyle)` に分離する → `Prefer<QuoteStyle>` に統合**
- ~~現在: `StringStyle::Preserve`, `DoubleQuoted`, `Block`, `Unquoted`~~
- ~~変更後: `StringStyle::Preserve`, `StringStyle::Explicit(QuoteStyle)`~~
- 最終: `Prefer<T>` generic 型を新設。`Prefer<QuoteStyle>` に統合し、`StringStyle` は削除
- `resolve_quote_style` の match が簡潔になる（`Preserve => detected.unwrap_or(...)`、`Explicit(style) => style` のみ）
- `QuoteStyle`（`ast` 側）との重複が解消され、一貫性が向上する

**`CommentStyle` を `Preserve` / `Explicit(CommentKind)` に分離する → `Prefer<CommentKind>` に統合**
- ~~現在: `CommentStyle::Preserve`, `Line`, `Hash`, `Block`~~
- ~~変更後: `CommentStyle::Preserve`, `CommentStyle::Explicit(CommentKind)`~~
- 最終: `Prefer<CommentKind>` に統合し、`CommentStyle` は削除
- `resolve_comment_style` の戻り値は `CommentKind` に確定し、`render_comment_text` への曖昧な `Preserve` 渡しが解消する
- `CommentKind`（`ast` 側）との重複が解消される
- `render_comment_text` で `CommentStyle::Preserve | CommentStyle::Block` を同じ分支にしていた箇所を削除

---

### [済] 2. コメント操作の設計統一

**`comment.rs` のドキュメント更新パターンを `object.rs` と統一する**
- `set_document_comment` / `set_member_comment` が内部で `doc.root = ...` を書き換えるのをやめ、`GreenNode` を返す形にする
- `object.rs` と同じく `self.root(updated)` で更新する流れに一本化

**コメント操作のパス分岐をディスパッチャに集約する**
- `get_comment` / `set_comment` / `remove_comment` で同じ `match path { Header, Footer, Before, Inline }` が3回ある
- 1箇所のディスパッチャに集約し、各操作は location ごとの差分だけ実装する

**`extract_document_comment` と `extract_comment_text` を統合する**
- 両方とも「Trivia から Comment を抽出して join する」という同一ロジック
- `extract_comment_text` のみ `.rev()` していて一貫性がない
- 共通ヘルパに統合し、呼び出し側で順序を制御する

### 3. 値変換・レンダリングの重複解消

**`node_to_value` のトークン変換をヘルパ関数に抽出する**
- `SyntaxKind::Value` ブランチとトップレベルで、String/Integer/Float/Bool/Null の変換が2重にある
- `token_to_value(token: &RedToken)` ヘルパを作り、両方から呼ぶ

**`render_value_like_node` が `RenderConfig` を無視しているのを直す**
- `newline` や `depth` を `..RenderConfig::default()` で無視している
- 呼び出し元で適切な `RenderConfig` を渡すようにする

**`render_array_item_elements` と `render_object_member_elements` の重複を共通化する**
- 空コンテナ時と pretty 時のインデント/改行生成がほぼ同じ
- `RenderConfig` に吸収するか、共通の要素生成ヘルパを作る

### 4. set/remove 処理の構造的重複解消

**`apply_set_to_node` と `apply_remove_to_node` を高階関数で統合する**
- 両者は match 構造が完全一致。中身が `set_xxx` か `remove_xxx` かだけ
- `transform_node(node, last, |parent, seg| { ... })` の形に抽象化する

**`set_object_child` / `set_array_child` の空コンテナ挿入ロジックを共通化する**
- `range_start` の Whitespace 遡り調整がコピペされている
- 共通の範囲計算ヘルパを導入する

**`build_append_array_elements` の責務境界を明確化する**
- 空配列時の処理が `set_array_child` に残り、非空時だけ関数に委譲されている
- 関数に任せるか、そもそも解体して `set_array_child` に統合する

### 5. 未使用・過剰コードの削除

**`HjsonObjectPath` の変換経路を `parse()` のみに絞る**
- `TryFrom<&str>` / `FromStr` / `From<Vec>` / `From<&[PathSegment]>` を削除
- 公開 API は `HjsonObjectPath::parse(&str)` のみにする

**`DetectNewline for RedNode` を削除する**
- コードベース内で完全に未使用
- `HjsonDocument::current_newline()` に一本化する

**`SyntaxKind::create_node` を削除する**
- `rowan::GreenNode::new(...)` で直接作れる過剰なラッパー
- 使用箇所も `GreenNode::new` で代替可能

**`HjsonDocument::set` の早期 return を削除する**
- `get`（パス解決 + `node_to_value`）を毎回実行する重い最適化
- ほとんどのケースで無駄

**`set_object_child` / `set_array_child` の `resolve_insert_style` を共通化する**
- `match (string_style, detected) { (Preserve, None) => None, _ => Some(...) }` がコピペ
- `resolve_insert_style(container_ast, string_style)` ヘルパにする

**`find_mutable_child` と `normalized_start` を統合する**
- 両者とも Root/Value から実質子を取り出すロジック
- 1つのヘルパに集約する

---

## やらないこと（今回のスコープ外）

### AST 内部構造の大改修（cast 抽象化、`collect_container_*_trivia` 統合など）

**今やらない理由**
- AST 構造は現状で壊れておらず、今回の high_level 設計ブレとは別の問題
- `15df995` で公開 API が整理された直後であり、再び構造を変えると安定性を損なう

**将来必要な理由**
- 新しいコンテナ型（Set, Tuple など）を追加する際、Object/Array の実装が鏡像になっているため同じ修正を2箇所ずつ行う必要がある
- `collect_container_*_trivia` も delimiter スタイルや新しいコンテナ型を追加する際に修正箇所が増える

### `append_member_index_*` / `append_value_index_*` の統合

**今やらない理由**
- これは AST 側の責務であり、今回の high_level リファクタ（コメント操作の不統一、set/remove の重複など）とは独立している
- `Object` と `Array` にそれぞれ2系統ずつ存在するが、現状の high_level コードでは正しく動作している

**将来必要な理由**
- AppendMode の概念を AST 側で統一的に扱いたい場合、Object と Array で同じロジックが分散していると一貫性が保てない
- `Appendable` トレイトなどで抽象化すれば、将来のコンテナ型追加時に楽になる

### `current_newline()` / `syntax_root()` の可視性変更

**今やらない理由**
- CLI（`edit.rs`, `parse.rs`）とテストで両方とも使っている
- 可視性を下げると CLI やテストの修正が必要になり、今回のスコープを逸脱する

**将来必要な理由**
- ライブラリとして公開する際、内部構造である `RedNode` を `pub` API から排除する必要がある
- `syntax_root()` は rowan の型に依存しており、ライブラリユーザに露出させたくない
- CLI 側では `doc.to_string()` の結果から改行を検出するなど、内部状態への依存を減らすべき
