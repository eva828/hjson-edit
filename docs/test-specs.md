# hjson-edit テスト仕様書

> 本ドキュメントは `hjson-edit` の全テストケースについて、
> 入力・期待動作・備考をまとめたものである。

---

## 1. パス解析

### parse_valid_simple_key_succeeds (tests/path_parsing.rs)

#### 入力
`"name"` を `HjsonObjectPath::parse` に渡す

#### 期待動作
パース成功。`depth()` が `1` を返す

#### 備考
単一キーの基本的なパス

### parse_valid_nested_key_succeeds (tests/path_parsing.rs)

#### 入力
`"server.host"` をパース

#### 期待動作
パース成功。`depth()` が `2` を返す

#### 備考
ドット区切りのネストパス

### parse_valid_array_index_succeeds (tests/path_parsing.rs)

#### 入力
`"items[0]"` をパース

#### 期待動作
パース成功。`depth()` が `2` を返す

#### 備考
配列インデックス `[n]` を含むパス

### parse_valid_array_push_succeeds (tests/path_parsing.rs)

#### 入力
`"items[-]"` をパース

#### 期待動作
パース成功。`depth()` が `2` を返す

#### 備考
配列末尾へのプッシュ `[-]`

### parse_valid_array_insert_at_succeeds (tests/path_parsing.rs)

#### 入力
`"items[+1]"` をパース

#### 期待動作
パース成功。`depth()` が `2` を返す

#### 備考
配列中途への挿入 `[+n]`

### parse_valid_complex_mixed_path_succeeds (tests/path_parsing.rs)

#### 入力
`"a.b[0].c[1][2]"` をパース

#### 期待動作
パース成功。`depth()` が `6` を返す

#### 備考
キーと配列インデックスが混在した複雑なパス

### parse_empty_path_succeeds (tests/path_parsing.rs)

#### 入力
`""` をパース

#### 期待動作
パース成功。`depth()` が `0` を返す

#### 備考
空パスはルートを指す

### parse_unclosed_bracket_fails (tests/path_parsing.rs)

#### 入力
`"name["` をパース

#### 期待動作
パース失敗（エラーを返す）

#### 備考
閉じていないブラケットは不正

### parse_non_numeric_index_fails (tests/path_parsing.rs)

#### 入力
`"name[abc]"` をパース

#### 期待動作
パース失敗

#### 備考
インデックスは数値である必要がある

### parse_plus_without_number_fails (tests/path_parsing.rs)

#### 入力
`"name[+]"` をパース

#### 期待動作
パース失敗

#### 備考
`[+]` だけでは不十分で `[+n]` の形式が必要

### parse_double_dot_fails (tests/path_parsing.rs)

#### 入力
`"name..nested"` をパース

#### 期待動作
パース失敗（エラーを返す）

#### 備考
連続ドットは不正なパスとしてエラーになる

### parse_comment_path_header_succeeds (tests/path_parsing.rs)

#### 入力
`":header"` を `HjsonCommentPath::try_from` でパース

#### 期待動作
`HjsonCommentPath::Header` にマッチする

#### 備考

### parse_comment_path_footer_succeeds (tests/path_parsing.rs)

#### 入力
`":footer"` をパース

#### 期待動作
`HjsonCommentPath::Footer` にマッチする

#### 備考

### parse_comment_path_before_succeeds (tests/path_parsing.rs)

#### 入力
`"name:before"` をパース

#### 期待動作
`HjsonCommentPath::Before(_)` にマッチする

#### 備考

### parse_comment_path_inline_succeeds (tests/path_parsing.rs)

#### 入力
`"name:inline"` をパース

#### 期待動作
`HjsonCommentPath::Inline(_)` にマッチする

#### 備考

### parse_comment_path_nested_before_succeeds (tests/path_parsing.rs)

#### 入力
`"a.b:before"` をパース

#### 期待動作
`HjsonCommentPath::Before(_)` にマッチする

#### 備考
ネストパスに対する before コメント

### parse_comment_path_unknown_suffix_fails (tests/path_parsing.rs)

#### 入力
`"name:unknown"` をパース

#### 期待動作
パース失敗（エラーを返す）

#### 備考
`:header`, `:footer`, `:before`, `:inline` 以外のサフィックスは不正なパスとしてエラーになる

### path_split_first_returns_first_and_rest (tests/path_parsing.rs)

#### 入力
`"a.b.c"` をパースし `split_first()` を呼ぶ

#### 期待動作
`rest.depth()` が `2` を返す（先頭セグメントを除いた残り）

#### 備考

### path_split_last_returns_rest_and_last (tests/path_parsing.rs)

#### 入力
`"a.b.c"` をパースし `split_last()` を呼ぶ

#### 期待動作
`rest.depth()` が `2` を返す（最終セグメントを除いた残り）

#### 備考

### path_split_at_returns_two_halves (tests/path_parsing.rs)

#### 入力
`"a.b.c"` をパースし `split_at(1)` を呼ぶ

#### 期待動作
`left.depth()` が `1`、`right.depth()` が `2` を返す

#### 備考

### parse_leading_dot_fails (tests/path_edge_cases.rs)

#### 入力
`".a"` をパース

#### 期待動作
パース失敗（エラーを返す）

#### 備考
先頭ドットは不正なパスとしてエラーになる

### parse_trailing_dot_fails (tests/path_edge_cases.rs)

#### 入力
`"a."` をパース

#### 期待動作
パース失敗（エラーを返す）

#### 備考
末尾ドットは不正なパスとしてエラーになる

### parse_multiple_consecutive_dots_fails (tests/path_edge_cases.rs)

#### 入力
`"a...b"` をパース

#### 期待動作
パース失敗（エラーを返す）

#### 備考
連続ドット（2個以上）は不正なパスとしてエラーになる

### parse_unicode_segment_succeeds (tests/path_edge_cases.rs)

#### 入力
`"日本語"` をパース

#### 期待動作
`depth()` が `1` を返す

#### 備考
Unicode キーがパース可能

### parse_unicode_mixed_path_succeeds (tests/path_edge_cases.rs)

#### 入力
`"日本語.English"` をパース

#### 期待動作
`depth()` が `2` を返す

#### 備考
Unicode と ASCII の混在パス

---

## 2. 値取得 (tests/get.rs)

### get_existing_object_value_returns_string_value

#### 入力
`"{\n  name: app\n}\n"` をパースし、パス `"name"` で get

#### 期待動作
`HjsonValue::String("app")` が返る

#### 備考

### get_nonexistent_key_errors_path_not_found

#### 入力
`"{\n  name: app\n}\n"` をパースし、パス `"missing"` で get

#### 期待動作
エラー（`PathNotFound`）が返る

#### 備考

### get_nested_path_returns_string_value

#### 入力
`"{\n  server: { host: localhost }\n}\n"` をパースし、パス `"server.host"` で get

#### 期待動作
`HjsonValue::String("localhost")` が返る

#### 備考

### get_array_index_returns_integer_value

#### 入力
`"{\n  items: [1, 2, 3]\n}\n"` をパースし、パス `"items[0]"` で get

#### 期待動作
`HjsonValue::Integer(1)` が返る

#### 備考

### get_array_push_index_returns_last_integer_value

#### 入力
`"{\n  items: [1, 2, 3]\n}\n"` をパースし、パス `"items[-]"` で get

#### 期待動作
`HjsonValue::Integer(3)` が返る（`[-]` は末尾要素を指す）

#### 備考

### get_array_middle_index_returns_integer_value

#### 入力
`"{\n  items: [1, 2, 3]\n}\n"` をパースし、パス `"items[1]"` で get

#### 期待動作
`HjsonValue::Integer(2)` が返る

#### 備考

### get_root_scalar_returns_integer_value

#### 入力
`"123\n"` をパースし、空パス `""` で get

#### 期待動作
`HjsonValue::Integer(123)` が返る

#### 備考
ルートがスカラー値の場合

### get_root_braceless_object_value_returns_integer_value

#### 入力
`"name: app\nport: 8080\n"` をパースし、パス `"port"` で get

#### 期待動作
`HjsonValue::Integer(8080)` が返る

#### 備考
ブレースなしルートオブジェクト

### get_deeply_nested_path_returns_string_value

#### 入力
`"{\n  a: { b: { c: value } }\n}\n"` をパースし、パス `"a.b.c"` で get

#### 期待動作
`HjsonValue::String("value")` が返る

#### 備考

### get_array_then_object_path_returns_string_value

#### 入力
`"{\n  items: [{ name: a }, { name: b }]\n}\n"` をパースし、パス `"items[0].name"` で get

#### 期待動作
`HjsonValue::String("a")` が返る

#### 備考
配列→オブジェクトと遷移するパス

### get_value_with_brackets

#### 入力
`"{ spawn: x[i:CalamityEntropy/ProphecyToken]y }"` をパースし、パス `"spawn"` で get

#### 期待動作
`HjsonValue::String("x[i:CalamityEntropy/ProphecyToken]y")` が返る

#### 備考
`]` がアンクォート値の一部として正しく収集されることを確認（値開始時点では UnquotedString であり ScalarScanner 経由で収集される）

### get_value_with_braces

#### 入力
`"{ value: a{1}b }"` をパースし、パス `"value"` で get

#### 期待動作
`HjsonValue::String("a{1}b")` が返る

