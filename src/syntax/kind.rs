use rowan::Language;
use std::fmt;

/// HJSON構文要素の種別
///
/// 行コメント、ブロックコメント、空白も重要な構文要素として保持する
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    // ===== 字句トークン =====
    // 構文記号
    LBrace,   // {
    RBrace,   // }
    LBracket, // [
    RBracket, // ]
    Colon,    // :
    Comma,    // ,

    // リテラル値
    True,  // true
    False, // false
    Null,  // null

    // 数値
    Integer, // 整数（123）
    Float,   // 浮動小数点（1.23）

    // 文字列
    String,         // "..."
    BlockString,    // '''...'''
    UnquotedString, // 引用符なし文字列

    // コメント
    LineComment,  // // 行コメント
    HashComment,  // # 行コメント
    BlockComment, // /* ブロックコメント */

    // 空白・改行
    Whitespace, // 空白・タブ
    Newline,    // 改行

    // ===== 構文木ノード =====
    // ルート
    Root,

    // 値
    Object,
    Array,
    Pair, // キー: 値
    Key,
    Value,

    // エラー回復用
    Error,
}

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        Self(kind as u16)
    }
}

impl fmt::Display for SyntaxKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// HJSON言語定義（rowan用）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HjsonLanguage {}

impl Language for HjsonLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
        match raw.0 {
            0 => SyntaxKind::LBrace,
            1 => SyntaxKind::RBrace,
            2 => SyntaxKind::LBracket,
            3 => SyntaxKind::RBracket,
            4 => SyntaxKind::Colon,
            5 => SyntaxKind::Comma,
            6 => SyntaxKind::True,
            7 => SyntaxKind::False,
            8 => SyntaxKind::Null,
            9 => SyntaxKind::Integer,
            10 => SyntaxKind::Float,
            11 => SyntaxKind::String,
            12 => SyntaxKind::BlockString,
            13 => SyntaxKind::UnquotedString,
            14 => SyntaxKind::LineComment,
            15 => SyntaxKind::HashComment,
            16 => SyntaxKind::BlockComment,
            17 => SyntaxKind::Whitespace,
            18 => SyntaxKind::Newline,
            19 => SyntaxKind::Root,
            20 => SyntaxKind::Object,
            21 => SyntaxKind::Array,
            22 => SyntaxKind::Pair,
            23 => SyntaxKind::Key,
            24 => SyntaxKind::Value,
            25 => SyntaxKind::Error,
            n => panic!("Unknown SyntaxKind: {n}"),
        }
    }

    fn kind_to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
        kind.into()
    }
}

impl SyntaxKind {
    /// 新しいトークンを作成
    pub fn create_token(self, text: impl Into<String>) -> rowan::GreenToken {
        rowan::GreenToken::new(self.into(), text.into().as_str())
    }

    /// 新しいノードを作成
    pub fn create_node(
        self,
        children: Vec<crate::syntax::green::GreenElement>,
    ) -> rowan::GreenNode {
        use crate::syntax::green::GreenElement;
        use rowan::GreenNodeBuilder;

        fn push_element(builder: &mut GreenNodeBuilder<'static>, element: &GreenElement) {
            match element {
                rowan::NodeOrToken::Node(node) => {
                    builder.start_node(node.kind());
                    for child in node.children() {
                        let elem = child_to_green_element(child);
                        push_element(builder, &elem);
                    }
                    builder.finish_node();
                }
                rowan::NodeOrToken::Token(token) => {
                    let kind: rowan::SyntaxKind = token.kind();
                    builder.token(kind, token.text());
                }
            }
        }

        fn child_to_green_element(
            child: rowan::NodeOrToken<&rowan::GreenNodeData, &rowan::GreenTokenData>,
        ) -> GreenElement {
            match child {
                rowan::NodeOrToken::Node(node_data) => rowan::NodeOrToken::Node(
                    rowan::GreenNode::from(std::borrow::Cow::Borrowed(node_data)),
                ),
                rowan::NodeOrToken::Token(token_data) => rowan::NodeOrToken::Token(
                    rowan::GreenToken::new(token_data.kind(), token_data.text()),
                ),
            }
        }

        let mut builder = GreenNodeBuilder::new();
        builder.start_node(self.into());
        for child in children {
            push_element(&mut builder, &child);
        }
        builder.finish_node();
        builder.finish()
    }

    /// トークンかどうか
    pub fn is_token(self) -> bool {
        use SyntaxKind::*;
        matches!(
            self,
            LBrace
                | RBrace
                | LBracket
                | RBracket
                | Colon
                | Comma
                | True
                | False
                | Null
                | Integer
                | Float
                | String
                | BlockString
                | UnquotedString
                | LineComment
                | HashComment
                | BlockComment
                | Whitespace
                | Newline
        )
    }

    /// ノードかどうか
    pub fn is_node(self) -> bool {
        use SyntaxKind::*;
        matches!(self, Root | Object | Array | Pair | Key | Value | Error)
    }

    /// トリビア（空白・コメント）かどうか
    pub fn is_trivia(self) -> bool {
        use SyntaxKind::*;
        matches!(
            self,
            Whitespace | Newline | LineComment | HashComment | BlockComment
        )
    }

    /// コメントかどうか
    pub fn is_comment(self) -> bool {
        use SyntaxKind::*;
        matches!(self, LineComment | HashComment | BlockComment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SyntaxKindからrowan::SyntaxKindへの変換と逆変換が一致することを検証
    #[test]
    fn test_kind_conversion() {
        let kind = SyntaxKind::LBrace;
        let raw: rowan::SyntaxKind = kind.into();
        let converted = HjsonLanguage::kind_from_raw(raw);
        assert_eq!(kind, converted);
    }

    /// 全SyntaxKindバリアントの往復変換（kind_to_raw → kind_from_raw）が一致することを検証
    ///
    /// これにより kind_from_raw の match アームが全バリアントを網羅していることが
    /// テスト実行時に保証される。バリアントを追加した場合はここに追加すること。
    #[test]
    fn test_all_kind_roundtrip() {
        use SyntaxKind::*;
        let all_kinds = [
            LBrace,
            RBrace,
            LBracket,
            RBracket,
            Colon,
            Comma,
            True,
            False,
            Null,
            Integer,
            Float,
            String,
            BlockString,
            UnquotedString,
            LineComment,
            HashComment,
            BlockComment,
            Whitespace,
            Newline,
            Root,
            Object,
            Array,
            Pair,
            Key,
            Value,
            Error,
        ];
        for kind in all_kinds {
            let raw = HjsonLanguage::kind_to_raw(kind);
            let roundtripped = HjsonLanguage::kind_from_raw(raw);
            assert_eq!(kind, roundtripped, "Roundtrip failed for {:?}", kind);
        }
    }
}
