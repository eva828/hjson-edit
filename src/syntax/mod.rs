/// 入力の最大バイト数。
///
/// 構文木の位置情報（rowan の `TextSize`）が u32 のため、これを超える入力は扱えない。
pub const MAX_INPUT_BYTES: usize = u32::MAX as usize;

pub mod lexer;
pub mod parser;
pub mod scan;

use rowan::Language;
use rowan::{SyntaxNode, SyntaxToken};
use std::fmt;

/// 改行コードの種類
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum NewlineKind {
    /// Line Feed (`\n`)
    #[default]
    Lf,
    /// Carriage Return + Line Feed (`\r\n`)
    CrLf,
}

impl NewlineKind {
    /// 改行コードを文字列として取得
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }

    /// 改行文字列から改行コード種別を解析
    pub fn from_text(text: &str) -> Option<Self> {
        match text {
            "\n" => Some(Self::Lf),
            "\r\n" => Some(Self::CrLf),
            _ => None,
        }
    }
}

/// HJSON構文要素の種別
///
/// 行コメント、ブロックコメント、空白も重要な構文要素として保持する
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum SyntaxKind {
    // ===== 字句トークン =====
    // 構文記号
    /// `{`。
    LBrace,
    /// `}`。
    RBrace,
    /// `[`。
    LBracket,
    /// `]`。
    RBracket,
    /// `:`。
    Colon,
    /// `,`。
    Comma,

    // リテラル値
    /// `true`。
    True,
    /// `false`。
    False,
    /// `null`。
    Null,

    // 数値
    /// 整数。
    Integer,
    /// 浮動小数。
    Float,

    // 文字列
    /// 引用符付きの文字列。
    String,
    /// 複数行文字列。
    BlockString,
    /// 引用符なし文字列。
    UnquotedString,

    // コメント
    /// `//` の行コメント。
    LineComment,
    /// `#` の行コメント。
    HashComment,
    /// ブロックコメント。
    BlockComment,

    // 空白・改行
    /// 空白・タブ。
    Whitespace,
    /// 改行。
    Newline,

    // ===== 構文木ノード =====
    // ルート
    /// 文書のルート。
    Root,

    // 値
    /// オブジェクト。
    Object,
    /// 配列。
    Array,
    /// メンバー（キー・区切り・値）。
    Member,
    /// キー。
    Key,
    /// 値。
    Value,

    // コメントノード
    /// コメント。
    Comment,

    // エラー回復用
    /// エラーを表すトークン。
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
            22 => SyntaxKind::Member,
            23 => SyntaxKind::Key,
            24 => SyntaxKind::Value,
            25 => SyntaxKind::Comment,
            26 => SyntaxKind::Error,
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

    /// 構造的トークンの正準テキストを返す
    pub fn canonical_text(self) -> &'static str {
        match self {
            SyntaxKind::Comma => ",",
            SyntaxKind::Colon => ":",
            SyntaxKind::LBrace => "{",
            SyntaxKind::RBrace => "}",
            SyntaxKind::LBracket => "[",
            SyntaxKind::RBracket => "]",
            SyntaxKind::Whitespace => " ",
            SyntaxKind::True => "true",
            SyntaxKind::False => "false",
            SyntaxKind::Null => "null",
            SyntaxKind::Newline => "\n",
            _ => panic!("SyntaxKind::{:?} has no canonical text", self),
        }
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
        matches!(
            self,
            Root | Object | Array | Member | Key | Value | Comment | Error
        )
    }

    /// トリビア（空白・コメントトークン）かどうか
    /// Comment ノードは含めない（トークンとノードを区別するため）
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
        matches!(self, LineComment | HashComment | BlockComment | Comment)
    }
}

/// GreenElement型エイリアス（NodeまたはToken）
pub type GreenElement = rowan::NodeOrToken<rowan::GreenNode, rowan::GreenToken>;

/// RedNode型エイリアス
pub type RedNode = SyntaxNode<HjsonLanguage>;
/// RedToken型エイリアス
pub type RedToken = SyntaxToken<HjsonLanguage>;