#### 備考
`}` がアンクォート値の一部として正しく収集されることを確認

---

## 3. 設定・削除 (tests/set_and_remove.rs)

### set_array_push_appends_to_end

#### 入力
`"{\n  items: [1, 2, 3]\n}\n"` に対し、パス `"items[-]"` で値 `4` を set

#### 期待動作
`"{\n  items: [1, 2, 3, 4]\n}\n"` が出力される

#### 備考
`[-]` による末尾追加

### set_array_index_updates_existing_value

#### 入力
`"{\n  items: [1, 2, 3]\n}\n"` に対し、パス `"items[1]"` で値 `4` を set

#### 期待動作
`"{\n  items: [1, 4, 3]\n}\n"` が出力される

#### 備考
既存インデックスの上書き

### set_array_insert_at_past_end_fails

#### 入力
`"{\n  items: [1, 2]\n}\n"` に対し、パス `"items[+10]"` で値 `3` を set

#### 期待動作
`ArrayIndexOutOfBounds` エラーが返る

#### 備考
`[+n]` は `n > items.len()` の場合エラーになる。`n == items.len()` の場合は末尾追加として動作する

### set_array_insert_at_inserts_in_middle

#### 入力
`"{\n  items: [1, 3]\n}\n"` に対し、パス `"items[+1]"` で値 `2` を set

#### 期待動作
`"{\n  items: [1, 2, 3]\n}\n"` が出力される

#### 備考
`[+1]` でインデックス1の位置に挿入

### set_array_insert_at_len_appends

#### 入力
`"{\n  items: [1, 2]\n}\n"` に対し、パス `"items[+2]"`（配列長と一致）で値 `3` を set

#### 期待動作
`"{\n  items: [1, 2, 3]\n}\n"` が出力される（末尾追加）

#### 備考
`[+n]` で `n == items.len()` の場合は末尾追加として扱われる

### set_object_key_updates_existing_value

#### 入力
`"{\n  name: app\n}\n"` に対し、パス `"name"` で値 `"api"` を set

#### 期待動作
`"{\n  name: api\n}\n"` が出力される

#### 備考
既存キーの値更新

### set_nested_object_path_creates_member

#### 入力
`"{\n  server: { host: localhost }\n}\n"` に対し、パス `"server.port"` で値 `8080` を set

#### 期待動作
`"{\n  server: { host: localhost, port: 8080 }\n}\n"` が出力される

#### 備考
既存のネストオブジェクトに新しいキーを追加

### set_missing_nested_path_creates_parents

#### 入力
`"{\n  root: {\n  }\n}\n"` に対し、パス `"root.new.nested"` で値 `"value"` を set

#### 期待動作
`"{\n  root: {\n    new: {\n      nested: value\n    }\n  }\n}\n"` が出力される

#### 備考
中間パスが存在しない場合は自動的にオブジェクトが作成される

### set_into_empty_object_inserts_member

#### 入力
`"{\n}\n"` に対し、パス `"name"` で値 `"app"` を set

#### 期待動作
`"{\n  name: app\n}\n"` が出力される

#### 備考
空オブジェクトへのメンバー追加

### set_into_empty_array_inserts_item

#### 入力
`"[\n]\n"` に対し、パス `"[-]"` で値 `1` を set

#### 期待動作
`"[\n  1\n]\n"` が出力される

#### 備考
空配列へのアイテム追加

### set_existing_key_preserves_leading_comments

#### 入力
`"{\n  // keep this note\n  name: app\n}\n"` に対し、パス `"name"` で値 `"api"` を set

#### 期待動作
`"{\n  // keep this note\n  name: api\n}\n"` が出力される

#### 備考
既存キーの前方コメントは保持される

### set_preserves_crlf_line_endings

#### 入力
CRLF を含むドキュメントに対し、パス `"items[-]"` で値 `3` を set

#### 期待動作
`"{\n  items: [\r\n    1\r\n    2\r\n    3\r\n  ]\n}\n"` が出力される

#### 備考
CRLF 改行が保持される

### append_to_multiline_object

#### 入力
`"{\n  a: 1\n}\n"` に対し、パス `"b"` で値 `2` を set

#### 期待動作
`"{\n  a: 1\n  b: 2\n}\n"` が出力される

#### 備考
Hjson では改行がセパレータとなる（カンマは付かない）

### append_to_multiline_array

#### 入力
`"[\n  1\n]\n"` に対し、パス `"[-]"` で値 `2` を set

#### 期待動作
`"[\n  1\n  2\n]\n"` が出力される

#### 備考

### remove_object_key_deletes_member

#### 入力
`"{\n  server: { host: localhost, port: 8080 }\n}\n"` に対し、パス `"server.port"` を remove

#### 期待動作
`"{\n  server: { host: localhost,}\n}\n"` が出力される

#### 備考
オブジェクトメンバーの削除。カンマは残る

### remove_array_last_item_deletes_end_element

#### 入力
`"{\n  items: [1, 2, 3]\n}\n"` に対し、パス `"items[-]"` を remove

#### 期待動作
`"{\n  items: [1, 2,]\n}\n"` が出力される

#### 備考
配列末尾の削除。末尾カンマが残る

### remove_object_member_cleans_trivia_avoids_empty_lines

#### 入力
`"{\n  a: aaa,\n  b: bbb\n  c: ccc\n  d: ddd\n}\n"` に対し、パス `"c"` を remove

#### 期待動作
`"{\n  a: aaa,\n  b: bbb\n  d: ddd\n}\n"` が出力される（空行が発生しない）

#### 備考
削除後の trivia 後片付けにより空行が除去される

### remove_last_object_member_preserves_trailing_comma

#### 入力
`"{\n  keep: 1,\n  remove: 2\n}\n"` に対し、パス `"remove"` を remove

#### 期待動作
`"{\n  keep: 1,\n}\n"` が出力される

#### 備考
最終メンバー削除後も末尾カンマは維持される

---

## 4. コメント操作 (tests/comments.rs / tests/deep_comments.rs / tests/append_comment.rs)

### set_header_comment_uses_explicit_line_style (tests/comments.rs)

#### 入力
`"{\n  name: app\n}\n"` に対し、`comment_style(Line)` で `":header"` に `"top"` を set_comment

#### 期待動作
`"// top\n{\n  name: app\n}\n"` が出力される

#### 備考

### set_header_comment_uses_explicit_hash_style (tests/comments.rs)

#### 入力
`"{\n  name: app\n}\n"` に対し、`comment_style(Hash)` で `":header"` に `"top"` を set_comment

#### 期待動作
`"# top\n{\n  name: app\n}\n"` が出力される

#### 備考

### set_header_comment_uses_explicit_block_style (tests/comments.rs)

#### 入力
`"{\n  name: app\n}\n"` に対し、`comment_style(Block)` で `":header"` に `"top"` を set_comment

#### 期待動作
`"/* top */\n{\n  name: app\n}\n"` が出力される

#### 備考

### set_then_remove_header_comment_roundtrips (tests/comments.rs)

#### 入力
`"{\n  name: app\n}\n"` → set_header → get_header → remove_header

#### 期待動作
1. ヘッダーコメント設定後、`get_comment` で `Some("top")` が返る
2. `remove_comment` 後、元の入力と同一の文字列に戻る

#### 備考
set → get → remove のラウンドトリップ

### set_before_comment_inserts_above_member (tests/comments.rs)

#### 入力
`"{\n  name: app\n}\n"` に対し、`"name:before"` に `"note"` を set_comment

#### 期待動作
`"{\n  // note\n  name: app\n}\n"` が出力される

#### 備考

### set_inline_comment_appends_after_value (tests/comments.rs)

#### 入力
`"{\n  name: app\n}\n"` に対し、`"name:inline"` に `"note"` を set_comment

#### 期待動作
`"{\n  name: app // note\n}\n"` が出力される

#### 備考

### set_footer_comment_appends_after_document (tests/comments.rs)

#### 入力
`"{\n  name: app\n}\n"` に対し、`comment_style(Hash)` で `":footer"` に `"done"` を set_comment

#### 期待動作
`"{\n  name: app\n}\n# done\n"` が出力される

#### 備考

### set_then_remove_footer_comment_roundtrips (tests/comments.rs)

#### 入力
ドキュメント → set_footer → get_footer → remove_footer

#### 期待動作
1. `get_comment` で `Some("done")` が返る
2. `remove_comment` 後、元の入力と同一の文字列に戻る

#### 備考

### remove_header_comment_preserves_leading_blank_lines (tests/comments.rs)

#### 入力
`"\n\n# existing\n{\n  name: app\n}\n"` に対し、`":header"` のコメントを remove

#### 期待動作
`"\n\n{\n  name: app\n}\n"` が出力される（先頭の空行は維持）

#### 備考

### remove_footer_comment_preserves_trailing_blank_lines (tests/comments.rs)

#### 入力
`"{\n  name: app\n}\n\n# existing\n\n"` に対し、`":footer"` のコメントを remove

#### 期待動作
`"{\n  name: app\n}\n\n\n"` が出力される（末尾の空行は維持）

#### 備考

### set_header_comment_preserves_existing_blank_lines (tests/comments.rs)

#### 入力
`"\n\n{\n  name: app\n}\n"` に対し、`":header"` に `"top"` を set_comment

