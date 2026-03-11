use rowan::Language;

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

    // 識別子（キーとして使用可能）
    Ident, // 識別子

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

/// HJSON言語定義（rowan用）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HjsonLanguage {}

impl Language for HjsonLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
        #[allow(clippy::enum_glob_use)]
        use SyntaxKind::*;
        const KINDS: &[SyntaxKind] = &[
            LBrace, RBrace, LBracket, RBracket, Colon, Comma,
            True, False, Null,
            Integer, Float,
            String, BlockString, UnquotedString,
            Ident,
            LineComment, HashComment, BlockComment,
            Whitespace, Newline,
            Root,
            Object, Array, Pair, Key, Value,
            Error,
        ];
        KINDS[raw.0 as usize]
    }

    fn kind_to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
        kind.into()
    }
}

impl SyntaxKind {
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
                | Ident
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
        matches!(
            self,
            LineComment | HashComment | BlockComment
        )
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
}
