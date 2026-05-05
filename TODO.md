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