#### 期待動作
`"\n\n// top\n{\n  name: app\n}\n"` が出力される（先頭の空行は維持）

#### 備考

### set_footer_comment_preserves_existing_blank_lines (tests/comments.rs)

#### 入力
`"{\n  name: app\n}\n\n"` に対し、`":footer"` に `"done"` を set_comment

#### 期待動作
`"{\n  name: app\n}\n\n# done\n"` が出力される（末尾の空行は維持）

#### 備考

### remove_missing_header_comment_is_noop (tests/comments.rs)

#### 入力
`"{\n  foo: 1\n}"` に対し、`":header"` のコメントを remove

#### 期待動作
入力と同じ文字列が出力される（no-op）

#### 備考

### set_before_comment_overwrites_existing (tests/comments.rs)

#### 入力
`"{\n  // old comment\n  foo: 1\n}"` に対し、`"foo:before"` に `"new comment"` を set_comment

#### 期待動作
`"{\n  // new comment\n  foo: 1\n}"` が出力される

#### 備考
既存の before コメントは上書きされる

### set_multiline_header_comment_with_line_style_splits_into_lines (tests/comments.rs)

#### 入力
`"{\n  foo: 1\n}"` に対し、`comment_style(Line)` で `":header"` に複数行テキスト `"line1\nline2"` を set_comment

#### 期待動作
`"// line1\n// line2\n{\n  foo: 1\n}"` が出力される（行ごとに独立した `//` コメントに分割）

#### 備考
Line スタイルが明示された場合、複数行コメントは行分割されて出力される

### set_inline_comment_on_deep_path (tests/deep_comments.rs)

#### 入力
`"{\n  a: {\n    b: {\n      c: value\n    }\n  }\n}\n"` に対し、パス `"a.b.c:inline"` に `"note"` を set_comment

#### 期待動作
出力に `"// note"` が含まれる

#### 備考
深くネストされたパスへのインラインコメント設定

### get_inline_comment_on_deep_path_returns_comment (tests/deep_comments.rs)

#### 入力
`"{\n  a: {\n    b: {\n      c: \"value\" // note\n    }\n  }\n}\n"` に対し、パス `"a.b.c:inline"` で get_comment

#### 期待動作
`Some("note")` が返る

#### 備考

### remove_inline_comment_on_deep_path_removes_comment (tests/deep_comments.rs)

#### 入力
`"{\n  a: {\n    b: {\n      c: \"value\" // note\n    }\n  }\n}\n"` に対し、パス `"a.b.c:inline"` のコメントを remove

#### 期待動作
出力に `"// note"` が含まれない

#### 備考

### set_before_comment_on_deep_path (tests/deep_comments.rs)

#### 入力
`"{\n  a: {\n    b: {\n      c: value\n    }\n  }\n}\n"` に対し、パス `"a.b.c:before"` に `"note"` を set_comment

#### 期待動作
出力に `"// note"` が含まれる

#### 備考

### append_comment_to_existing_header_appends_below (tests/append_comment.rs)

#### 入力
既存のヘッダーコメントがあるドキュメントに対し、`:header` に `"second"` を append_comment

#### 期待動作
`"// first\n// second\n{\n  name: app\n}\n"` が出力される（行ごとに `//` コメントを追加）

#### 備考
append_comment は `\n` で連結し、既存の Line スタイルを継承して行分割される

### append_comment_to_existing_inline_appends_inline (tests/append_comment.rs)

#### 入力
既存のインラインコメントがあるドキュメントに対し、`"name:inline"` に `"second"` を append_comment

#### 期待動作
`"{\n  name: app /* first\nsecond */}\n"` が出力される

#### 備考
複数行テキストは Block スタイルに強制される

### append_comment_to_member_without_existing_comment_inserts_new (tests/append_comment.rs)

#### 入力
コメントがないメンバーに対し、`"name:before"` に `"note"` を append_comment

#### 期待動作
`"{\n  // note\n  name: app\n}\n"` が出力される

#### 備考
既存コメントがない場合は新規挿入と同じ動作

### append_comment_to_footer_appends_below_existing (tests/append_comment.rs)

#### 入力
既存のフッターコメントがあるドキュメントに対し、`:footer` に `"second"` を append_comment

#### 期待動作
`"{\n  name: app\n}\n# first\n# second\n"` が出力される（行ごとに `#` コメントを追加）

#### 備考
既存の Hash スタイルを継承して行分割される

---

## 5. レクサーエラー (tests/lexer_errors.rs)

### lexer_unterminated_block_comment_produces_error

#### 入力
`"/* hello"` をパース

#### 期待動作
エラーが返る

#### 備考
閉じていないブロックコメント

### lexer_unterminated_block_string_produces_error

#### 入力
`"'''hello"` をパース

#### 期待動作
エラーが返る

#### 備考
閉じていないブロック文字列

### lexer_unterminated_double_quoted_string_produces_error

#### 入力
`"\"hello"` をパース

#### 期待動作
エラーが返る

#### 備考
閉じていないダブルクォート文字列

### lexer_newline_in_double_quoted_string_produces_error

#### 入力
`"\"hello\nworld\""` をパース

#### 期待動作
エラーが返る

#### 備考
ダブルクォート文字列内の改行は不正

### lexer_empty_double_quoted_string_is_valid

#### 入力
`"\"\""` をパース

#### 期待動作
成功

#### 備考
空文字列 `""` は有効

### lexer_escaped_backslash_at_start_of_string

#### 入力
`"\"\\\\\""` をパース

#### 期待動作
成功

#### 備考
`"\\"`（エスケープされたバックスラッシュ1個）は有効

### lexer_invalid_escape_x_at_start_is_rejected

#### 入力
`"\"\\x\""` をパース

#### 期待動作
エラーが返る

#### 備考
`\x` は不正なエスケープシーケンス

### lexer_invalid_escape_z_at_start_is_rejected

#### 入力
`"\"\\z\""` をパース

#### 期待動作
エラーが返る

#### 備考
`\z` は不正なエスケープシーケンス

### lexer_incomplete_unicode_escape_at_start_is_rejected

#### 入力
`"\"\\u\""` をパース

#### 期待動作
エラーが返る

#### 備考
`\u` の後に16進数がない

### lexer_invalid_unicode_hex_at_start_is_rejected

#### 入力
`"\"\\uZZZZ\""` をパース

#### 期待動作
エラーが返る

#### 備考
`\uZZZZ` は不正な Unicode エスケープ

### lexer_unexpected_punctuator_in_quoteless_context_produces_error

#### 入力
`"{"` をパース

#### 期待動作
エラーが返る

#### 備考
閉じられていないオブジェクト開始

### lexer_empty_input_parses_successfully

#### 入力
`""` をパース

#### 期待動作
成功

#### 備考
空入力は有効

### lexer_whitespace_only_input_parses_successfully

#### 入力
`"   "` をパース

#### 期待動作
成功

#### 備考
空白のみの入力は有効

### lexer_line_comment_only_input_parses_successfully

#### 入力
`"// hello"` をパース

#### 期待動作
成功

#### 備考
コメントのみの入力は有効

### lexer_hash_comment_only_input_parses_successfully

#### 入力
`"# hello"` をパース

#### 期待動作
成功

#### 備考
ハッシュコメントのみの入力は有効

---

## 6. パーサーエラー (tests/parser_errors.rs)

### parser_unclosed_object_produces_error

#### 入力
`"{ \"a\": 1"` をパース

#### 期待動作
エラーが返る

#### 備考

### parser_unclosed_array_produces_error

#### 入力
`"[1, 2"` をパース

#### 期待動作
エラーが返る

#### 備考

### parser_missing_colon_produces_error

#### 入力
`"{ \"a\" 1 }"` をパース

#### 期待動作
エラーが返る

#### 備考
キーと値の間にコロンがない

### parser_empty_key_produces_error

#### 入力
`"{:1}"` をパース

#### 期待動作
エラーが返る

#### 備考
空のキーは不正

### parser_stray_rbrace_produces_error

#### 入力
`"}"` をパース

#### 期待動作
エラーが返る

#### 備考
対応する開き括弧のない閉じ括弧

### parser_stray_rbracket_produces_error

#### 入力
`"]"` をパース

#### 期待動作
エラーが返る

#### 備考

### parser_stray_comma_produces_error

#### 入力
`","` をパース

#### 期待動作
エラーが返る

#### 備考

### parser_stray_colon_produces_error

#### 入力
`":"` をパース

#### 期待動作
エラーが返る

#### 備考

### parser_double_colon_produces_error

#### 入力
`"{\"a\"::1}"` をパース

#### 期待動作
エラーが返る

#### 備考

### parser_block_string_with_trailing_content_produces_error

#### 入力
`"'''hello'''world"` をパース

#### 期待動作
エラーが返る

#### 備考
ブロック文字列の直後に余分な内容がある

### parser_scalar_before_braceless_object_produces_error

#### 入力
`"123\na:1"` をパース

#### 期待動作
エラーが返る

#### 備考
スカラー値とブレースレスオブジェクトの混在

### parser_multiple_root_values_produces_error

#### 入力
`"1 2 3"` をパース

#### 期待動作
エラーが返る

#### 備考
ルートに複数の値がある

### parser_whitespace_separated_array_items_should_error

#### 入力
`"[1 2 3]"` をパース

