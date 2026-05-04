## 優先度高: 仕様や出力の不整合につながりそうなもの

### 1. 改行コード管理が複数系統に分裂している

対象:

- `hjson-edit/src/high_level/mod.rs#L27-L33`
- `hjson-edit/src/high_level/mod.rs#L60-L66`
- `hjson-edit/src/high_level/mod.rs#L101-L103`
- `hjson-edit/src/high_level/style.rs#L7-L38`
- `hjson-edit/src/ast/mod.rs#L8-L29`
- `hjson-edit/src/high_level/ops/object.rs#L356`
- `hjson-edit/src/high_level/ops/comment.rs#L89-L90`
- `hjson-edit/src/high_level/ops/comment.rs#L242`

問題点:

- `ast::NewlineKind` と `high_level::style::NewlineKind` が別々に存在しています。
- `HjsonDocument` は `newline: String` を保持していますが、編集処理では `doc.current_newline()` ではなく、毎回 `parent.detect_newline()` や `current_root().detect_newline()` を使っています。
- CLI では最後に `doc.current_newline()` を使って末尾改行を補っていますが、編集ロジック本体は別ルートで改行検出しています。

影響:

- ある編集では「入力全体の改行」、別の編集では「編集対象ノード内で最初に見つかった改行」、また別の処理では「保存済み newline」を使うため、CRLF/LF の扱いがブレます。
- 部分木に改行がない場合、`detect_newline()` はデフォルト LF になりやすく、ドキュメント全体が CRLF でも挿入部分だけ LF になる可能性があります。

整理案:

- `NewlineKind` は `ast` か共通モジュールに一本化。
- `HjsonDocument` の `newline` を編集処理の唯一の基準にする。
- `DetectNewline for RedNode` は「初期解析時の fallback」用途に限定する。

---

### 2. `DetectNewline for &[GreenElement]` の実装が怪しい

対象:

- `hjson-edit/src/high_level/style.rs#L65-L79`

問題点:

- `filter(|e| ... != SyntaxKind::Newline)` となっており、`Newline` ではないトークンだけを残しています。
- その後で `NewlineKind::from_newline_text(token.text())` を呼んでいるため、通常は改行トークンを検出できません。

影響:

- この実装が使われると常にデフォルト改行になりやすいです。
- 現状 grep 上はほぼ使われていないように見えますが、残しておくと将来バグ源になります。

整理案:

- 条件を `== SyntaxKind::Newline` に直す。
- そもそも未使用なら削除。

---

### 3. `InsertAt` と `Push` の配列挿入処理が不統一

対象:

- `hjson-edit/src/high_level/ops/object.rs#L441-L558`

問題点:

- `ArrayPathSegment::InsertAt(idx)` の `idx >= items.len()` は append 相当として処理されています。
- `ArrayPathSegment::Push` も append 相当ですが、処理内容が別実装です。
- `InsertAt` 側は `pretty_nested`、`indent`、`array_ast.layout.depth + 1` を考慮しています。
- `Push` 側は空配列時に固定で `"  "` を使い、`pretty_nested` もほぼ考慮していません。
- `InsertAt` の中間挿入ではセパレータを入れず、テストも `"[1,2 3]"` のような不自然な出力を期待しています。

影響:

- `items[+10]` と `items[-]` が意味的には似ているのに、整形結果が変わる可能性があります。
- ネスト配列や CRLF の扱いでも差が出やすいです。

整理案:

- append 処理を `append_array_value(...)` のような関数に統合。
- `Push` は `InsertAt(items.len())` に正規化してから処理する。
- 中間挿入時の comma/space/newline 挿入方針を明確化する。

---

### 4. `resolve_parent` と `resolve_existing_prefix` の探索開始位置が違う

対象:

- `hjson-edit/src/high_level/ops/object.rs#L222-L233`
- `hjson-edit/src/high_level/ops/object.rs#L235-L253`
- `hjson-edit/src/high_level/ops/comment.rs#L147-L161`

問題点:

- `resolve_parent` は `root` から始めています。
- `resolve_existing_prefix` は `root.document_start_node()` から始めています。
- コメント側の `resolve_member` も `document_start_node()` から始めています。

影響:

- braceless root、root value、root object でパス解決の挙動が揺れる可能性があります。
- `RedNodeExt::child()` が再帰 fallback を持っているため、見た目上動いていても「直接の子を辿っている」のか「どこかの子孫を探索している」のか曖昧になります。

整理案:

- パス解決の入口を一本化する。
- `Root` は必ず `document_start_node()` に正規化してから辿る、などのルールを固定する。
- `walker` / `resolve_parent` / `resolve_existing_prefix` を統合する。

