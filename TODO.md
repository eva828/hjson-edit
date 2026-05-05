## 優先度中: object/comment ops の重複

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