#### 期待動作
エラーが返る

#### 備考
配列アイテムはカンマまたは改行で区切る必要がある（空白のみは不可）

### parser_whitespace_separated_object_members_should_error

#### 入力
`"{a:1 b:2}"` をパース

#### 期待動作
エラーが返る

#### 備考
オブジェクトメンバーも同様

### parser_tab_separated_array_items_should_error

#### 入力
`"[1\t2\t3]"` をパース

#### 期待動作
エラーが返る

#### 備考
タブ区切りも不可

### parser_tab_separated_object_members_should_error

#### 入力
`"{a:1\tb:2}"` をパース

#### 期待動作
エラーが返る

#### 備考

### parser_string_values_without_comma_should_error

#### 入力
`"[\"a\" \"b\"]"` をパース

#### 期待動作
エラーが返る

#### 備考
クォートされた文字列でも空白のみの区切りは不可

### unquoted_value_with_bracket_is_valid

#### 入力
`"{ v: x[y]z }"` をパース

#### 期待動作
パース成功 (`Ok`)

#### 備考
アンクォート値内の `]` はエラーにならない（`parser_stray_rbracket_produces_error` の回帰テスト）

### unquoted_value_with_brace_is_valid

#### 入力
`"{ v: x{y}z }"` をパース

#### 期待動作
パース成功 (`Ok`)

#### 備考
アンクォート値内の `}` はエラーにならない（`parser_stray_rbrace_produces_error` の回帰テスト）

---

## 7. ラウンドトリップ (tests/round_trip.rs)

### roundtrip_braceless_root

#### 入力
`"foo: bar\n  baz: 123"` をパースし `to_string()` で出力

#### 期待動作
入力と出力が同一

#### 備考
ブレースなしルートオブジェクトのラウンドトリップ

### roundtrip_mixed_comments

#### 入力
ハッシュコメント・ラインコメント・ブロックコメント・インラインコメントを含むドキュメント

#### 期待動作
入力と出力が同一

#### 備考

### roundtrip_block_strings

#### 入力
`"{\n  doc: '''\n    Line 1\n    Line 2\n  '''\n}"` をパース

#### 期待動作
入力と出力が同一

#### 備考
ブロック文字列のラウンドトリップ

### roundtrip_mixed_newlines

#### 入力
`\n` と `\r\n` が混在したドキュメント

#### 期待動作
入力と出力が同一

#### 備考

### roundtrip_escape_sequences

#### 入力
各種エスケープシーケンスを含むダブルクォート文字列

#### 期待動作
入力と出力が同一

#### 備考

### roundtrip_unquoted_with_bracket_in_value

#### 入力
`"{ spawn: x[i:CalamityEntropy/ProphecyToken]y }"` をパースし `to_string()` で出力

#### 期待動作
入力と出力が同一

#### 備考
アンクォート値に `[` と `]` を含むケースのラウンドトリップ（値先頭が非 punctuator なので ScalarScanner 経由で収集される）

### roundtrip_unquoted_with_brace_in_value

#### 入力
`"{ value: a{1}b }"` をパースし `to_string()` で出力

#### 期待動作
入力と出力が同一

#### 備考
アンクォート値に `{` と `}` を含むケースのラウンドトリップ

### roundtrip_unquoted_with_bracket_braceless

#### 入力
`"key: abc[xyz]def\n"` をパースし `to_string()` で出力

#### 期待動作
入力と出力が同一

#### 備考
ブレースレスルートで `]` を含むケース

### roundtrip_unquoted_with_mixed_brackets

#### 入力
`"{ data: pre[abc]{def}post }"` をパースし `to_string()` で出力

#### 期待動作
入力と出力が同一

#### 備考
`]` と `}` の両方を含むケース

---

## 8. 数値分類 (tests/number_integration.rs + src/number.rs)

### root_number_with_leading_zero_returns_string (tests/number_integration.rs)

#### 入力
`"01"` をパースし `get("")` で値取得

#### 期待動作
`HjsonValue::String("01")` が返る

#### 備考
先頭ゼロは文字列として扱われる

### root_number_with_trailing_dot_returns_float (tests/number_integration.rs)

#### 入力
`"1."` をパースし `get("")` で値取得

#### 期待動作
`HjsonValue::Float(1.0)` が返る

#### 備考
後置ドットは Float として解釈される

### root_number_with_incomplete_exponent_returns_string (tests/number_integration.rs)

#### 入力
`"1e"` をパースし `get("")` で値取得

#### 期待動作
`HjsonValue::String("1e")` が返る

#### 備考
不完全な指数は文字列として扱われる

### root_number_with_plus_sign_returns_string (tests/number_integration.rs)

#### 入力
`"+1"` をパースし `get("")` で値取得

#### 期待動作
`HjsonValue::String("+1")` が返る

#### 備考
`+` 符号は許容されず文字列として扱われる

### root_number_hex_returns_string (tests/number_integration.rs)

#### 入力
`"0x1"` をパースし `get("")` で値取得

#### 期待動作
`HjsonValue::String("0x1")` が返る

#### 備考
16進数は文字列として扱われる

### root_number_bare_decimal_returns_string (tests/number_integration.rs)

#### 入力
`".5"` をパースし `get("")` で値取得

#### 期待動作
`HjsonValue::String(".5")` が返る

#### 備考
小数点のみの数値は文字列として扱われる

### root_number_over_i64_max_returns_error (tests/number_integration.rs)

#### 入力
`"9223372036854775808"` をパースし `get("")` で値取得

#### 期待動作
エラーが返る（i64 範囲超過）

#### 備考

### root_number_under_i64_min_returns_error (tests/number_integration.rs)

#### 入力
`"-9223372036854775809"` をパースし `get("")` で値取得

#### 期待動作
エラーが返る（i64 範囲超過）

#### 備考

### root_number_large_exponent_does_not_panic (tests/number_integration.rs)

#### 入力
`"1e99999999999999999999999999999999999999"` をパースし `get("")` で値取得

#### 期待動作
パニックしない（エラーは許容）

#### 備考
巨大な指数が与えられてもパニックせず適切に処理する

### classify_valid_integer_returns_some_integer (src/number.rs)

#### 入力
`"0"`, `"123"`, `"-456"`, `"-0"` を classify

#### 期待動作
全て `Some(NumberKind::Integer)` が返る

#### 備考

### classify_valid_float_returns_some_float (src/number.rs)

#### 入力
`"1.5"`, `"-3.14"`, `"1e10"`, `"2.5E-3"`, `"6.022e+23"` を classify

#### 期待動作
全て `Some(NumberKind::Float)` が返る

#### 備考

### classify_leading_zero_with_digit_returns_none (src/number.rs)

#### 入力
`"01"`, `"007"` を classify

#### 期待動作
`None` が返る

#### 備考
先頭ゼロに続けて数字がある場合、数値として認識されない

### classify_trailing_decimal_returns_float (src/number.rs)

#### 入力
`"1."`, `"123."` を classify

#### 期待動作
`Some(NumberKind::Float)` が返る

#### 備考

### classify_incomplete_exponent_returns_none (src/number.rs)

#### 入力
`"1e"`, `"1E"`, `"1e+"`, `"1e-"` を classify

#### 期待動作
全て `None` が返る

#### 備考

### classify_plus_sign_returns_none (src/number.rs)

#### 入力
`"+1"` を classify

#### 期待動作
`None` が返る

#### 備考

### classify_hex_returns_none (src/number.rs)

#### 入力
`"0x1"`, `"0x1A"` を classify

#### 期待動作
`None` が返る

#### 備考

### classify_bare_decimal_returns_none (src/number.rs)

#### 入力
`".5"`, `"."` を classify

#### 期待動作
`None` が返る

#### 備考

### classify_multiple_decimals_returns_none (src/number.rs)

#### 入力
`"1.2.3"` を classify

#### 期待動作
`None` が返る

#### 備考

### classify_non_numeric_returns_none (src/number.rs)

#### 入力
`""`, `"-"`, `"NaN"`, `"Infinity"`, `"12abc"` を classify

#### 期待動作
全て `None` が返る

#### 備考

### is_valid_returns_true_for_valid_numbers (src/number.rs)

#### 入力
`"42"`, `"-1.5"`, `"1e10"`, `"1."`, `"123."`

#### 期待動作
`is_valid` が `true` を返す

#### 備考

### is_valid_returns_false_for_invalid_numbers (src/number.rs)

#### 入力
`"01"`, `"1e"`, `"abc"`

#### 期待動作
`is_valid` が `false` を返す

#### 備考

---

## 9. 文字列デコード (tests/string_decode.rs)

### decode_double_quoted_simple

#### 入力
`QuoteStyle::DoubleQuoted.decode_body("\"hello\"")`

#### 期待動作
`"hello"` が返る

#### 備考

### decode_double_quoted_with_escaped_quote

#### 入力
`"\"hello\\\"world\""` をデコード

#### 期待動作
`"hello\"world"` が返る

#### 備考

### decode_double_quoted_with_escaped_backslash

#### 入力
`"\"path\\\\to\\\\file\""` をデコード

#### 期待動作
`"path\\to\\file"` が返る

#### 備考

### decode_double_quoted_with_escaped_solidus

#### 入力
`"\"hello\\/world\""` をデコード

#### 期待動作
`"hello/world"` が返る