---

### 5. `RedNodeExt::child()` の再帰 fallback がパス解決として曖昧

対象:

- `hjson-edit/src/high_level/ext.rs#L26-L58`

問題点:

- `Object` と `Array` の場合は専用処理。
- それ以外のノードでは `self.children().find_map(|child| child.child(segment))` で子孫を再帰探索します。
- パスの 1 セグメントは通常「現在ノードの直下」を意味するはずですが、この実装だと直下でなくても見つかる可能性があります。

影響:

- 本来 `PathNotFound` にすべきパスが、たまたま深い位置の同名 key にヒットする可能性があります。
- コメント解決や set/remove の挙動が AST 構造に依存して曖昧になります。

整理案:

- `Value` の unwrap だけ許可し、それ以外の再帰探索はやめる。
- `Root`、`Value`、`Object`、`Array` のケースを明示的に扱う。

---

## 優先度中: 重複・不要ラッパー・統合候補

### 6. `walker.rs` は `get()` でしか使われておらず、パス解決処理と重複している

対象:

- `hjson-edit/src/high_level/walker.rs#L8-L49`
- `hjson-edit/src/high_level/ops/object.rs#L42-L49`
- `hjson-edit/src/high_level/ops/object.rs#L222-L233`

問題点:

- `PathWalker` は実質 `get()` 専用です。
- `set()` / `remove()` は別の `resolve_parent()` を使っています。
- つまりパス解決の実装が二系統あります。

影響:

- `get()` と `set()` / `remove()` でパス解決の edge case がズレます。
- 空パスについても、`set()` は root 置換をサポートしていますが、`get()` は `walker().last()` が `None` になり `PathNotFound` になります。

整理案:

- `resolve_path(root, path)` を共通化し、`get()` / `set()` / `remove()` / comment で使い回す。
- `PathWalker` が不要なら削除。
- 途中経路の iterator が API として必要なら、逆に `resolve_parent()` を walker ベースに寄せる。

---

### 7. `RedNodeExt` に未使用・薄いラッパーが多い

対象:

- `hjson-edit/src/high_level/ext.rs#L8-L18`
- `hjson-edit/src/high_level/ext.rs#L60-L109`
- `hjson-edit/src/high_level/ext.rs#L130-L133`

該当候補:

- `path()`
- `container_depth()`
- `to_hjson_value()`
- `children_by_kind()`
- `member_key()`
- `extract_key()`

問題点:

- `path()` と `container_depth()` は grep 上、crate 内で実質未使用に見えます。
- `to_hjson_value()` は `node_to_value()` の単純 alias です。
- `member_key()` / `extract_key()` は `ast::Member::member_key()` や `ast::Key::body_text()` と役割が重複しています。

整理案:

- `node_to_value()` だけ残す、または `AstNode` / semantic AST 側へ寄せる。
- `children_by_kind()` は便利メソッドとして残すなら `pub(crate)` 相当に閉じる。
- 公開 API でなければ未使用メソッドは削除。

---

### 8. `ast::edit` が薄い facade と未使用関数の置き場になっている

対象:

- `hjson-edit/src/ast/edit.rs#L11-L43`
- `hjson-edit/src/ast/edit.rs#L46-L61`
- `hjson-edit/src/ast/edit.rs#L64-L82`
- `hjson-edit/src/ast/edit.rs#L85-L124`
- `hjson-edit/src/ast/edit.rs#L127-L135`

問題点:

- `detected_string_style(...)`
- `detected_string_style_of_object(...)`
- `detected_string_style_of_array(...)`
- `detected_string_style_of_value(...)`

これらは既存の `document.detected_string_style()` 等を呼ぶだけの薄いラッパーです。

さらに:

- `render_trivia(...)`
- `protected_trivia_prefix_len(...)`
- `clone_green_elements(...)`

も grep 上、現在の src/tests では使用箇所が見当たりません。

また:

- `ast::edit::replace_member_value(...)`
- `high_level::ops::object::replace_member_value(...)`

が別々に存在しています。

影響:

- `ast::edit` が「編集用共通処理」なのか「high_level の残骸」なのか曖昧です。
- 実際には `high_level::ops::object` 側でローカル関数を使っており、`ast::edit::replace_member_value` は使われていないように見えます。

整理案:

- 本当に必要なものだけ `ast` のメソッドに寄せる。
- `detected_string_style_of_node` だけ必要なら、それを `ast` の utility として明示。
- 未使用の facade は削除候補。

---