#### 備考

### decode_double_quoted_with_escaped_backspace

#### 入力
`"\"hello\\bworld\""` をデコード

#### 期待動作
`"hello\u{0008}world"` が返る

#### 備考

### decode_double_quoted_with_escaped_formfeed

#### 入力
`"\"hello\\fworld\""` をデコード

#### 期待動作
`"hello\u{000C}world"` が返る

#### 備考

### decode_double_quoted_with_escaped_newline

#### 入力
`"\"hello\\nworld\""` をデコード

#### 期待動作
`"hello\nworld"` が返る

#### 備考

### decode_double_quoted_with_escaped_carriage_return

#### 入力
`"\"hello\\rworld\""` をデコード

#### 期待動作
`"hello\rworld"` が返る

#### 備考

### decode_double_quoted_with_escaped_tab

#### 入力
`"\"hello\\tworld\""` をデコード

#### 期待動作
`"hello\tworld"` が返る

#### 備考

### decode_double_quoted_unicode_escapes_preserved

#### 入力
`"\"\\u0041\""` をデコード

#### 期待動作
`"\\u0041"` が返る（デコードせず生のテキストとして保持）

#### 備考
RFC 8259 では `\u0041` は `'A'` にデコードすべきだが、この実装ではラウンドトリップ保存のため生テキストを保持する

### decode_block_simple

#### 入力
`"'''hello'''"` をデコード

#### 期待動作
`"hello"` が返る

#### 備考

### decode_block_empty

#### 入力
`"''''''"` をデコード

#### 期待動作
`""` が返る

#### 備考

### decode_block_strips_common_indent

#### 入力
`"'''\n  line1\n  line2\n'''"` をデコード

#### 期待動作
`"line1\nline2"` が返る（共通インデントが除去される）

#### 備考

### decode_block_crlf_normalized

#### 入力
`"'''hello\r\nworld'''"` をデコード

#### 期待動作
`"hello\nworld"` が返る（CRLF が LF に正規化）

#### 備考

### decode_block_cr_normalized

#### 入力
`"'''hello\rworld'''"` をデコード

#### 期待動作
`"hello\nworld"` が返る（CR が LF に正規化）

#### 備考

### decode_unquoted_returns_raw

#### 入力
`"hello"` をデコード

#### 期待動作
`"hello"` が返る

#### 備考
引用符なしはそのまま

### decode_double_quoted_unknown_escapes_preserved

#### 入力
`"\"\\z\""` をデコード

#### 期待動作
`"\\z"` が返る（生のテキストとして保持）

#### 備考
RFC 8259 では `\z` はエラーにすべきだが、この実装ではラウンドトリップ保存のため生テキストを保持する

---

## 10. 文字列エンコード (tests/string_encode.rs)

### encode_double_quoted_escapes_backslash

#### 入力
`QuoteStyle::DoubleQuoted.encode_body("path\\to\\file")`

#### 期待動作
`"path\\\\to\\\\file"` が返る

#### 備考

### encode_double_quoted_escapes_quote

#### 入力
`"hello\"world"` をエンコード

#### 期待動作
`"hello\\\"world"` が返る

#### 備考

### encode_double_quoted_no_special_chars_is_identity

#### 入力
`"hello"` をエンコード

#### 期待動作
`"hello"` が返る（変化なし）

#### 備考

### encode_block_is_identity

#### 入力
`"hello\nworld"` を Block スタイルでエンコード

#### 期待動作
`"hello\nworld"` が返る（変化なし）

#### 備考

### encode_unquoted_is_identity

#### 入力
`"hello"` を Unquoted スタイルでエンコード

#### 期待動作
`"hello"` が返る（変化なし）

#### 備考

---

## 11. コメントデコード (tests/comment_decode.rs)

### decode_line_comment_strips_prefix_and_leading_space

#### 入力
`CommentKind::Line.decode_body("// comment")`

#### 期待動作
`"comment"` が返る

#### 備考

### decode_line_comment_strips_prefix_without_space

#### 入力
`"//comment"` をデコード

#### 期待動作
`"comment"` が返る

#### 備考

### decode_line_comment_strips_multiple_leading_spaces

#### 入力
`"//  comment"` をデコード

#### 期待動作
`"comment"` が返る

#### 備考

### decode_hash_comment_strips_prefix_and_leading_space

#### 入力
`"# comment"` をデコード

#### 期待動作
`"comment"` が返る

#### 備考

### decode_hash_comment_strips_prefix_without_space

#### 入力
`"#comment"` をデコード

#### 期待動作
`"comment"` が返る

#### 備考

### decode_block_comment_strips_delimiters_and_trims

#### 入力
`"/* comment */"` をデコード

#### 期待動作
`"comment"` が返る

#### 備考

### decode_block_comment_strips_delimiters_with_extra_whitespace

#### 入力
`"/*  comment  */"` をデコード

#### 期待動作
`"comment"` が返る

#### 備考

---

## 12. ルート配列 (tests/root_array.rs)

### get_root_array_item_by_index

#### 入力
`"[1, 2, 3]"` をパースし、パス `"[0]"` で get

#### 期待動作
`HjsonValue::Integer(1)` が返る

#### 備考
ルートが配列の場合の要素取得

### set_root_array_item_by_index

#### 入力
`"[1, 2, 3]"` に対し、パス `"[1]"` で値 `4` を set

#### 期待動作
`"[1, 4, 3]"` が出力される

#### 備考

### remove_root_array_last_item_no_trailing_comma

#### 入力
`"[1, 2, 3]"` に対し、パス `"[-]"` を remove

#### 期待動作
`"[1, 2,]"` が出力される（末尾カンマが残る）

#### 備考

### set_push_on_root_array_appends

#### 入力
`"[1, 2]"` に対し、パス `"[-]"` で値 `3` を set

#### 期待動作
`"[1, 2, 3]"` が出力される

#### 備考

---

## 13. ルート・エッジケース (tests/root_and_edge_cases.rs)

### parse_completely_empty_file_roundtrips

#### 入力
`""` をパースし `to_string()` で出力

#### 期待動作
`""` が出力される

#### 備考

### set_in_empty_document_inserts_braceless_object_member

#### 入力
空ドキュメントに対し、パス `"foo"` で値 `1` を set

#### 期待動作
`"foo: 1\n"` が出力される

#### 備考
空ドキュメントへの set はブレースレスオブジェクトとして出力

### parse_whitespace_only_file_roundtrips

#### 入力
`"   \n  \t  \n"` をパースし `to_string()` で出力

#### 期待動作
入力と出力が同一

#### 備考

### set_in_whitespace_only_document_preserves_whitespace_and_adds_member

#### 入力
空白のみのドキュメントに対し、パス `"foo"` で値 `1` を set

#### 期待動作
`"   \n  \t  \nfoo: 1\n"` が出力される（既存の空白は維持）

#### 備考

### parse_comment_only_file_roundtrips

#### 入力
`"// just a comment\n/* block */\n# hash comment"` をパースし `to_string()` で出力

#### 期待動作
入力と出力が同一

#### 備考

### set_in_comment_only_document_preserves_comments_and_adds_member

#### 入力
コメントのみのドキュメントに対し、パス `"foo"` で値 `1` を set

#### 期待動作
1. 出力に `"foo: 1"` が含まれる
2. 出力に `"just a comment"` が含まれる（コメント維持）

#### 備考

### parse_single_scalar_value_roundtrips

#### 入力
`"1"` をパースし `to_string()` で出力

#### 期待動作
`"1"` が出力される

#### 備考

### set_on_scalar_root_returns_error

#### 入力
`"1"` にパス `"foo"` で set

#### 期待動作
エラーが返る（スカラールートには set 不可）

#### 備考

---

## 14. スタイル制御 (tests/style_and_format.rs)

### set_with_preserve_string_style_keeps_double_quotes

#### 入力
`"{\n  name: \"app\"\n}\n"` に対し、`string_style(Preserve)` でパス `"name"` に `"api"` を set

#### 期待動作
`"{\n  name: \"api\"\n}\n"` が出力される（ダブルクォートを維持）

#### 備考

### set_with_explicit_double_quoted_renders_double_quotes

#### 入力
`"{\n  name: \"app\"\n}\n"` に対し、`string_style(Explicit(DoubleQuoted))` でパス `"name"` に `"api"` を set

#### 期待動作
`"{\n  name: \"api\"\n}\n"` が出力される

#### 備考

### set_with_explicit_block_renders_block_quotes

#### 入力
`"{\n  name: \"app\"\n}\n"` に対し、`string_style(Explicit(Block))` でパス `"name"` に `"api"` を set

#### 期待動作
`"{\n  name: '''api'''\n}\n"` が出力される

#### 備考

### set_with_explicit_unquoted_renders_without_quotes

#### 入力
`"{\n  name: \"app\"\n}\n"` に対し、`string_style(Explicit(Unquoted))` でパス `"name"` に `"api"` を set

#### 期待動作
`"{\n  name: api\n}\n"` が出力される（引用符なし）

#### 備考

---

## 15. CLI (tests/cli.rs)

### cli_set_multiple_values_outputs_updated_object

#### 入力
空オブジェクト `{}` のファイルに対し、`hjson-edit edit --set foo 1 --set bar 2 <file>`

#### 期待動作
1. コマンド成功
2. 標準出力に `"foo: 1"` が含まれる
3. 標準出力に `"bar: 2"` が含まれる

#### 備考

### cli_set_and_remove_combines_operations

#### 入力
`"{\n  foo: 1\n}"` のファイルに対し、`hjson-edit edit --set bar 2 --remove foo <file>`

#### 期待動作
1. コマンド成功
2. 標準出力に `"bar: 2"` が含まれる
3. 標準出力に `"foo: 1"` が含まれない

#### 備考
set と remove の組み合わせ

### cli_in_place_edit_writes_updated_file

#### 入力
`"{\n  foo: 1\n}"` のファイルに対し、`hjson-edit edit --in-place --set foo 2 <file>`

#### 期待動作
1. コマンド成功
2. ファイル内容に `"foo: 2"` が含まれる
3. ファイル内容に `"foo: 1"` が含まれない

#### 備考
`--in-place` によるファイル直接書き換え

### cli_missing_file_argument_fails_with_usage_message

#### 入力
ファイル引数なしで `hjson-edit edit --set foo 1`

#### 期待動作
1. コマンド失敗
2. 標準エラーにエラーメッセージが出力される

#### 備考

---

## 16. AST プロジェクション (tests/ast_projection.rs)

### nested_braced_object_uses_braced_delimiter_and_keeps_edge_trivia_separate

#### 入力
`"k: {\r\n    l: lll\r\n      m: mm\r\n    }\r\n"` をパース

#### 期待動作
1. ネストされたオブジェクトの `delimiter_style` が `Braced`
2. ネストオブジェクトの `leading_trivia` / `trailing_trivia` に `"      "`（6スペース）の Whitespace が含まれない（trivia はメンバーが所有）

#### 備考

### braced_array_reports_braced_delimiter_style

#### 入力
`"[\n  1\n]\n"` をパース

#### 期待動作
配列の `delimiter_style` が `Braced`

#### 備考

### root_scalar_trivia_is_owned_by_document_not_value

#### 入力
`"\r\n// header\r\n42\r\n// footer\r\n"` をパース

#### 期待動作
1. `DocumentItem::Value(_)` にマッチ
2. ドキュメントの `leading_trivia` に Comment が含まれる
3. ドキュメントの `trailing_trivia` に Comment が含まれる
4. 値の内容が Integer(raw: "42")

#### 備考
ルートスカラーの場合、trivia はドキュメントが所有し Value ノードは所有しない

### array_item_owns_item_trivia_and_exposes_value

#### 入力
`"[\n  // first\n  1,\n  2\n]\n"` をパース

#### 期待動作
1. 配列アイテム数が `2`
2. 最初のアイテムの `leading_trivia` に body が `"first"` の Comment が含まれる
3. 最初のアイテムの値が Integer(raw: "1")

#### 備考
配列アイテムは自身の trivia を所有する

---

## 17. BOM (tests/bom.rs)

### bom_at_start_is_ignored

#### 入力
`"\u{FEFF}{\n  a: 1\n}\n"`（BOM 付き）をパース

#### 期待動作
パース成功

#### 備考
RFC 8259 Section 9.1 に基づき BOM は無視される

---

## 18. ハッシュコメント動作 (tests/hash_comment_behavior.rs)

### hash_in_quoteless_string_is_part_of_value

#### 入力
`"key: value # comment"` をパースし、パス `"key"` で get

#### 期待動作
`HjsonValue::String("value # comment")` が返る

#### 備考
`#` が引用符なし文字列の中途にある場合、値の一部と解釈される

### hash_in_quoteless_key_is_part_of_key

#### 入力
`"key#value: 1"` をパースし、パス `"key#value"` で get

#### 期待動作
`HjsonValue::Integer(1)` が返る

#### 備考
`#` がキーの中途にある場合、キーの一部と解釈される

### hash_in_braceless_root_is_part_of_scalar_string

#### 入力
`"key# value : 1"` をパースし、空パス `""` で get

#### 期待動作
`HjsonValue::String("key# value : 1")` が返る

#### 備考
ブレースレスルートで `#` が出現した場合、全体がスカラー文字列と解釈される

### hash_at_line_start_is_comment

#### 入力
`"# comment\nkey: value"` をパースし、パス `"key"` で get

#### 期待動作
`HjsonValue::String("value")` が返る

#### 備考
行頭の `#` はコメント開始と解釈される

---

## 19. エラーフォーマット (tests/error_formatting.rs)

### parse_error_includes_line_and_column

#### 入力
`"{\n  \"a\": 1"`（閉じていないオブジェクト）をパース

#### 期待動作
1. エラーが返る
2. エラーメッセージに `"line"`, `"column"`, `"offset"` のいずれかが含まれる

#### 備考

### path_not_found_error_includes_path

#### 入力
有効なドキュメントに対し、存在しないパス `"missing.path"` で get

#### 期待動作
1. エラーが返る
2. エラーメッセージに `"missing"` または `"path"` が含まれる

#### 備考

---

## 20. Unicode (tests/unicode.rs)

### parse_unicode_key_and_value

#### 入力
`"日本語: 値"` をパースし、パス `"日本語"` で get

#### 期待動作
`HjsonValue::String("値")` が返る

#### 備考

### get_and_set_with_unicode_path

#### 入力
`"{\n  日本語: 値\n}\n"` をパース

#### 期待動作
1. `get("日本語")` で `HjsonValue::String("値")` が返る
2. `set("日本語", "新値")` 後、出力に `"新値"` が含まれる

#### 備考

---

## 21. インラインユニットテスト (src/)

### test_cast_value_node_to_mismatched_kinds_rejected (src/ast/mod.rs)

#### 入力
Value ノードを Object / Array / Document / Member / Key としてキャスト

#### 期待動作
全てエラーが返る

#### 備考
AstNode::cast の型安全性検証

### test_cast_object_node_to_mismatched_kinds_rejected (src/ast/mod.rs)

#### 入力
Object ノードを Value / Array としてキャスト

#### 期待動作
全てエラーが返る

#### 備考

### test_cast_comment_node_to_value_rejected (src/ast/mod.rs)

#### 入力
Comment ノードを Value としてキャスト

#### 期待動作
エラーが返る

#### 備考

### test_format_is_passthrough (src/format/mod.rs)

#### 入力
コメント付き HJSON を Formatter に通す

#### 期待動作
入力と出力が同一

#### 備考
現在の Formatter はパススルー（何も変更しない）

### test_cli_value_parser_keywords_and_numbers (src/cli/edit.rs)

#### 入力
`"true"`, `"false"`, `"null"`, `"123"`, `"3.14"` を `CliValueParser::parse_value` に渡す

#### 期待動作
それぞれ `Bool(true)`, `Bool(false)`, `Null`, `Integer(123)`, `Float(3.14に近い値)` が返る

#### 備考

### test_cli_value_parser_string_edge_cases (src/cli/edit.rs)

#### 入力
`""`, `"\""`, `"\"\""`, `"\"foo"`, `"'''"`, `"''''''"` をパース

#### 期待動作
それぞれ対応する `HjsonValue::String` が返る（空文字列や不完全なクォート含む）

#### 備考

### test_cli_value_parser_multibyte (src/cli/edit.rs)

#### 入力
`"\"あ\""`, `"'''あ'''"` をパース

#### 期待動作
それぞれ `HjsonValue::String("あ")` が返る

#### 備考

### test_render_comment_text (src/high_level/ops/comment.rs)

#### 入力
3種の CommentKind（Line / Hash / Block）とテキスト `"hello"`、空文字 `""`

#### 期待動作
`render_comment_text` がそれぞれ `"// hello"`, `"//"`, `"# hello"`, `"#"`, `"/* hello */"` を返す

#### 備考
空ボディの場合、Line / Hash は余分スペースを付けずに `//` / `#` のみを返す

### test_kind_from_style (src/high_level/ops/comment.rs)

#### 入力
3種の CommentKind

#### 期待動作
`kind_from_style` が対応する SyntaxKind を返す

#### 備考

### test_resolve_comment_style_explicit_respects_multiline_line (src/high_level/ops/comment.rs)

#### 入力
`Prefer::Explicit(CommentKind::Line)`

#### 期待動作
`CommentKind::Line` が返る

#### 備考
`resolve_comment_style` はボディの改行有無にかかわらず指定されたスタイルを返す。行分割は呼び出し側（`make_comment_nodes`）で行われる

### test_resolve_comment_style_preserve_falls_back_to_block (src/high_level/ops/comment.rs)

#### 入力
`Prefer::Preserve` で既存スタイルなし

#### 期待動作
`CommentKind::Block` が返る

#### 備考
Preserve かつ既存スタイルがない場合のデフォルトは Block

### test_resolve_comment_style_preserve_keeps_existing (src/high_level/ops/comment.rs)

#### 入力
`Prefer::Preserve` で既存スタイル `Some(CommentKind::Line)`

#### 期待動作
`CommentKind::Line` が返る

#### 備考

### test_resolve_comment_style_explicit_overrides (src/high_level/ops/comment.rs)

#### 入力
`Prefer::Explicit(CommentKind::Hash)` で既存スタイルなし

#### 期待動作
`CommentKind::Hash` が返る

#### 備考

### test_extract_comment_bodies_mixed_trivia (src/high_level/ops/comment.rs)