### 9. `HjsonDocument::current_*` は内部用 getter が public API になっている

対象:

- `hjson-edit/src/high_level/mod.rs#L89-L109`

該当:

- `current_string_style()`
- `current_comment_style()`
- `current_append_mode()`
- `current_newline()`
- `current_root()`

問題点:

- `current_root()` は CLI parse でも使われていますが、内部構造の `RedNode` を直接公開しています。
- 他の `current_*` は主に high_level 内部実装用です。
- `current_newline()` は保存値を返しますが、編集処理本体では別の newline 検出を使うため、意味が不安定です。

整理案:

- 内部用は `pub(crate)` に落とす。
- CLI の tree dump 用には `syntax_root()` のような明示 API にする。
- `current_newline()` を残すなら、編集処理でも同じ値を使うようにする。

---

### 10. `HjsonObjectPath::new` と変換 trait がやや過剰

対象:

- `hjson-edit/src/high_level/path.rs#L54-L56`
- `hjson-edit/src/high_level/path.rs#L163-L191`

問題点:

- `HjsonObjectPath::new(path: impl Into<HjsonObjectPath>)` は一見便利ですが、`HjsonObjectPath` 自身への変換を経由するため意図が分かりづらいです。
- `impl Into<HjsonObjectPath> for &[PathSegment]` と `impl Into<HjsonObjectPath> for Vec<PathSegment>` は Rust 的には `From` 実装の方が自然です。
- `parse` / `TryFrom<&str>` / `FromStr` / `new` の役割がやや混在しています。

整理案:

- `HjsonObjectPath::parse(&str)` と `HjsonObjectPath::from_segments(Vec<PathSegment>)` に分ける。
- `Into` ではなく `From<Vec<PathSegment>>` / `From<&[PathSegment]>` にする。

---

### 11. `ObjectPathSegment` が 1 variant enum になっている

対象:

- `hjson-edit/src/high_level/path.rs#L5-L8`
- `hjson-edit/src/high_level/path.rs#L193-L199`

問題点:

- `ObjectPathSegment` は現在 `Key(String)` しかありません。
- `match_key()` も `k == key` の薄いラッパーです。
- `PathSegment::Object(ObjectPathSegment::Key(...))` というネストが深くなっています。

整理案:

- 将来 variant を増やす予定がなければ、`PathSegment::Key(String)` に簡略化できます。
- 将来拡張予定なら現状維持でもよいですが、今はやや過剰です。

---

### 12. `HjsonValue::render_scalar()` は `render_value()` の完全 alias

対象:

- `hjson-edit/src/high_level/value.rs#L265-L267`

問題点:

- `render_scalar()` は `self.render_value(string_style)` を呼ぶだけです。
- grep 上、使用箇所も見当たりません。

整理案:

- 未使用なら削除。
- API として必要なら名前を `render_value_node()` など実態に合わせる。

---

## 優先度中: レンダリング・変換処理の重複/不整合

### 13. 文字列 decode / strip / escape の実装が複数あり、挙動が揃っていない

対象:

- `hjson-edit/src/high_level/ext.rs#L253-L319`
- `hjson-edit/src/high_level/ext.rs#L321-L359`
- `hjson-edit/src/ast/mod.rs#L959-L971`
- `hjson-edit/src/high_level/value.rs#L269-L292`

問題点:

- `high_level/ext.rs` の `decode_double_quoted_lossless()` は escape sequence や unicode surrogate pair まで処理します。
- `ast/mod.rs` の `strip_quotes()` は引用符を外すだけで escape decode しません。
- `strip_block_string_lossless()` は block string の改行・インデント正規化をします。
- `ast/mod.rs` の `strip_block()` は `'''` を外すだけです。
- `render_string()` / `escape_double_quoted()` は `\` と `"` だけ escape し、改行やタブなどは処理していません。

影響:

- `doc.get()` で得た値と `doc.ast()` から得た値の文字列 body が異なる可能性があります。
- `get -> set` の往復で escape の扱いが変わる可能性があります。
- block string の body 解釈が high_level と ast でズレます。

整理案:

- `string_codec` のような共通モジュールに集約。
- AST projection と high_level value 変換で同じ decode/encode を使う。

---

### 14. `render_value` / `render_value_pretty` / `render_member` / `render_member_pretty` が重複している

対象:

- `hjson-edit/src/high_level/value.rs#L66-L132`
- `hjson-edit/src/high_level/value.rs#L134-L205`
- `hjson-edit/src/high_level/value.rs#L207-L229`
- `hjson-edit/src/high_level/value.rs#L232-L263`

問題点:

- 通常レンダリングと pretty レンダリングで GreenNodeBuilder の構築処理がほぼ二重化しています。
- `Object` と `Array` の pretty 処理も似た構造です。
- `render_member_pretty()` は `depth + 1` を渡していて、呼び出し側の `object_ast.layout.depth` との関係が分かりづらいです。

影響:

- 片方だけ直して片方を直し忘れるリスクがあります。
- `set_object_child()` / `set_array_child()` 側にも indent/newline 組み立てがあり、責務が分散しています。

整理案:

- `RenderConfig { quote_style, pretty, depth, newline }` のような設定オブジェクトを導入。
- value 側で「ノードをどう描画するか」、ops 側で「どこに挿入するか」に責務分離する。

---

### 15. `push_node()` が GreenNode を再帰コピーしている

対象:

- `hjson-edit/src/high_level/value.rs#L294-L304`
- `hjson-edit/src/high_level/value.rs#L306-L311`

問題点:

- `render_content()` が `GreenElement` を返し、`render_value()` がそれを再度 builder に push しています。
- Node の場合、`push_node()` で再帰的にコピーしています。

影響:

- 小規模なら問題ありませんが、配列/オブジェクトが大きい場合は余分なコピーになります。
- `GreenNode` を直接組み立てられる場所と、`GreenElement` を経由する場所が混在しています。

整理案:

- `render_content` と `render_value` の責務を見直す。
- 可能なら `GreenNode` を直接 splice できる形に寄せる。
- `GreenElement` 経由を減らす。

---

## 優先度中: object/comment ops の重複

### 16. root/value child の set/remove 処理がほぼ同じ

対象:

- `hjson-edit/src/high_level/ops/object.rs#L259-L284`
- `hjson-edit/src/high_level/ops/object.rs#L286-L311`
- `hjson-edit/src/high_level/ops/object.rs#L564-L585`

問題点:

- `set_root_child()` と `set_value_child()` は「対象 child を探して replace する」構造がほぼ同じです。
- `remove_root_child()` と `remove_value_child()` も同様です。

整理案:

- `replace_first_non_trivia_child(parent, predicate, f)` のような共通関数にする。
- `Root` と `Value` の違いは child の選び方だけにする。

---

### 17. `rebuild_tree_with_set()` の親種別 dispatch と `rebuild_child_with_set()` が重複している

対象:

- `hjson-edit/src/high_level/ops/object.rs#L105-L132`
- `hjson-edit/src/high_level/ops/object.rs#L145-L191`
- `hjson-edit/src/high_level/ops/object.rs#L657-L680`

問題点:

- `SyntaxKind::Root/Object/Array/Value` の match が複数箇所にあります。
- `rebuild_tree_with_set()` 内で既に dispatch しているのに、`set_root_child()` / `set_value_child()` 内でも `rebuild_child_with_set()` に委譲して再 dispatch しています。

整理案:

- 「指定ノードに対して 1 segment を set/remove する」関数を一本化する。
- `Root` と `Value` は最初に unwrap してから `Object` / `Array` に渡す。

---

### 18. コメント操作の before/inline/header/footer で似た処理が分散している

対象:

- `hjson-edit/src/high_level/ops/comment.rs#L55-L139`
- `hjson-edit/src/high_level/ops/comment.rs#L164-L223`
- `hjson-edit/src/high_level/ops/comment.rs#L231-L267`
- `hjson-edit/src/high_level/ops/comment.rs#L321-L374`

問題点:

- `set_comment()` の `Before` と `Inline` は以下の流れが似ています。
  - 対象 member 解決
  - 既存 style 抽出
  - 既存コメント削除
  - style 解決
  - comment node 作成
  - splice
- `Header` / `Footer` も style 抽出と削除・挿入が別関数に分かれています。
- `extract_comment_style()` と `extract_document_comment_style()` も似ています。

整理案:

- `make_comment_node(style, text)` を共通化。
- `replace_comment_at(Location)` のように location ごとの差分だけ分離。
- style 抽出は `trivia_comment_style(&[Trivia])` に統一し、document 側もそれを使う。

---

### 19. document comment の削除範囲が広すぎる可能性がある

対象:

- `hjson-edit/src/high_level/ops/comment.rs#L270-L319`

問題点:

- header 削除時、先頭から `Comment` または trivia/comment token をすべて削除しています。
- footer 削除時も末尾から trivia/comment をまとめて削除しています。
- 「コメントだけを消す」というより、周辺空白・改行もかなり広く消します。

影響:

- 入力の先頭/末尾に意図的な空行がある場合、それも消える可能性があります。
- `get_comment()` の単位と `remove_comment()` の削除単位が一致していない可能性があります。

整理案:

- `Document.leading_trivia` / `trailing_trivia` でコメント部分の span を厳密に取る。
- 「コメントだけ削除」「コメント周辺の空行も削除」をモード分けする。

---

## 優先度低: 未使用・公開 API 整理候補

### 20. `HjsonEditError::TypeMismatch` が未使用に見える

対象:

- `hjson-edit/src/high_level/error.rs#L23-L26`

問題点:

- grep 上、`TypeMismatch` を返している箇所が見当たりません。
- `expected` / `found` が `HjsonValue` なので、将来的なバリデーション用途の残骸に見えます。

整理案:

- 使う予定がなければ削除。
- 型チェック API を予定しているなら TODO コメントを付ける。

---

### 21. `ast` 側の `comments()` 系 API が shallow かつ未使用っぽい

対象:

- `hjson-edit/src/ast/mod.rs#L343-L347`
- `hjson-edit/src/ast/mod.rs#L427-L429`
- `hjson-edit/src/ast/mod.rs#L557-L559`
- `hjson-edit/src/ast/mod.rs#L653-L655`
- `hjson-edit/src/ast/mod.rs#L973-L978`

問題点:

- `Document::comments()` / `Object::comments()` / `Array::comments()` / `Member::comments()` は `collect_comments(&self.syntax)` を呼ぶだけです。
- `collect_comments()` は直下の `SyntaxKind::Comment` ノードだけを見ます。
- コメントは trivia として扱われる部分も多く、API 名から期待される「全コメント取得」とズレる可能性があります。
- grep 上、src/tests で積極的に使われていないように見えます。

整理案:

- API として残すなら「直下コメントのみ」など名前か doc を明確化。
- 未使用なら削除候補。

---

### 22. `empty_body_insert_index()` / `body_end_index()` / `append_*_index()` に getter と alias が多い

対象:

- `hjson-edit/src/ast/mod.rs#L430-L459`
- `hjson-edit/src/ast/mod.rs#L561-L590`

問題点:

- `append_member_index()` は `append_member_index_before_footer()` の alias。
- `append_value_index()` も `append_value_index_before_footer()` の alias。
- `empty_body_insert_index()` / `body_end_index()` は field を返すだけで、使われ方が限定的です。

整理案:

- 方針が `BeforeFooter` / `AfterComments` に分かれたなら、alias の `append_member_index()` / `append_value_index()` は削除してよさそうです。
- getter が public API として必要なければ field か internal helper に寄せる。

---

### 23. `Member` / `ArrayItem` の index helper が一部未使用

対象:

- `hjson-edit/src/ast/mod.rs#L666-L682`
- `hjson-edit/src/ast/mod.rs#L940-L956`

候補:

- `insert_after_index()`
- `post_node_index()`

問題点:

- grep 上、`insert_before_index()` は使われていますが、`insert_after_index()` / `post_node_index()` は使用箇所が見当たりません。
- `Member` と `ArrayItem` に同じ形で実装されています。

整理案:

- 未使用なら削除。
- 必要なら `SpannedItem` 的な trait にまとめる。

---

## CLI 側にも残っている不整合

### 24. comment path parse が `Result` を返すのに collector 内で `expect` している

対象:

- `hjson-edit/src/cli/edit.rs#L154-L156`
- `hjson-edit/src/cli/edit.rs#L240-L244`
- `hjson-edit/src/cli/edit.rs#L259-L263`
- `hjson-edit/src/cli/edit.rs#L276-L279`

問題点:

- `CliValueParser::parse_comment_path()` は `Result<HjsonCommentPath>` を返します。
- しかし `collect_comment_set_ops()` などでは `.expect("comment path should parse")` しています。
- `collect_comment_ops()` 自体は `Result<()>` を返す設計なのに、エラー伝播せず panic します。

整理案:

- `collect_comment_set_ops()` / `collect_comment_append_ops()` / `collect_comment_remove_ops()` を `Result<()>` にする。
- `?` でエラーを伝播する。

---

### 25. `comment-append` は CLI 側で `get_comment + set_comment` によって実装されている

対象:

- `hjson-edit/src/cli/edit.rs#L393-L401`

問題点:

- コメント append は high_level API ではなく CLI の組み合わせ処理です。
- `get_comment()` の結合順序や multiline 変換の仕様が CLI 側に漏れています。

整理案:

- `HjsonDocumentCommentEdit` に `append_comment()` を追加する。
- 仕様を high_level 側に閉じ込める。