#### 入力
Whitespace + LineComment("first") + BlockComment("second") の trivia

#### 期待動作
`["first", "second"]` が返る

#### 備考
trivia からコメント body のみを抽出

### test_extract_comment_bodies_no_comments (src/high_level/ops/comment.rs)

#### 入力
Whitespace のみの trivia

#### 期待動作
空の Vec が返る

#### 備考

### test_join_comment_bodies_empty_trivia (src/high_level/ops/comment.rs)

#### 入力
Whitespace のみの trivia

#### 期待動作
`None` が返る

#### 備考

### test_join_comment_bodies_single_comment (src/high_level/ops/comment.rs)

#### 入力
LineComment("hello") のみの trivia

#### 期待動作
`Some("hello")` が返る

#### 備考

### test_join_comment_bodies_multiple_comments (src/high_level/ops/comment.rs)

#### 入力
LineComment("first") + BlockComment("second") の trivia

#### 期待動作
`Some("first\nsecond")` が返る

#### 備考
複数コメントは `\n` で連結

### test_trivia_comment_style (src/high_level/ops/comment.rs)

#### 入力
1. Line + Block の混在 trivia
2. コメントなし trivia
3. 空スライス

#### 期待動作
1. `Some(CommentKind::Line)` が返る（最初のコメントスタイル）
2. `None` が返る
3. `None` が返る

#### 備考

### test_resolve_comment_target_header (src/high_level/ops/comment.rs)

#### 入力
`HjsonCommentPath::Header`

#### 期待動作
`CommentTarget::Document { header: true }` にマッチ

#### 備考

### test_resolve_comment_target_footer (src/high_level/ops/comment.rs)

#### 入力
`HjsonCommentPath::Footer`

#### 期待動作
`CommentTarget::Document { header: false }` にマッチ

#### 備考

### test_resolve_comment_target_before (src/high_level/ops/comment.rs)

#### 入力
`HjsonCommentPath::Before(HjsonObjectPath::parse("a").unwrap())`

#### 期待動作
`CommentTarget::Member { location: MemberCommentLocation::Before, .. }` にマッチ

#### 備考

### test_resolve_comment_target_inline (src/high_level/ops/comment.rs)

#### 入力
`HjsonCommentPath::Inline(HjsonObjectPath::parse("a").unwrap())`

#### 期待動作
`CommentTarget::Member { location: MemberCommentLocation::Inline, .. }` にマッチ

#### 備考

### test_resolve_quote_style (src/high_level/ops/object.rs)

#### 入力
1. `Prefer::Preserve` + `Some(DoubleQuoted)`
2. `Prefer::Preserve` + `None`
3. `Prefer::Explicit(Unquoted)` + `None`

#### 期待動作
1. `DoubleQuoted`
2. `DoubleQuoted`
3. `Unquoted`

#### 備考
Preserve かつ既存スタイルなしのデフォルトは DoubleQuoted

### test_resolve_insert_style (src/high_level/ops/object.rs)

#### 入力
1. `Prefer::Preserve` + `Some(DoubleQuoted)`
2. `Prefer::Preserve` + `None`
3. `Prefer::Explicit(Unquoted)` + `Some(DoubleQuoted)`
4. `Prefer::Explicit(Unquoted)` + `None`

#### 期待動作
1. `Some(DoubleQuoted)`
2. `None`
3. `Some(Unquoted)`
4. `Some(Unquoted)`

#### 備考

### test_build_nested_value_single_key_wraps_in_object (src/high_level/ops/object.rs)

#### 入力
パス `["a"]` と値 `Integer(1)`

#### 期待動作
`HjsonValue::Object(vec![("a", Integer(1))])` が返る

#### 備考

### test_build_nested_value_nested_keys (src/high_level/ops/object.rs)

#### 入力
パス `["a", "b"]` と値 `Integer(1)`

#### 期待動作
`HjsonValue::Object(vec![("a", Object(vec![("b", Integer(1))]))])` が返る（ネスト構造）

#### 備考

### test_build_nested_value_array_segment (src/high_level/ops/object.rs)

#### 入力
パス `[Array(Index(0))]` と値 `Integer(1)`

#### 期待動作
`HjsonValue::Array(vec![Integer(1)])` が返る

#### 備考

### resolve_node_empty_path_returns_body (src/high_level/resolve.rs)

#### 入力
`"name: app\n"` をパースし、空パスで `resolve_node`

#### 期待動作
`SyntaxKind::Object` ノードが返る（ルートオブジェクト）

#### 備考

### resolve_node_simple_key (src/high_level/resolve.rs)

#### 入力
`"{\n  name: app\n}\n"` をパースし、パス `"name"` で resolve_node

#### 期待動作
`SyntaxKind::Value` ノードが返る

#### 備考

### resolve_node_nested_key (src/high_level/resolve.rs)

#### 入力
ネストオブジェクトに対しパス `"server.host"` で resolve_node

#### 期待動作
`SyntaxKind::Value` ノードが返る

#### 備考

### resolve_node_array_index (src/high_level/resolve.rs)

#### 入力
配列に対しパス `"items[0]"` で resolve_node

#### 期待動作
`SyntaxKind::Value` ノードが返る

#### 備考

### resolve_node_array_push_reads_last (src/high_level/resolve.rs)

#### 入力
`[1, 2, 3]` に対しパス `"items[-]"` で resolve_node

#### 期待動作
`SyntaxKind::Value` ノードが返り、値が `"3"`

#### 備考
`[-]` は末尾要素を指す（読み取り時）

### resolve_node_array_insert_at_reads_as_index (src/high_level/resolve.rs)

#### 入力
`[1, 2, 3]` に対しパス `"items[+0]"` で resolve_node

#### 期待動作
`SyntaxKind::Value` ノードが返り、値が `"1"`

#### 備考
`[+n]` は読み取り時は `[n]` と同じ動作

### resolve_node_not_found_missing_key (src/high_level/resolve.rs)

#### 入力
存在しないキー `"missing"` で resolve_node

#### 期待動作
`HjsonEditError::PathNotFound` エラー

#### 備考

### resolve_node_not_found_out_of_range_index (src/high_level/resolve.rs)

#### 入力
範囲外のインデックス `"items[10]"` で resolve_node

#### 期待動作
`HjsonEditError::PathNotFound` エラー

#### 備考

### resolve_node_not_found_array_segment_on_object (src/high_level/resolve.rs)

#### 入力
オブジェクト値に対して配列セグメント `"name[0]"` で resolve_node

#### 期待動作
`HjsonEditError::PathNotFound` エラー

#### 備考

### resolve_node_not_found_key_segment_on_array (src/high_level/resolve.rs)

#### 入力
配列ルートに対してキーセグメント `"name"` で resolve_node

#### 期待動作
`HjsonEditError::PathNotFound` エラー

#### 備考

### resolve_node_root_array (src/high_level/resolve.rs)

#### 入力
ルート配列 `[1, 2, 3]` に対しパス `"[1]"` で resolve_node

#### 期待動作
`SyntaxKind::Value` ノードが返り、値が `"2"`

#### 備考

### resolve_node_root_scalar (src/high_level/resolve.rs)

#### 入力
ルートスカラー `"123\n"` に対し空パスで resolve_node

#### 期待動作
`SyntaxKind::Value` ノードが返る

#### 備考

### resolve_node_braceless_root_object (src/high_level/resolve.rs)

#### 入力
ブレースレスルート `"name: app\nport: 8080\n"` に対しパス `"port"` で resolve_node

#### 期待動作
`SyntaxKind::Value` ノードが返り、値が `"8080"`

#### 備考

### resolve_parent_nested_object (src/high_level/resolve.rs)

#### 入力
パス `"server.host"` で resolve_parent

#### 期待動作
`parent` が `SyntaxKind::Value`（inner object をラップ）、`last` が `Key("host")`

#### 備考

### resolve_parent_array_element (src/high_level/resolve.rs)

#### 入力
パス `"items[0]"` で resolve_parent

#### 期待動作
`parent` が `SyntaxKind::Value`（array をラップ）、`last` が `Array(Index(0))`

#### 備考

### resolve_parent_empty_path_is_invalid (src/high_level/resolve.rs)

#### 入力
空パスで resolve_parent

#### 期待動作
`HjsonEditError::InvalidPath` エラー

#### 備考

### resolve_parent_partial_ok (src/high_level/resolve.rs)

#### 入力
パス `"server.missing"`（途中まで存在）で resolve_parent

#### 期待動作
`parent` が `SyntaxKind::Value`（既存部分のノード）、`last` が `Key("missing")`

#### 備考
親解決は部分パスでも成功する（存在しない最終セグメントを除いて解決）

### resolve_existing_prefix_fully_resolved (src/high_level/resolve.rs)

#### 入力
パス `"server.host"`（完全に存在）で resolve_existing_prefix

#### 期待動作
`consumed` が `2`、`ancestor` が `SyntaxKind::Value`

#### 備考

### resolve_existing_prefix_partial (src/high_level/resolve.rs)

#### 入力
パス `"root.new.nested"`（`root` まで存在）で resolve_existing_prefix

#### 期待動作
`consumed` が `1`、`ancestor` が `SyntaxKind::Value`

#### 備考

### resolve_existing_prefix_none (src/high_level/resolve.rs)

#### 入力
パス `"missing.path"`（全く存在しない）で resolve_existing_prefix

#### 期待動作
`consumed` が `0`、`ancestor` が `SyntaxKind::Value`（ドキュメントボディへのフォールバック）

#### 備考

### resolve_existing_prefix_array_then_key (src/high_level/resolve.rs)

#### 入力
パス `"items[0].name"`（配列→オブジェクト→キー）で resolve_existing_prefix

#### 期待動作
`consumed` が `3`

#### 備考

### resolve_existing_prefix_array_insert_at (src/high_level/resolve.rs)

#### 入力
パス `"items[+3].name"` で resolve_existing_prefix（範囲外の InsertAt）

#### 期待動作
`consumed` が `1`（items までしか解決できない）

#### 備考

### resolve_member_simple_key (src/high_level/resolve.rs)

#### 入力
パス `"name"` で resolve_member

#### 期待動作
`SyntaxKind::Member` ノードが返る

#### 備考

### resolve_member_nested (src/high_level/resolve.rs)

#### 入力
パス `"server.host"` で resolve_member

#### 期待動作
`SyntaxKind::Member` ノードが返り、`member_key()` が `Some("host")`

#### 備考

### resolve_member_array_then_key (src/high_level/resolve.rs)

#### 入力
パス `"items[0].name"` で resolve_member

#### 期待動作
`SyntaxKind::Member` ノードが返り、`member_key()` が `Some("name")`

#### 備考

### resolve_member_empty_path_is_invalid (src/high_level/resolve.rs)

#### 入力
空パスで resolve_member

#### 期待動作
`HjsonEditError::InvalidPath` エラー

#### 備考

### resolve_member_not_found (src/high_level/resolve.rs)

#### 入力
存在しないパス `"missing"` で resolve_member

#### 期待動作
`HjsonEditError::PathNotFound` エラー

#### 備考

### resolve_member_root_scalar_is_not_member (src/high_level/resolve.rs)

#### 入力
ルートスカラーに対し空パスで resolve_member

#### 期待動作
`HjsonEditError::InvalidPath` エラー

#### 備考

### test_detect_newline_lf (src/high_level/style.rs)

#### 入力
`"foo\nbar"`, `"\n"`

#### 期待動作
`NewlineKind::Lf` が返る

#### 備考

### test_detect_newline_crlf (src/high_level/style.rs)

#### 入力
`"foo\r\nbar"`, `"\r\n"`

#### 期待動作
`NewlineKind::CrLf` が返る

#### 備考

### test_detect_newline_mixed_uses_majority (src/high_level/style.rs)

#### 入力
`"foo\nbar\nbaz\r\n"`（LF=2, CRLF=1）、`"foo\r\nbar\r\nbaz\n"`（LF=1, CRLF=2）

#### 期待動作
前者は `NewlineKind::Lf`、後者は `NewlineKind::CrLf` が返る

#### 備考
混在時は多数決で判定される（同数の場合は LF 優先）

### test_detect_newline_cr_only_or_none (src/high_level/style.rs)

#### 入力
`"foo\rbar"`, `"foo bar"`, `""`

#### 期待動作
全て `NewlineKind::Lf` が返る（CR のみや改行なしは LF として扱う）

#### 備考

### test_detect_newline_string (src/high_level/style.rs)

#### 入力
`String::from("foo\r\nbar")`

#### 期待動作
`NewlineKind::CrLf` が返る

#### 備考
`String` 型でも動作することを確認

### test_is_simple_unquoted_text_accepts_alphanumeric_underscore_dash_dot (src/high_level/value.rs)

#### 入力
`"hello_world"`, `"foo"`, `"a-b.c_d-e0"`

#### 期待動作
全て `is_valid_for_text` が `true`

#### 備考

### test_is_simple_unquoted_text_rejects_space_and_punctuation (src/high_level/value.rs)

#### 入力
`"hello world"`, `"foo!"`, `""`

#### 期待動作
全て `is_valid_for_text` が `false`

#### 備考

### test_is_simple_unquoted_text_rejects_reserved_words (src/high_level/value.rs)

#### 入力
`"true"`, `"false"`, `"null"`

#### 期待動作
全て `is_valid_for_text` が `false`

#### 備考
予約語は引用符なしでは使用不可

### test_is_simple_unquoted_text_rejects_numeric_tokens (src/high_level/value.rs)

#### 入力
`"123"`, `"1.23"`

#### 期待動作
全て `is_valid_for_text` が `false`

#### 備考
数値トークンも引用符なしでは使用不可

### test_wrap_container_elements_empty_container (src/high_level/value.rs)

#### 入力
空コンテナ、depth=2、pretty=true

#### 期待動作
5要素が生成される（改行 + インデント空白 + 値 + 改行 + インデント空白）

#### 備考

### test_wrap_container_elements_pretty_not_empty (src/high_level/value.rs)

#### 入力
非空コンテナ、depth=2、pretty=true

#### 期待動作
5要素が生成される（同上）

#### 備考

### test_wrap_container_elements_not_pretty_not_empty (src/high_level/value.rs)

#### 入力
非空コンテナ、depth=1、pretty=false

#### 期待動作
1要素のみ（値のみ、ラップなし）

#### 備考

### test_render_body_string_default_style (src/high_level/value.rs)

#### 入力
`HjsonValue::String("foo")`、デフォルト RenderConfig

#### 期待動作
`render_body` が `SyntaxKind::UnquotedString` の要素を返す

#### 備考

### test_render_body_string_double_quoted_style (src/high_level/value.rs)

#### 入力
`HjsonValue::String("foo")`、`string_style = Some(DoubleQuoted)`

#### 期待動作
`render_body` が `SyntaxKind::String` の要素を返す

#### 備考

### test_render_body_string_block_style (src/high_level/value.rs)

#### 入力
`HjsonValue::String("foo")`、`string_style = Some(Block)`

#### 期待動作
`render_body` が `SyntaxKind::BlockString` の要素を返す

#### 備考

### test_render_body_array_empty (src/high_level/value.rs)

#### 入力
`HjsonValue::Array(vec![])`、デフォルト RenderConfig

#### 期待動作
`render_body` が1要素を返す

#### 備考

### test_render_body_object_empty (src/high_level/value.rs)

#### 入力
`HjsonValue::Object(vec![])`、デフォルト RenderConfig

#### 期待動作
`render_body` が1要素を返す

#### 備考

### test_cursor_basics (src/syntax/lexer.rs)

#### 入力
マルチバイト文字 `"aあb"` を含む文字列で Cursor 操作

#### 期待動作
1. `peek()` で先頭文字が取得できる
2. `peek_n(1)` で1つ先の文字が取得できる
3. `peek_prev()` で前の文字が取得できる
4. `advance()` でカーソルが進む
5. 終端では `peek()` が `None`、`is_at_end()` が `true`
6. 終端での `advance()` は `None` を返す

#### 備考
マルチバイト文字（UTF-8）でも正しく動作することを確認

### test_cursor_match (src/syntax/lexer.rs)

#### 入力
`"foo bar"` で match_str / match_char 操作

#### 期待動作
1. `match_str("foo")` が `true` で位置が `3`
2. `match_str("baz")` が `false` で位置が `3` のまま
3. `match_char(' ')` が `true` で位置が `4`

#### 備考

### test_lexer_hash_comment_start (src/syntax/lexer.rs)

#### 入力
1. `"# foo\n"`（行頭の `#`）
2. `"  # foo\n"`（空白の後の `#`）
3. `"a# foo\n"`（テキスト中の `#`）

#### 期待動作
1. `is_hash_comment_start()` が `true`
2. `true`
3. `false`

#### 備考
`#` が行頭（空白のみを許容）にある場合のみコメント開始と判定

### test_lexer_invalid_escape_sequence_produces_error (src/syntax/lexer.rs)

#### 入力
`r#""\x""#`

#### 期待動作
エラーが発生し、メッセージに `"Invalid escape sequence: \\x"` が含まれる

#### 備考

### test_lexer_valid_escape_sequence_produces_no_error (src/syntax/lexer.rs)

#### 入力
`r#""\"""#`

#### 期待動作
エラーなし

#### 備考

### test_lexer_eof_after_backslash_produces_error (src/syntax/lexer.rs)

#### 入力
`r#""\"#`

#### 期待動作
エラーが発生し、メッセージに `"Unterminated string literal"` が含まれる

#### 備考

### test_parser_recover_from_missing_colon_reports_error_but_produces_ast (src/syntax/parser.rs)

#### 入力
`"{\n  \"key\" \"value\"\n}"`（コロン欠落）

#### 期待動作
1. エラーが報告される
2. メンバー数が `1` の AST が生成される

#### 備考
パーサーはエラーリカバリを行い、部分的な AST を生成する

### test_parser_recover_unclosed_object_reports_error_but_produces_ast (src/syntax/parser.rs)

#### 入力
`"{ \"a\": 1"`（閉じていないオブジェクト）

#### 期待動作
1. エラーが報告される
2. メンバー数が `1` の AST が生成される

#### 備考

### test_parser_recover_unclosed_array_reports_error_but_produces_ast (src/syntax/parser.rs)

#### 入力
`"[ 1, 2"`（閉じていない配列）

#### 期待動作
1. エラーが報告される
2. アイテム数が `2` の AST が生成される

#### 備考
